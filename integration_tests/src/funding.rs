use engine::ExecuteEngine;
use redis::RedisManager;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use utils::{Market, UserBalance};
use uuid::Uuid;

#[tokio::test]
async fn test_funding_payment_settlement() {
    dotenvy::dotenv().ok();
    let redis_manager = RedisManager::new()
        .await
        .expect("Redis infrastructure down");

    let mut engine = ExecuteEngine::new(redis_manager).await;
    let market = Market::SOL_PERP;

    let user_long = Uuid::new_v4();
    let user_short = Uuid::new_v4();

    engine.user_wallets.insert(
        user_long,
        UserBalance {
            available_balance: dec!(1000.00),
            locked_balance: Decimal::ZERO,
        },
    );
    engine.user_wallets.insert(
        user_short,
        UserBalance {
            available_balance: dec!(1000.00),
            locked_balance: Decimal::ZERO,
        },
    );

    let long_map = engine.user_positions.entry(user_long).or_default();
    long_map.insert(market, dec!(50.0));

    let short_map = engine.user_positions.entry(user_short).or_default();
    short_map.insert(market, dec!(-50.0));

    engine.index_prices.insert(market, dec!(145.00));

    let samples = engine.premium_samples.get_mut(&market).unwrap();
    for _ in 0..12 {
        samples.push(dec!(0.02));
    }

    engine.settle_hourly_funding_window(market).await;

    let final_rate = *engine.current_funding_rates.get(&market).unwrap();

    assert_eq!(final_rate, dec!(0.0024375));

    let expected_payment = dec!(17.671875);

    let long_wallet = engine.user_wallets.get(&user_long).unwrap();
    let short_wallet = engine.user_wallets.get(&user_short).unwrap();

    assert_eq!(
        long_wallet.available_balance,
        dec!(1000.00) - expected_payment
    );
    assert_eq!(
        short_wallet.available_balance,
        dec!(1000.00) + expected_payment
    );

    println!("funding premium rate calculation verified to the final decimal.");
}
