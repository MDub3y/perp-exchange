use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

pub mod trade;
use trade::orderbook::Orderbook;

use redis::RedisManager;
use utils::{
    Fill, FundingTelemetry, Market, Order, OrderRequests, OrderSide, OrderType, UserBalance,
};

const INGESTION_STREAM: &str = "exchange:ingestion:stream";
const PERSISTENCE_STREAM: &str = "exchange:persistence:stream";
const CONSUMER_GROUP: &str = "engine:matching:group";
const ENGINE_IDENTITY: &str = "matching_engine_primary_node";

pub struct ExecuteEngine {
    pub orderbooks: HashMap<Market, Orderbook>,
    pub user_wallets: HashMap<Uuid, UserBalance>,
    pub user_positions: HashMap<Uuid, HashMap<Market, Decimal>>,
    pub index_prices: HashMap<Market, Decimal>,
    pub premium_samples: HashMap<Market, Vec<Decimal>>,
    pub current_funding_rates: HashMap<Market, Decimal>,
    pub redis: RedisManager,
}

impl ExecuteEngine {
    pub async fn new(redis: RedisManager) -> Self {
        let mut orderbooks = HashMap::new();
        let mut index_prices = HashMap::new();
        let mut premium_samples = HashMap::new();
        let mut current_funding_rates = HashMap::new();

        for &market in &[Market::SOL_PERP, Market::BTC_PERP, Market::ETH_PERP] {
            orderbooks.insert(market, Orderbook::new(market));
            premium_samples.insert(market, Vec::new());
            current_funding_rates.insert(market, Decimal::ZERO);
        }

        redis
            .setup_consumer_group(INGESTION_STREAM, CONSUMER_GROUP)
            .await;

        let mut instance = Self {
            orderbooks,
            user_wallets: HashMap::new(),
            user_positions: HashMap::new(),
            index_prices,
            premium_samples,
            current_funding_rates,
            redis,
        };

        instance.seed_sandbox_balances();
        instance
    }

    fn seed_sandbox_balances(&mut self) {
        let user_1 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let user_2 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        self.user_wallets.insert(
            user_1,
            UserBalance {
                available_balance: Decimal::new(100000, 0),
                locked_balance: Decimal::ZERO,
            },
        );
        self.user_wallets.insert(
            user_2,
            UserBalance {
                available_balance: Decimal::new(50000, 0),
                locked_balance: Decimal::ZERO,
            },
        );
    }

    pub async fn start_polling_loop(&mut self) {
        println!("Engine active and streaming transactions...");

        let premium_redis = self.redis.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;

                let update_tick = OrderRequests::IndexUpdate(utils::IndexPriceUpdate {
                    market: Market::SOL_PERP,
                    price: Decimal::ZERO,
                });
                let _ = premium_redis
                    .enqueue_request(INGESTION_STREAM, &update_tick)
                    .await;
            }
        });

        loop {
            match self
                .redis
                .fetch_next_delivery(INGESTION_STREAM, CONSUMER_GROUP, ENGINE_IDENTITY)
                .await
            {
                Ok(Some((delivery_id, raw_json))) => {
                    if let Ok(request) = serde_json::from_str::<OrderRequests>(&raw_json) {
                        self.handle_order_request(request).await;
                    }
                    let _ = self
                        .redis
                        .acknowledge_processed(INGESTION_STREAM, CONSUMER_GROUP, &delivery_id)
                        .await;
                }
                Ok(None) => {
                    sleep(Duration::from_millis(1)).await;
                }
                Err(e) => {
                    eprintln!("Stream extraction failuee: {:?}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn handle_order_request(&mut self, request: OrderRequests) {
        match request {
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

            // IPD = max(bid_impact - Index, 0) - max(Index - ask_impact, 0)
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

        println!(
            "[FUNDING CALCULATOR] Market {:?} hourly rate locked at: {}%",
            market,
            fr_hour * dec!(100)
        );

        let index_price = *self.index_prices.get(&market).unwrap_or(&dec!(1.0));

        for (user_id, positions) in &self.user_positions {
            if let Some(&position_size) = positions.get(&market) {
                if position_size.is_zero() {
                    continue;
                }

                let funding_payment = position_size * fr_hour * index_price;

                if let Some(wallet) = self.user_wallets.get_mut(user_id) {
                    wallet.available_balance -= funding_payment;
                    println!(
                        "[FUNDING SETTLEMENT] Routed payment of ${} to/from User {}",
                        funding_payment, user_id
                    );
                }
            }
        }
    }

    fn settle_balances(
        &mut self,
        fills: &[Fill],
        taker_side: OrderSide,
        taker_type: OrderType,
        initial_liability: Decimal,
    ) {
        for fill in fills {
            let matched_value = fill.price * fill.quantity;

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
        }
    }

    pub fn print_book_matrix(&self, market: Market) {
        if let Some(book) = self.orderbooks.get(&market) {
            println!("\n=================================================");
            println!("ORDERBOOK L2 DEPTH SNAPSHOT: {:?}", market);
            println!("=================================================");
            println!("ASKS (SELL SIDE)");
            for (price, queue) in book.get_asks().iter().rev() {
                println!(
                    "   Price: ${:<10} | Vol: {}",
                    price,
                    queue.iter().map(|o| o.quantity).sum::<Decimal>()
                );
            }
            println!("-------------------------------------------------");
            println!("BIDS (BUY SIDE)");
            for (Reverse(price), queue) in book.get_bids().iter() {
                println!(
                    "   Price: ${:<10} | Vol: {}",
                    price,
                    queue.iter().map(|o| o.quantity).sum::<Decimal>()
                );
            }
            println!("=================================================\n");
        }
    }
}
