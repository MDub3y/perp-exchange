use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{env, time::Duration};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use engine::ExecuteEngine;
use fred::prelude::*;
use fred::types::Value;
use redis::RedisManager;
use utils::{
    CreateOrderArgs, Market, OrderBookDepth, OrderRequests, OrderSide, OrderType,
    ProcessOrderResult,
};

const INGESTION_STREAM: &str = "exchange:ingestion:stream";
const PERSISTENCE_STREAM: &str = "exchange:persistence:stream";

#[tokio::test]
async fn test_end_to_end_pipeline_flow() {
    dotenvy::from_filename("../.env").ok();
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let redis_manager = RedisManager::new()
        .await
        .expect("Failed to connect to redis!");

    let _: () = redis_manager.client.del(INGESTION_STREAM).await.unwrap();
    let _: () = redis_manager.client.del(PERSISTENCE_STREAM).await.unwrap();

    let mut engine = ExecuteEngine::new(redis_manager.clone()).await;

    tokio::spawn(async move {
        engine.start_polling_loop().await;
    });
    println!("Engine loop spawned");

    let config = Config::from_url(&redis_url).unwrap();
    let client_ws_subscriber = Builder::from_config(config).build().unwrap();
    client_ws_subscriber.init().await.unwrap();

    let depth_channel = "market:SOL_PERP:depth";
    client_ws_subscriber.subscribe(depth_channel).await.unwrap();
    let mut depth_pubsub_stream = client_ws_subscriber.message_rx();
    println!(
        "Websocket subscriber linked to Pub/Sub channel: {}",
        depth_channel
    );

    let user_maker = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let user_taker = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

    println!("\n[STAGE 2: SEEDING RESTING LIMIT LIQUIDITY]");

    let order_id_a = Uuid::new_v4();
    let ask_payload_1 = OrderRequests::CreateOrder(CreateOrderArgs {
        order_id: order_id_a,
        market: Market::SOL_PERP,
        price: dec!(145.00),
        quantity: dec!(10.0),
        side: OrderSide::SELL,
        order_type: OrderType::LIMIT,
        user_id: user_maker,
        pubsub_id: Some(Uuid::new_v4()),
        leverage: dec!(10.0),
    });

    let order_id_b = Uuid::new_v4();
    let ask_payload_2 = OrderRequests::CreateOrder(CreateOrderArgs {
        order_id: order_id_b,
        market: Market::SOL_PERP,
        price: dec!(146.00),
        quantity: dec!(5.0),
        side: OrderSide::SELL,
        order_type: OrderType::LIMIT,
        user_id: user_maker,
        pubsub_id: Some(Uuid::new_v4()),
        leverage: dec!(10.0),
    });

    redis_manager
        .enqueue_request(INGESTION_STREAM, &ask_payload_1)
        .await
        .unwrap();
    redis_manager
        .enqueue_request(INGESTION_STREAM, &ask_payload_2)
        .await
        .unwrap();
    println!("Buffered 2 resting Asks into Ingestion Stream (Total Vol: 15.0 SOL)");

    sleep(Duration::from_millis(50)).await;

    println!("\n[STAGE 3: INGESTING AGGRESSIVE TAKER MARKET ORDER]");
    let order_id_taker = Uuid::new_v4();
    let taker_payload = OrderRequests::CreateOrder(CreateOrderArgs {
        order_id: order_id_taker,
        market: Market::SOL_PERP,
        price: Decimal::ZERO,
        quantity: dec!(12.5),
        side: OrderSide::BUY,
        order_type: OrderType::MARKET,
        user_id: user_taker,
        pubsub_id: Some(Uuid::new_v4()),
        leverage: dec!(10.0), // Fixed: Added required leverage field parameter
    });

    redis_manager
        .enqueue_request(INGESTION_STREAM, &taker_payload)
        .await
        .unwrap();
    println!("Aggressive Market Buy Order for 12.5 SOL sent to execution spine.");

    println!("\n[STAGE 4: DEEP PIPELINE VALIDATION]");

    println!("Verifying Pub/Sub L2 orderbook updates...");
    let mut final_depth: Option<OrderBookDepth> = None;

    while let Ok(Ok(msg)) = timeout(Duration::from_millis(100), depth_pubsub_stream.recv()).await {
        let raw_payload: String = msg.value.convert().unwrap();
        if let Ok(parsed_depth) = serde_json::from_str::<OrderBookDepth>(&raw_payload) {
            final_depth = Some(parsed_depth);
        }
    }

    assert!(
        final_depth.is_some(),
        "Pub/Sub channel failed to broadcast updated depth matrices"
    );
    let depth = final_depth.unwrap();

    println!("  [Live Pub/Sub Captured Depth] Asks: {:?}", depth.asks);
    assert_eq!(depth.asks.len(), 1);
    assert_eq!(depth.asks[0], (dec!(146.00), dec!(2.5)));
    println!(
        "Programmatic Assert Pass: Orderbook levels matched cleanly according to FIFO price priority."
    );

    println!("Verifying Persistence Log Stream entries...");

    let persistence_data: Value = redis_manager
        .client
        .xread(Some(5), None, PERSISTENCE_STREAM, "0")
        .await
        .unwrap();

    assert!(
        !persistence_data.is_null(),
        "Persistence stream logs are empty or failed to save updates"
    );

    let mut verified_persistence_matches = false;
    if let Value::Array(streams) = persistence_data {
        if let Some(Value::Array(stream_wrapper)) = streams.get(0) {
            if let Some(Value::Array(entries)) = stream_wrapper.get(1) {
                for entry in entries {
                    if let Value::Array(entry_fields) = entry {
                        if let Some(Value::Array(fields)) = entry_fields.get(1) {
                            if let Some(raw_json) = fields.get(1).and_then(|v| v.as_string()) {
                                if let Ok(result) =
                                    serde_json::from_str::<ProcessOrderResult>(&raw_json)
                                {
                                    if result.executed_quantity == dec!(12.5) {
                                        println!(
                                            "  [Durable Stream Log Capture] Executed: {} units across {} fills",
                                            result.executed_quantity,
                                            result.fills.len()
                                        );
                                        assert_eq!(result.fills.len(), 2);
                                        assert_eq!(result.fills[0].maker_order_id, order_id_a);
                                        assert_eq!(result.fills[1].maker_order_id, order_id_b);
                                        verified_persistence_matches = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(
            verified_persistence_matches,
            "Matching execution entries not found in durability streams"
        );
        println!(
            "Programmatic Assert Pass: Data backup pipelines are verified and running error-free!"
        );
        println!("\n[TEST SUMMARY: ALL LIFECYCLE CHANNELS VERIFIED GREEN]\n");
    }
}
