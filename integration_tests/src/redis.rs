use rust_decimal_macros::dec;
use std::{env, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

use fred::prelude::*;
use redis::RedisManager;
use utils::{CreateOrderArgs, Market, OrderRequests, OrderType};

const TEST_INGESTION_STREAM: &str = "test:exchange:ingestion:stream";
const TEST_CONSUMER_GROUP: &str = "test:engine:matching:group";
const TEST_ENGINE_IDENTITY: &str = "test_matching_engine_worker_node";

#[tokio::test]
async fn test_redis_stream_ingress_egress_and_acknowledgemant() {
    dotenvy::dotenv().ok();
    let _redis_url = env::var("REDIS_URL").unwrap();
    let redis_manager = RedisManager::new()
        .await
        .expect("Redis test network cluster unreachable");

    let _: () = redis_manager
        .client
        .del(TEST_INGESTION_STREAM)
        .await
        .unwrap();

    let client_order_id = Uuid::new_v4();
    let client_user_id = Uuid::new_v4();

    let inbound_request = OrderRequests::CreateOrder(CreateOrderArgs {
        order_id: client_order_id,
        user_id: client_user_id,
        market: utils::Market::ETH_PERP,
        price: dec!(2600.00),
        quantity: dec!(1.5),
        side: utils::OrderSide::BUY,
        order_type: OrderType::LIMIT,
        pubsub_id: Some(Uuid::new_v4()),
    });

    println!("[TEST] Enqueuing verified payload into append-only log stream");
    redis_manager
        .enqueue_request(TEST_INGESTION_STREAM, &inbound_request)
        .await
        .unwrap();

    println!("[TEST] Setting up isolated consumer group boundary zones...");
    let _: Result<(), _> = redis_manager
        .client
        .xgroup_create(TEST_INGESTION_STREAM, TEST_CONSUMER_GROUP, "0", true)
        .await;

    println!("[TEST] Polling delivery block off stream group offset offsets...");
    let extraction = redis_manager
        .fetch_next_delivery(
            TEST_INGESTION_STREAM,
            TEST_CONSUMER_GROUP,
            TEST_ENGINE_IDENTITY,
        )
        .await
        .unwrap();

    assert!(extraction.is_some());
    let (delivery_id, raw_json) = extraction.unwrap();

    let parsed_request: OrderRequests = serde_json::from_str(&raw_json).unwrap();
    if let OrderRequests::CreateOrder(payload) = parsed_request {
        assert_eq!(payload.order_id, client_order_id);
        assert_eq!(payload.price, dec!(2600.00));
        assert_eq!(payload.quantity, dec!(1.5));
    } else {
        panic!("Extracted stream entry payload corrupted or typed incorrectly");
    }

    println!("[TEST] Clearing Pending Entries List via explicit XACK message token...");
    redis_manager
        .acknowledge_processed(TEST_INGESTION_STREAM, TEST_CONSUMER_GROUP, &delivery_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_engine_pubsub_broadcast_channels() {
    dotenvy::dotenv().ok();
    let _redis_url = env::var("REDIS_URL").unwrap();
    let redis_manager = RedisManager::new()
        .await
        .expect("Redis infrastructure link broken");

    let config = Config::from_url("redis://127.0.0.1:6379").unwrap();
    let subscriber_client = Builder::from_config(config).build().unwrap();
    subscriber_client.init().await.unwrap();

    let target_channel = "market:BTC_PERP:depth";
    subscriber_client.subscribe(target_channel).await.unwrap();
    let mut message_stream = subscriber_client.message_rx();

    println!("[TEST] Emitting broadcast telemetry out-of-band via engine coordinator...");
    let test_payload = "{\"bids\":[[65000.0,1.2]],\"asks\":[]}";
    redis_manager
        .publish_market_update(Market::BTC_PERP, "depth", test_payload)
        .await
        .unwrap();

    println!("[TEST] Listening for message propagation on subscriber stream barrier...");
    let message_catch = timeout(Duration::from_millis(500), message_stream.recv()).await;

    assert!(
        message_catch.is_ok(),
        "Pub/Sub layer timed out or failed to route message within sub-millisecond limits"
    );
    let received_message = message_catch.unwrap().unwrap();

    let processed_payload: String = received_message.value.convert().unwrap();
    assert_eq!(processed_payload, test_payload);
    assert_eq!(received_message.channel, target_channel);
}
