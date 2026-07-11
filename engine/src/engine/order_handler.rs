use rust_decimal::Decimal;
use utils::{Fill, Market, Order, OrderRequests, OrderSide, OrderType, Position, UserBalance};
use uuid::Uuid;

use super::ExecuteEngine;

const PERSISTENCE_STREAM: &str = "exchange:persistence:stream";

impl ExecuteEngine {
    pub async fn handle_order_request(&mut self, request: OrderRequests) {
        match request {
            OrderRequests::MarkTick => {
                self.calculate_all_market_mark_prices().await;
            }
            OrderRequests::ExternalMarkUpdate { market, price } => {
                self.external_marks.insert(market, price);
            }
            OrderRequests::IndexUpdate(update) => {
                if update.price.is_zero() {
                    self.sample_premium_indices().await;
                } else {
                    self.index_prices.insert(update.market, update.price);
                }
            }

            OrderRequests::CreateOrder(req) => {
                let mut liability = Decimal::ZERO;
                let mut insufficient_funds = false;

                if req.order_type == OrderType::LIMIT {
                    liability = req.price * req.quantity;
                    let wallet = self.user_wallets.entry(req.user_id).or_insert(UserBalance {
                        available_balance: Decimal::ZERO,
                        locked_balance: Decimal::ZERO,
                    });

                    if wallet.available_balance < liability {
                        insufficient_funds = true;
                    } else {
                        wallet.available_balance -= liability;
                        wallet.locked_balance += liability;
                        println!(
                            "[BALANCE LOCK] User {}: Available = ${}, Locked = ${}",
                            req.user_id, wallet.available_balance, wallet.locked_balance
                        );
                    }
                }

                if insufficient_funds {
                    println!(
                        "[RISK REJECTION]: Insufficient available collateral for User {}. Required: ${}",
                        req.user_id, liability
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
                    self.settle_balances(&result.fills, req.side, req.order_type, liability);

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
                        let returned_capital = order.price * order.quantity;
                        wallet.locked_balance -= returned_capital;
                        wallet.available_balance += returned_capital;
                        println!(
                            "[BALANCE UNLOCK] Order {} Canceled. Capital Returned: ${}",
                            req.order_id, returned_capital
                        );
                    }
                } else {
                    println!(
                        "Order {} was not found active inside targeted book context",
                        req.order_id
                    );
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
                        println!(
                            "[ENGINE QUERY] Broadcasted open orders snapshot for user {}",
                            req.user_id
                        );
                    }
                }
            }
        }
    }

    fn settle_balances(
        &mut self,
        fills: &[Fill],
        taker_side: OrderSide,
        taker_type: OrderType,
        _initial_liability: Decimal,
    ) {
        for fill in fills {
            let matched_value = fill.price * fill.quantity;
            self.last_trade_prices.insert(fill.market, fill.price);

            if let Some(maker_wallet) = self.user_wallets.get_mut(&fill.maker_user_id) {
                maker_wallet.locked_balance -= matched_value;
                maker_wallet.available_balance += matched_value;
            }

            if let Some(taker_wallet) = self.user_wallets.get_mut(&fill.taker_user_id) {
                match taker_type {
                    OrderType::LIMIT => {
                        taker_wallet.locked_balance -= matched_value;
                        taker_wallet.available_balance += matched_value;
                    }
                    OrderType::MARKET => {
                        taker_wallet.available_balance -= matched_value;
                    }
                }
            }

            let (long_user, short_user) = match taker_side {
                OrderSide::BUY => (fill.taker_user_id, fill.maker_user_id),
                OrderSide::SELL => (fill.maker_user_id, fill.taker_user_id),
            };

            self.update_user_position_inventory(long_user, fill.market, fill.quantity, fill.price);
            self.update_user_position_inventory(
                short_user,
                fill.market,
                -fill.quantity,
                fill.price,
            );
        }
    }

    fn update_user_position_inventory(
        &mut self,
        user_id: Uuid,
        market: Market,
        size_delta: Decimal,
        fill_price: Decimal,
    ) {
        let markets_map = self.user_positions.entry(user_id).or_default();
        let position = markets_map.entry(market).or_insert(Position {
            market,
            size: Decimal::ZERO,
            avg_entry_price: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
        });

        if position.size.is_zero() {
            position.size = size_delta;
            position.avg_entry_price = fill_price;
        } else if position.size.is_sign_positive() == size_delta.is_sign_positive() {
            let new_size = position.size + size_delta;
            let current_notional = position.size.abs() * position.avg_entry_price;
            let fill_notional = size_delta.abs() * fill_price;

            position.avg_entry_price = (current_notional + fill_notional) / new_size.abs();
            position.size = new_size;
        } else {
            let current_abs = position.size.abs();
            let delta_abs = size_delta.abs();

            if delta_abs < current_abs {
                position.size += size_delta;
            } else if delta_abs == current_abs {
                position.size = Decimal::ZERO;
                position.avg_entry_price = Decimal::ZERO;
                position.unrealized_pnl = Decimal::ZERO;
            } else {
                let remaining_qty = delta_abs - current_abs;
                position.size = if size_delta.is_sign_positive() {
                    remaining_qty
                } else {
                    -remaining_qty
                };
                position.avg_entry_price = fill_price;
            }
        }
    }
}
