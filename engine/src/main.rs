use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{cmp::Reverse, collections::HashMap};
use uuid::Uuid;

use crate::trade::*;

mod trade;

pub struct ExecuteEngine {
    pub orderbooks: HashMap<Market, Orderbook>,
}

impl ExecuteEngine {
    pub fn new() -> Self {
        let mut orderbooks = HashMap::new();

        orderbooks.insert(Market::SOL_PERP, Orderbook::new(Market::SOL_PERP));
        orderbooks.insert(Market::BTC_PERP, Orderbook::new(Market::BTC_PERP));
        orderbooks.insert(Market::ETH_PERP, Orderbook::new(Market::ETH_PERP));

        Self { orderbooks }
    }

    pub fn execute_transaction(&mut self, payload: OrderPayload) {
        let market_target = &payload.market;

        if let Some(book) = self.orderbooks.get_mut(&market_target) {
            println!(
                "[ENGINE] Route order {} to market {:?}",
                payload.order.order_id, market_target
            );

            match book.process_order(payload) {
                Ok(result) => {
                    println!(
                        "[ENGINE EXECUTION COMPLETE] Cleared {} units",
                        result.executed_quantity
                    );
                    for fill in result.fills {
                        println!(
                            "  MATCHED: {} units @ ${} [Taker: {} <-> Maker: {}]",
                            fill.quantity, fill.price, fill.taker_user_id, fill.maker_user_id
                        );
                    }
                }
                Err(err) => eprintln!("[ENGINE ERROR] Transaciton processing failed: {}", err),
            }
        } else {
            eprintln!("[ENGINE WARN] Dropped order targeting uninitiailzed market vactor");
        }
    }

    pub fn print_book_matrix(&self, market: Market) {
        if let Some(book) = self.orderbooks.get(&market) {
            println!("\n=================================================");
            println!("📊 MARKET MATRIX DEPTH VIEW: {:?}", market);
            println!("=================================================");

            println!("🔴 ASKS (SELL SIDE)");
            if book.get_asks().is_empty() {
                println!("   [ Empty Tree ]");
            } else {
                for (price, queue) in book.get_asks().iter().rev() {
                    let aggregate_volume: Decimal = queue.iter().map(|o| o.quantity).sum();
                    let order_count = queue.len();
                    println!(
                        "   Price: ${:<10} | Vol: {:<10} | Orders: {}",
                        price, aggregate_volume, order_count
                    );
                }
            }

            println!("-------------------------------------------------");
            println!("🟢 BIDS (BUY SIDE)");
            if book.get_bids().is_empty() {
                println!("   [ Empty Tree ]");
            } else {
                for (Reverse(price), queue) in book.get_bids().iter() {
                    let aggregate_volume: Decimal = queue.iter().map(|o| o.quantity).sum();
                    let order_count = queue.len();
                    println!(
                        "   Price: ${:<10} | Vol: {:<10} | Orders: {}",
                        price, aggregate_volume, order_count
                    );
                }
            }
            println!("=================================================\n");
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Perpetual Matching Engine core running...");

    let mut engine = ExecuteEngine::new();

    let maker_1 = Uuid::new_v4();
    let maker_2 = Uuid::new_v4();
    let taker = Uuid::new_v4();

    println!("\n--- Step 0: Printing Initial State ---");
    engine.print_book_matrix(Market::SOL_PERP);

    println!("--- Step 1: Placing resting Maker Limit Sell Order at $145.50 ---");
    let limit_sell_1 = OrderPayload::new(
        maker_1,
        Market::SOL_PERP,
        dec!(145.50),
        dec!(10.0),
        OrderSide::SELL,
        OrderType::LIMIT,
    );
    engine.execute_transaction(limit_sell_1);

    println!("\n--- Step 1 ---");
    engine.print_book_matrix(Market::SOL_PERP);

    println!("--- Step 2: Placing higher resting Maker Limit Sell Order at $146.00 ---");
    let limit_sell_2 = OrderPayload::new(
        maker_2,
        Market::SOL_PERP,
        dec!(146.00),
        dec!(5.5),
        OrderSide::SELL,
        OrderType::LIMIT,
    );
    engine.execute_transaction(limit_sell_2);

    println!("\n--- Step 2 ---");
    engine.print_book_matrix(Market::SOL_PERP);

    println!("--- Step 3: Ingesting Taker Market Buy Order for 12.0 Units ---");
    let market_buy = OrderPayload::new(
        taker,
        Market::SOL_PERP,
        dec!(0.0), // Market orders bypass limit boundaries
        dec!(12.0),
        OrderSide::BUY,
        OrderType::MARKET,
    );
    engine.execute_transaction(market_buy);

    println!("\n--- Step 3 ---");
    engine.print_book_matrix(Market::SOL_PERP);
}
