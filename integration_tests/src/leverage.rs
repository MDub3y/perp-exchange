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

    let user_maker = Uuid::new_v4();
    let user_trader = Uuid::new_v4();

    // 1. Seed baseline wallets
    engine.user_wallets.insert(
        user_maker,
        utils::UserBalance {
            available_balance: dec!(1000.00),
            locked_balance: Decimal::ZERO,
        },
    );
    engine.user_wallets.insert(
        user_trader,
        utils::UserBalance {
            available_balance: dec!(1000.00),
            locked_balance: Decimal::ZERO,
        },
    );
    engine.index_prices.insert(market, dec!(100.00));

    // 2. Seed resting counterparty Ask Liquidity at $100.00
    let counterparty_ask = CreateOrderArgs {
        order_id: Uuid::new_v4(),
        market,
        price: dec!(100.00),
        quantity: dec!(10.0),
        side: OrderSide::SELL,
        order_type: OrderType::LIMIT,
        user_id: user_maker,
        pubsub_id: None,
        leverage: dec!(1.0),
    };
    engine
        .handle_order_request(OrderRequests::CreateOrder(counterparty_ask))
        .await;

    // 3. Execute target trader Buy Limit order using 10x leverage
    let long_args_10x = CreateOrderArgs {
        order_id: Uuid::new_v4(),
        market,
        price: dec!(100.00),
        quantity: dec!(10.0),
        side: OrderSide::BUY,
        order_type: OrderType::LIMIT,
        user_id: user_trader,
        pubsub_id: None,
        leverage: dec!(10.0), // 10x Leverage selected
    };
    engine
        .handle_order_request(OrderRequests::CreateOrder(long_args_10x))
        .await;

    // 4. Assert against the isolated position account fields
    let position = engine
        .user_positions
        .get(&user_trader)
        .unwrap()
        .get(&market)
        .unwrap();
    assert_eq!(position.margin, dec!(100.00)); // (10 * $100) / 10 = $100.00
    assert_eq!(position.leverage, dec!(10.0));
    assert_eq!(position.side, PositionSide::Long);

    // Liquidation boundary math check: (100 - (100/10)) / 0.99 = 90.909090...
    assert_eq!(position.liquidation_price.round_dp(4), dec!(90.9091));
    println!("Dynamic leverage metrics verified successfully.");
}
