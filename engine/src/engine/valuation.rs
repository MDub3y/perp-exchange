use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;
use rust_decimal_macros::dec;
use utils::{FundingTelemetry, MarkPriceTelemetry, Market, OrderRequests, Position};

use super::ExecuteEngine;

const MMR_RATIO: Decimal = dec!(0.01); // 1% Maintenance Margin Requirement

impl ExecuteEngine {
    pub async fn execute_valuation_routines(&mut self, request: OrderRequests) {
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
            _ => unreachable!(),
        }
    }

    pub async fn calculate_all_market_mark_prices(&mut self) {
        let markets = [Market::SOL_PERP, Market::BTC_PERP, Market::ETH_PERP];

        for market in markets {
            let index = *self.index_prices.get(&market).unwrap_or(&dec!(1.0));
            let book = self.orderbooks.get(&market).unwrap();

            let best_bid = book.peek_best_bid();
            let best_ask = book.peek_best_ask();

            // Candidate 1: Smoothed Order Book Mid
            let alpha = dec!(0.002663);
            let mut c1 = index;
            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                let mid = (bid + ask) / dec!(2.0);
                let prev_ema = self.c1_ema_state.entry(market).or_default();
                *prev_ema = (alpha * (mid - index)) + ((dec!(1.0) - alpha) * (*prev_ema));
                c1 = index + *prev_ema;
            }

            // Candidate 2: Local Market Activity Median
            let last_trade = *self.last_trade_prices.get(&market).unwrap_or(&index);
            let c2 = match (best_bid, best_ask) {
                (Some(bid), Some(ask)) => self.median_of_three(bid, ask, last_trade),
                _ => index,
            };

            // Candidate 3: External Feed Target Price
            let c3 = *self.external_marks.get(&market).unwrap_or(&index);

            let raw_mark = self.median_of_three(c1, c2, c3);
            let final_mark = raw_mark.round_dp(4);
            self.mark_prices.insert(market, final_mark);

            // Update PnL matrices and run real-time liquidation health checks
            self.recalibrate_unrealized_pnl_and_check_liquidations(market)
                .await;

            let telemetry = MarkPriceTelemetry {
                market,
                market_price: final_mark,
                c1_smoothed: c1,
                c2_local: c2,
                c3_external: c3,
            };
            if let Ok(telemetry_json) = serde_json::to_string(&telemetry) {
                let _ = self
                    .redis
                    .publish_market_update(market, "mark_price", &telemetry_json)
                    .await;
            }
        }
    }

    async fn recalibrate_unrealized_pnl_and_check_liquidations(&mut self, market: Market) {
        let mark_price = *self.mark_prices.get(&market).unwrap_or(&dec!(1.0));
        let mut liquidated_users = Vec::new();

        for (&user_id, positions_map) in self.user_positions.iter_mut() {
            if let Some(position) = positions_map.get_mut(&market) {
                if position.size.is_zero() {
                    continue;
                }

                // Live Unrealized PnL Calculation: size * (MarkPrice - EntryPrice)
                position.unrealized_pnl = position.size * (mark_price - position.avg_entry_price);

                // Isolated Position Equity Check: Margin + Unrealized PnL
                let isolated_position_equity = position.margin + position.unrealized_pnl;
                let maintenance_margin_requirement = position.qty * mark_price * MMR_RATIO;

                if isolated_position_equity < maintenance_margin_requirement {
                    liquidated_users.push(user_id);
                }
            }
        }

        // Process Liquidations: Evict insolvent positions from the engine's memory maps
        for user_id in liquidated_users {
            if let Some(positions_map) = self.user_positions.get_mut(&user_id) {
                if let Some(abandoned_position) = positions_map.remove(&market) {
                    println!(
                        "🚨 [LIQUIDATION ENGINE] Position wiped for User {} on {:?} @ Mark ${}",
                        user_id, market, mark_price
                    );
                    println!(
                        "   Wiped Isolated Margin: ${} | Final Realized Loss: ${}",
                        abandoned_position.margin, abandoned_position.unrealized_pnl
                    );

                    // Public liquidation broadcast over Redis channels
                    let log_alert = serde_json::json!({
                        "user_id": user_id, "market": market, "wiped_margin": abandoned_position.margin.to_string(), "execution_mark": mark_price.to_string()
                    });
                    let _ = self
                        .redis
                        .publish_market_update(market, "liquidations", &log_alert.to_string())
                        .await;
                }
            }
        }
    }

    fn median_of_three(&self, mut a: Decimal, mut b: Decimal, mut c: Decimal) -> Decimal {
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if b > c {
            std::mem::swap(&mut b, &mut c);
        }
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        b
    }

    pub async fn sample_premium_indices(&mut self) {
        let markets = [Market::SOL_PERP, Market::BTC_PERP, Market::ETH_PERP];
        let impact_notional = dec!(1000.00);

        for market in markets {
            let index_price = *self.index_prices.get(&market).unwrap_or(&dec!(1.0));
            let book = self.orderbooks.get(&market).unwrap();

            let bid_impact = book
                .calculate_bid_impact(impact_notional)
                .unwrap_or(index_price);
            let ask_impact = book
                .calculate_ask_impact(impact_notional)
                .unwrap_or(index_price);

            let term_1 = (bid_impact - index_price).max(Decimal::ZERO);
            let term_2 = (index_price - ask_impact).max(Decimal::ZERO);
            let ipd = term_1 - term_2;
            let premium_index = ipd / index_price;

            let samples = self.premium_samples.get_mut(&market).unwrap();
            samples.push(premium_index);

            if samples.len() >= 12 {
                self.settle_hourly_funding_window(market).await;
            } else {
                let telemetry = FundingTelemetry {
                    market,
                    index_price,
                    premium_index,
                    current_hourly_rate: *self
                        .current_funding_rates
                        .get(&market)
                        .unwrap_or(&Decimal::ZERO),
                };
                let telemetry_json = serde_json::to_string(&telemetry).unwrap_or_default();
                let _ = self
                    .redis
                    .publish_market_update(market, "funding", &telemetry_json)
                    .await;
            }
        }
    }

    pub async fn settle_hourly_funding_window(&mut self, market: Market) {
        let samples = self.premium_samples.get_mut(&market).unwrap();
        if samples.is_empty() {
            return;
        }

        let sum: Decimal = samples.iter().sum();
        let mean_p = sum / Decimal::from(samples.len());
        samples.clear();

        let interest_leg = dec!(0.0001);
        let interest_deviation = interest_leg - mean_p;
        let clamped_interest = interest_deviation.clamp(dec!(-0.0005), dec!(0.0005));
        let f_8h = mean_p + clamped_interest;
        let fr_hour = (f_8h / dec!(8.0)).clamp(dec!(-0.04), dec!(0.04));
        self.current_funding_rates.insert(market, fr_hour);

        let index_price = *self.index_prices.get(&market).unwrap_or(&dec!(1.0));

        // Funding payments apply directly to each position's isolated margin account
        for (user_id, positions) in &mut self.user_positions {
            if let Some(position) = positions.get_mut(&market) {
                if position.size.is_zero() {
                    continue;
                }

                let funding_payment = position.size * fr_hour * index_price;
                position.margin -= funding_payment;
                println!(
                    "💸 [ISOLATED FUNDING DEBIT] Subtracted ${} from User {}'s isolated margin balance container",
                    funding_payment, user_id
                );
            }
        }
    }
}
