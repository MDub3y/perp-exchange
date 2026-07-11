use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;
use rust_decimal_macros::dec;
use utils::{
    Fill, Market, Order, OrderRequests, OrderSide, OrderType, Position, PositionSide, UserBalance,
};
use uuid::Uuid;

use super::ExecuteEngine;

const PERSISTENCE_STREAM: &str = "exchange:persistence:stream";
const IMR_RATIO: Decimal = dec!(0.05); // 5% Initial Margin (20x Maximum Leverage)

impl ExecuteEngine {
    pub async fn handle_order_request(&mut self, request: OrderRequests) {
        match request {
            OrderRequests::CreateOrder(req) => {
                let mut required_margin = Decimal::ZERO;
                let mut insufficient_funds = false;

                if req.order_type == OrderType::LIMIT {
                    // Initial Margin Requirement Check: Quantity * Price * IMR_RATIO
                    required_margin = req.quantity * req.price * IMR_RATIO;

                    let wallet = self.user_wallets.entry(req.user_id).or_insert(UserBalance {
                        available_balance: Decimal::ZERO,
                        locked_balance: Decimal::ZERO,
                    });

                    if wallet.available_balance < required_margin {
                        insufficient_funds = true;
                    } else {
                        wallet.available_balance -= required_margin;
                        wallet.locked_balance += required_margin;
                        println!(
                            "[MARGIN LOCK] Locked global order collateral: ${} for User {}",
                            required_margin, req.user_id
                        );
                    }
                }

                if insufficient_funds {
                    println!(
                        "[RISK REJECTION]: Insufficient available margin for User {}. Required IM: ${}",
                        req.user_id, required_margin
                    );
                    return;
                }

                let order_item = Order {
                    user_id: req.user_id,
                    order_id: req.order_id,
                    price: req.price,
                    quantity: req.quantity,
                };
                let mut match_result = None;
                let mut depth_payload = String::new();

                {
                    if let Some(book) = self.orderbooks.get_mut(&req.market) {
                        if let Ok(result) = book.process_order(order_item, req.side, req.order_type)
                        {
                            depth_payload =
                                serde_json::to_string(&book.get_depth()).unwrap_or_default();
                            match_result = Some(result);
                        }
                    }
                }

                if let Some(result) = match_result {
                    self.settle_balances(&result.fills, req.side, req.order_type, required_margin);

                    let execution_json = serde_json::to_string(&result).unwrap_or_default();
                    let _ = self
                        .redis
                        .enqueue_persistence_log(PERSISTENCE_STREAM, &execution_json)
                        .await;

                    if !depth_payload.is_empty() {
                        let _ = self
                            .redis
                            .publish_market_update(req.market, "depth", &depth_payload)
                            .await;
                    }

                    for fill in &result.fills {
                        let serialized_fill = serde_json::to_string(fill).unwrap_or_default();
                        let _ = self
                            .redis
                            .publish_user_update(&fill.taker_user_id.to_string(), &serialized_fill)
                            .await;
                        let _ = self
                            .redis
                            .publish_user_update(&fill.maker_user_id.to_string(), &serialized_fill)
                            .await;
                    }
                }
                self.print_book_matrix(req.market);
            }

            OrderRequests::CancelOrder(req) => {
                let mut canceled_order = None;
                {
                    if let Some(book) = self.orderbooks.get_mut(&req.market) {
                        if let Ok(Some(order)) = book.cancel_order(req.order_id) {
                            canceled_order = Some(order);
                        }
                    }
                }
                if let Some(order) = canceled_order {
                    if let Some(wallet) = self.user_wallets.get_mut(&req.user_id) {
                        let returned_margin = order.quantity * order.price * IMR_RATIO;
                        wallet.locked_balance -= returned_margin;
                        wallet.available_balance += returned_margin;
                        println!(
                            "[CANCELLATION UNLOCK] Released pending order margin: ${}",
                            returned_margin
                        );
                    }
                }
                self.print_book_matrix(req.market);
            }

            OrderRequests::GetOpenOrders(req) => {
                if let Some(book) = self.orderbooks.get(&req.market) {
                    let open_orders = book.get_open_orders_for_user(req.user_id);
                    if let Ok(serialized) = serde_json::to_string(&open_orders) {
                        let _ = self
                            .redis
                            .publish_user_update(&req.user_id.to_string(), &serialized)
                            .await;
                    }
                }
            }
            other => self.execute_valuation_routines(other).await,
        }
    }

    fn settle_balances(
        &mut self,
        fills: &[Fill],
        taker_side: OrderSide,
        taker_type: OrderType,
        initial_locked_margin: Decimal,
    ) {
        for fill in fills {
            let matched_notional = fill.price * fill.quantity;
            let transaction_margin = matched_notional * IMR_RATIO;
            self.last_trade_prices.insert(fill.market, fill.price);

            // Settle Maker: Release order collateral from global locked_balance and transition to isolated margin account
            if let Some(maker_wallet) = self.user_wallets.get_mut(&fill.maker_user_id) {
                maker_wallet.locked_balance -= transaction_margin;
            }

            // Settle Taker
            if let Some(taker_wallet) = self.user_wallets.get_mut(&fill.taker_user_id) {
                match taker_type {
                    OrderType::LIMIT => {
                        // Order was already locked in global state; deduct from there
                        taker_wallet.locked_balance -= transaction_margin;
                    }
                    OrderType::MARKET => {
                        // Aggressive market order fills directly out of available capital instantly
                        taker_wallet.available_balance -= transaction_margin;
                    }
                }
            }

            let (long_user, short_user) = match taker_side {
                OrderSide::BUY => (fill.taker_user_id, fill.maker_user_id),
                OrderSide::SELL => (fill.maker_user_id, fill.taker_user_id),
            };

            self.update_isolated_position_inventory(
                long_user,
                fill.market,
                fill.quantity,
                fill.price,
                transaction_margin,
            );
            self.update_isolated_position_inventory(
                short_user,
                fill.market,
                -fill.quantity,
                fill.price,
                transaction_margin,
            );
        }

        // Return any unexecuted excess margin from partial fills back to the taker's available balance
        if taker_type == OrderType::LIMIT && initial_locked_margin > Decimal::ZERO {
            let actual_executed_margin = fills
                .iter()
                .map(|f| f.price * f.quantity * IMR_RATIO)
                .sum::<Decimal>();
            let remainder = initial_locked_margin - actual_executed_margin;
            if remainder > Decimal::ZERO {
                if let Some(taker_wallet) = self.user_wallets.get_mut(&fills[0].taker_user_id) {
                    taker_wallet.locked_balance -= remainder;
                    taker_wallet.available_balance += remainder;
                }
            }
        }
    }

    fn update_isolated_position_inventory(
        &mut self,
        user_id: Uuid,
        market: Market,
        size_delta: Decimal,
        fill_price: Decimal,
        allocated_margin: Decimal,
    ) {
        let markets_map = self.user_positions.entry(user_id).or_default();
        let position = markets_map.entry(market).or_insert(Position {
            market,
            size: Decimal::ZERO,
            qty: Decimal::ZERO,
            side: PositionSide::Long,
            margin: Decimal::ZERO,
            liquidation_price: Decimal::ZERO,
            avg_entry_price: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
        });

        if position.size.is_zero() {
            position.size = size_delta;
            position.qty = size_delta.abs();
            position.margin = allocated_margin;
            position.avg_entry_price = fill_price;
        } else if position.size.is_sign_positive() == size_delta.is_sign_positive() {
            // Increasing exposure: Lock additional margin and adjust the weighted cost-basis
            let new_size = position.size + size_delta;
            let current_notional = position.size.abs() * position.avg_entry_price;
            let fill_notional = size_delta.abs() * fill_price;

            position.avg_entry_price = (current_notional + fill_notional) / new_size.abs();
            position.size = new_size;
            position.qty = new_size.abs();
            position.margin += allocated_margin;
        } else {
            // Reducing or reversing exposure
            let current_abs = position.size.abs();
            let delta_abs = size_delta.abs();

            if delta_abs < current_abs {
                // Partial Close: Reduce size and return a proportional chunk of margin to available balance
                let reduction_ratio = delta_abs / current_abs;
                let released_margin = position.margin * reduction_ratio;

                position.margin -= released_margin;
                position.size += size_delta;
                position.qty = position.size.abs();

                if let Some(wallet) = self.user_wallets.get_mut(&user_id) {
                    wallet.available_balance += released_margin;
                }
            } else if delta_abs == current_abs {
                // Full Close: Wipe state clear and return all remaining isolated margin back to available balance
                let closing_margin = position.margin;
                *position = Position {
                    market,
                    size: Decimal::ZERO,
                    qty: Decimal::ZERO,
                    side: PositionSide::Long,
                    margin: Decimal::ZERO,
                    liquidation_price: Decimal::ZERO,
                    avg_entry_price: Decimal::ZERO,
                    unrealized_pnl: Decimal::ZERO,
                };
                if let Some(wallet) = self.user_wallets.get_mut(&user_id) {
                    wallet.available_balance += closing_margin;
                }
            } else {
                // Directional Position Flip (Net Mode)
                let net_new_qty = delta_abs - current_abs;
                let excess_margin = allocated_margin - (current_abs * fill_price * IMR_RATIO);

                let old_margin = position.margin;
                position.size = if size_delta.is_sign_positive() {
                    net_new_qty
                } else {
                    -net_new_qty
                };
                position.qty = net_new_qty;
                position.avg_entry_price = fill_price;
                position.margin = excess_margin.max(net_new_qty * fill_price * IMR_RATIO);

                if let Some(wallet) = self.user_wallets.get_mut(&user_id) {
                    wallet.available_balance += old_margin;
                }
            }
        }

        // Recalculate the position's liquidation boundaries instantly based on updated metrics
        if !position.size.is_zero() {
            let mmr_ratio = dec!(0.01); // 1% Maintenance Margin requirement
            if position.size.is_sign_positive() {
                position.liquidation_price = (position.avg_entry_price
                    - (position.margin / position.qty))
                    / (dec!(1.0) - mmr_ratio);
            } else {
                position.liquidation_price = (position.avg_entry_price
                    + (position.margin / position.qty))
                    / (dec!(1.0) + mmr_ratio);
            }
        }
    }
}
