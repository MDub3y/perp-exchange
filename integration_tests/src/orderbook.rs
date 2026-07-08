use rust_decimal_macros::dec;
use uuid::Uuid;

use engine::trade::orderbook::Orderbook;
use utils::{Market, Order, OrderSide, OrderType};

#[tokio::test]
async fn test_clob_core_fifo_execution_and_partial_fills() {
    let mut book = Orderbook::new(Market::SOL_PERP);
    let maker_1 = Uuid::new_v4();
    let maker_2 = Uuid::new_v4();
    let taker = Uuid::new_v4();

    let client_order_1 = Uuid::new_v4();
    let client_order_2 = Uuid::new_v4();

    println!("[TEST] Seeding book with resting liquidity layers...");
    let sell_1 = Order {
        user_id: maker_1,
        order_id: client_order_1,
        price: dec!(145.00),
        quantity: dec!(10.0),
    };
    let sell_2 = Order {
        user_id: maker_2,
        order_id: client_order_2,
        price: dec!(145.00),
        quantity: dec!(5.0),
    };

    book.process_order(sell_1, OrderSide::SELL, OrderType::LIMIT)
        .unwrap();
    book.process_order(sell_2, OrderSide::SELL, OrderType::LIMIT)
        .unwrap();

    let initial_depth = book.get_depth();
    assert_eq!(initial_depth.asks[0], (dec!(145.00), dec!(15.0)));

    println!("[TEST] Ingesting aggressive Taker Market order...");

    let taker_order = Order {
        user_id: taker,
        order_id: Uuid::new_v4(),
        price: dec!(0.0),
        quantity: dec!(12.5),
    };
    let match_summary = book
        .process_order(taker_order, OrderSide::BUY, OrderType::MARKET)
        .unwrap();

    assert_eq!(match_summary.executed_quantity, dec!(12.5));
    assert_eq!(match_summary.fills.len(), 2);

    assert_eq!(match_summary.fills[0].maker_order_id, client_order_1);
    assert_eq!(match_summary.fills[0].quantity, dec!(10.0));

    assert_eq!(match_summary.fills[1].maker_order_id, client_order_2);
    assert_eq!(match_summary.fills[1].quantity, dec!(2.5));

    let post_match_depth = book.get_depth();
    assert_eq!(post_match_depth.asks[0], (dec!(145.00), dec!(2.5)));
}
