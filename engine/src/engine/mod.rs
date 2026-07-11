use rust_decimal::Decimal;
use std::cmp::Reverse;
use std::collections::HashMap;
use uuid::Uuid;

use crate::trade::orderbook::Orderbook;
use redis::RedisManager;
use utils::{Market, Position, UserBalance};

pub struct ExecuteEngine {
    pub orderbooks: HashMap<Market, Orderbook>,
    pub user_wallets: HashMap<Uuid, UserBalance>,
    pub user_positions: HashMap<Uuid, HashMap<Market, Position>>,
    pub index_prices: HashMap<Market, Decimal>,
    pub external_marks: HashMap<Market, Decimal>,
    pub last_trade_prices: HashMap<Market, Decimal>,
    pub c1_ema_state: HashMap<Market, Decimal>,
    pub mark_prices: HashMap<Market, Decimal>,
    pub premium_samples: HashMap<Market, Vec<Decimal>>,
    pub current_funding_rates: HashMap<Market, Decimal>,
    pub redis: RedisManager,
}

impl ExecuteEngine {
    pub async fn new(redis: RedisManager) -> Self {
        let mut orderbooks = HashMap::new();
        let mut index_prices = HashMap::new();
        let mut external_marks = HashMap::new();
        let mut last_trade_prices = HashMap::new();
        let mut c1_ema_state = HashMap::new();
        let mut mark_prices = HashMap::new();
        let mut premium_samples = HashMap::new();
        let mut current_funding_rates = HashMap::new();

        for &market in &[Market::SOL_PERP, Market::BTC_PERP, Market::ETH_PERP] {
            orderbooks.insert(market, Orderbook::new(market));
            premium_samples.insert(market, Vec::new());
            current_funding_rates.insert(market, Decimal::ZERO);
            c1_ema_state.insert(market, Decimal::ZERO);

            index_prices.insert(market, rust_decimal_macros::dec!(145.00));
            external_marks.insert(market, rust_decimal_macros::dec!(145.00));
            last_trade_prices.insert(market, rust_decimal_macros::dec!(145.00));
            mark_prices.insert(market, rust_decimal_macros::dec!(145.00));
        }

        let mut instance = Self {
            orderbooks,
            user_wallets: HashMap::new(),
            user_positions: HashMap::new(),
            index_prices,
            external_marks,
            last_trade_prices,
            c1_ema_state,
            mark_prices,
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
                available_balance: rust_decimal_macros::dec!(100000.00),
                locked_balance: Decimal::ZERO,
            },
        );
        self.user_wallets.insert(
            user_2,
            UserBalance {
                available_balance: rust_decimal_macros::dec!(50000.00),
                locked_balance: Decimal::ZERO,
            },
        );
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

pub mod order_handler;
pub mod polling;
pub mod valuation;
