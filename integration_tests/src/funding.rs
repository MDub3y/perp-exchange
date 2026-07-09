use engine::ExecuteEngine;
use redis::RedisManager;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use utils::{Market, UserBalance};
use uuid::Uuid;

#[tokio::test]
async fn test_high_precision_funding_payment_settlement() {
    dotenvy::from_filename("../.env").unwrap();
    let redis_manager = RedisManager::new()
        .await
        .expect("Redis infrastructure down");

    let mut engine = ExecuteEngine::new(redis_manager).await;
    let market = Market::SOL_PERP;

    let user_long = Uuid::new_v4();
    let user_short = Uuid::new_v4();

    // 1. Manually seed baseline margin ledger entries inside engine structures
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

    // 2. Simulate crossing matched positions (Long 50 SOL vs Short 50 SOL)
    let long_map = engine.user_positions.entry(user_long).or_default();
    long_map.insert(market, dec!(50.0)); // Long 50.0 base asset contracts

    let short_map = engine.user_positions.entry(user_short).or_default();
    short_map.insert(market, dec!(-50.0)); // Short -50.0 base asset contracts

    // 3. Inject mock historical premium index samples to simulate a highly premium market state
    let samples = engine.premium_samples.get_mut(&market).unwrap();
    for _ in 0..12 {
        samples.push(dec!(0.02)); // Market trades at a premium over index
    }

    // 4. Force intermediate hour block execution pass
    engine.settle_hourly_funding_window(market).await;

    let final_rate = *engine.current_funding_rates.get(&market).unwrap();

    // Base rate math verification:
    // mean_p = 0.02
    // interest_deviation = 0.0001 - 0.02 = -0.0199 -> clamped to -0.0005
    // f_8h = 0.02 - 0.0005 = 0.0195
    // fr_hour = 0.0195 / 8 = 0.0024375 (0.24375%)
    assert_eq!(final_rate, dec!(0.0024375));

    // Payment check:
    // Position Size (50) * FR_hour (0.0024375) * Index Price ($145.00) = $17.671875
    let expected_payment = dec!(17.671875);

    let long_wallet = engine.user_wallets.get(&user_long).unwrap();
    let short_wallet = engine.user_wallets.get(&user_short).unwrap();

    // Long  pays short
    assert_eq!(
        long_wallet.available_balance,
        dec!(1000.00) - expected_payment
    );
    assert_eq!(
        short_wallet.available_balance,
        dec!(1000.00) + expected_payment
    );

    println!("✅ funding premium rate calculation verified to the final decimal.");
}
