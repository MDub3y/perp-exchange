use engine::ExecuteEngine;
use redis::RedisManager;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use utils::{CreateOrderArgs, Market, OrderRequests, OrderSide, OrderType};
use uuid::Uuid;

#[tokio::test]
async fn test_perpetual_full_lifecycle_flow() {
    dotenvy::dotenv().ok();
    let redis_manager = RedisManager::new().await.expect("Redis setup failed");
    let mut engine = ExecuteEngine::new(redis_manager).await;
    let market = Market::SOL_PERP;

    let user_maker = Uuid::new_v4();
    let user_trader = Uuid::new_v4();

    // 1. Initialize balances and baseline prices
    engine.user_wallets.insert(
        user_maker,
        utils::UserBalance {
            available_balance: dec!(2000.00),
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

    // 2. Open Position: Seed counterparty liquidity and fill trader's 20x Long position
    // Required Margin Cushion: (10 SOL * $100) / 20 = $50.00 isolated margin
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

    let trader_long = CreateOrderArgs {
        order_id: Uuid::new_v4(),
        market,
        price: dec!(100.00),
        quantity: dec!(10.0),
        side: OrderSide::BUY,
        order_type: OrderType::LIMIT,
        user_id: user_trader,
        pubsub_id: None,
        leverage: dec!(20.0), // 20x Max Leverage
    };
    engine
        .handle_order_request(OrderRequests::CreateOrder(trader_long))
        .await;

    // Verify initial liquidation threshold boundary point: (100 - (50/10)) / 0.99 = 95.95959
    let initial_pos = engine
        .user_positions
        .get(&user_trader)
        .unwrap()
        .get(&market)
        .unwrap();
    assert_eq!(initial_pos.margin, dec!(50.00));
    assert_eq!(initial_pos.liquidation_price.round_dp(4), dec!(95.9596));

    // 3. Process Funding Debit: Simulate an extreme premium to drain the isolated margin cushion
    let samples = engine.premium_samples.get_mut(&market).unwrap();
    for _ in 0..12 {
        samples.push(dec!(0.15));
    } // High premium deviation
    engine.settle_hourly_funding_window(market).await;

    // Check margin state after funding reduction
    let post_funding_pos = engine
        .user_positions
        .get(&user_trader)
        .unwrap()
        .get(&market)
        .unwrap();
    assert!(post_funding_pos.margin < dec!(50.00)); // Margin decreased due to funding debit

    // 4. Trigger Liquidation: Move spot price down to force position equity below the maintenance margin requirement
    engine.index_prices.insert(market, dec!(95.00));
    engine.external_marks.insert(market, dec!(95.00));

    // Execute a valuation tick to check health status and trigger the liquidation
    engine.calculate_all_market_mark_prices().await;

    // 5. Final Invariant Check: Confirm the insolvent position has been safely liquidated and removed
    let final_positions_map = engine.user_positions.get(&user_trader).unwrap();
    assert!(final_positions_map.get(&market).is_none());
    println!("💯 [SUCCESS]: Position lifecycle test passed cleanly from entry to liquidation.");
}
