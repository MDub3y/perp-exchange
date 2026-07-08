use rust_decimal::Decimal;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

mod trade;
use trade::orderbook::Orderbook;

use redis::RedisManager;
use utils::{
    Fill, Market, Order, OrderRequests, OrderSide, OrderType, ProcessOrderResult, UserBalance,
};

const INGESTION_STREAM: &str = "exchange:ingestion:stream";
const PERSISTENCE_STREAM: &str = "exchange:persistence:stream";
const CONSUMER_GROUP: &str = "engine:matching:group";
const ENGINE_IDENTITY: &str = "matching_engine_primary_node";

pub struct ExecuteEngine {
    pub orderbooks: HashMap<Market, Orderbook>,
    pub user_wallets: HashMap<Uuid, UserBalance>,
    pub redis: RedisManager,
}

impl ExecuteEngine {
    pub async fn new(redis: RedisManager) -> Self {
        let mut orderbooks = HashMap::new();
        orderbooks.insert(Market::SOL_PERP, Orderbook::new(Market::SOL_PERP));
        orderbooks.insert(Market::BTC_PERP, Orderbook::new(Market::BTC_PERP));
        orderbooks.insert(Market::ETH_PERP, Orderbook::new(Market::ETH_PERP));

        redis
            .setup_consumer_group(INGESTION_STREAM, CONSUMER_GROUP)
            .await;

        let mut instance = Self {
            orderbooks,
            user_wallets: HashMap::new(),
            redis,
        };

        instance.seed_sandbox_balances();
        instance
    }

    fn seed_sandbox_balances(&mut self) {
        // Seed predictable demo keys matching test scripts
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

            // Settle Maker Balance (Makers always have funds locked inside a resting LIMIT order)
            if let Some(maker_wallet) = self.user_wallets.get_mut(&fill.maker_user_id) {
                maker_wallet.locked_balance -= matched_value;
                // Add matched perp value adjustments to available capital
                maker_wallet.available_balance += matched_value;
            }

            // Settle Taker Balance
            if let Some(taker_wallet) = self.user_wallets.get_mut(&fill.taker_user_id) {
                match taker_type {
                    OrderType::LIMIT => {
                        // Taker already locked funds based on initial liability limits.
                        // Decrease locked balance and transfer value down to available capital.
                        taker_wallet.locked_balance -= matched_value;
                        taker_wallet.available_balance += matched_value;
                    }
                    OrderType::MARKET => {
                        // Market orders don't lock funds in advance; settle directly from available capital
                        taker_wallet.available_balance -= matched_value;
                    }
                }
            }
        }
    }

    pub fn print_book_matrix(&self, market: Market) {
        if let Some(book) = self.orderbooks.get(&market) {
            println!("\n=================================================");
            println!("📊 ORDERBOOK L2 DEPTH SNAPSHOT: {:?}", market);
            println!("=================================================");
            println!("🔴 ASKS (SELL SIDE)");
            for (price, queue) in book.get_asks().iter().rev() {
                println!(
                    "   Price: ${:<10} | Vol: {}",
                    price,
                    queue.iter().map(|o| o.quantity).sum::<Decimal>()
                );
            }
            println!("-------------------------------------------------");
            println!("🟢 BIDS (BUY SIDE)");
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

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let redis_manager = RedisManager::new()
        .await
        .expect("Failed to initialize engine message spine");
    let mut engine = ExecuteEngine::new(redis_manager).await;
    engine.start_polling_loop().await;
}
