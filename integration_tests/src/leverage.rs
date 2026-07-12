use engine::ExecuteEngine;
use redis::RedisManager;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use utils::{CreateOrderArgs, Market, OrderRequests, OrderSide, OrderType, PositionSide};
use uuid::Uuid;

#[tokio::test]
async fn test_leverage_margin_and_liquidation_scaling() {
    dotenvy::dotenv().ok();
    let redis_manager = RedisManager::new().await.expect("Redis offline");
    let mut engine = ExecuteEngine::new(redis_manager).await;
    let market = Market::SOL_PERP;
    let trader = Uuid::new_v4();

    engine.user_wallets.insert(
        trader,
        utils::UserBalance {
            available_balance: dec!(1000.00),
            locked_balance: Decimal::ZERO,
        },
    );
    engine.index_prices.insert(market, dec!(100.00));

    // Scenario A: Open 10 SOL Long Position using 10x Leverage
    // Expected Isolated Margin = (10 * 100) / 10 = $100.00
    let long_args_10x = CreateOrderArgs {
        order_id: Uuid::new_v4(),
        market,
        price: dec!(100.00),
        quantity: dec!(10.0),
        side: OrderSide::BUY,
        order_type: OrderType::LIMIT,
        user_id: trader,
        pubsub_id: None,
        leverage: dec!(10.0), // 10x Leverage select
    };
    engine
        .handle_order_request(OrderRequests::CreateOrder(long_args_10x))
        .await;

    let position = engine
        .user_positions
        .get(&trader)
        .unwrap()
        .get(&market)
        .unwrap();
    assert_eq!(position.margin, dec!(100.00));
    assert_eq!(position.leverage, dec!(10.0));
    assert_eq!(position.side, PositionSide::Long);
    // Liq Price Math: (100 - (100/10)) / 0.99 = 90.90909
    assert_eq!(position.liquidation_price.round_dp(4), dec!(90.9091));

    println!("Dynamic leverage verification passed all math-bounded validation tracks.");
}
