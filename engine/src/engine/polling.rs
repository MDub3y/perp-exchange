use rust_decimal::Decimal;
use std::time::Duration;
use tokio::time::sleep;
use utils::OrderRequests;

use super::ExecuteEngine;

const INGESTION_STREAM: &str = "exchange:ingestion:stream";
const CONSUMER_GROUP: &str = "engine:matching:group";
const ENGINE_IDENTITY: &str = "matching_engine_primary_node";

impl ExecuteEngine {
    pub async fn start_polling_loop(&mut self) {
        self.redis
            .setup_consumer_group(INGESTION_STREAM, CONSUMER_GROUP)
            .await;
        println!("Engine active and streaming transactions...");

        let tick_redis = self.redis.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(200)).await;
                let _ = tick_redis
                    .enqueue_request(INGESTION_STREAM, &OrderRequests::MarkTick)
                    .await;
            }
        });

        let premium_redis = self.redis.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;
                let update_tick = OrderRequests::IndexUpdate(utils::IndexPriceUpdate {
                    market: utils::Market::SOL_PERP,
                    price: Decimal::ZERO,
                });
                let _ = premium_redis
                    .enqueue_request(INGESTION_STREAM, &update_tick)
                    .await;
            }
        });

        loop {
            match self
                .redis
                .fetch_next_delivery(INGESTION_STREAM, CONSUMER_GROUP, ENGINE_IDENTITY)
                .await
            {
                Ok(Some((delivery_id, raw_json))) => {
                    if let Ok(request) = serde_json::from_str::<OrderRequests>(&raw_json) {
                        // why is this async
                        self.handle_order_request(request).await;
                    }
                    let _ = self
                        .redis
                        .acknowledge_processed(INGESTION_STREAM, CONSUMER_GROUP, &delivery_id)
                        .await;
                }
                Ok(None) => {
                    sleep(Duration::from_millis(5)).await;
                }
                Err(e) => {
                    eprintln!("Stream extraction error: {:?}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
