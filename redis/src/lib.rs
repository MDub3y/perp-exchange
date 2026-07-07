use fred::prelude::*;
use fred::types::Value;
use std::env;
pub use utils::{Market, OrderRequests};

#[derive(Clone)]
pub struct RedisManager {
    pub client: Client,
}

impl RedisManager {
    pub async fn new() -> Result<Self, Error> {
        let redis_url =
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let config = Config::from_url(&redis_url)?;

        let client = Builder::from_config(config).build()?;
        client.init().await?;

        Ok(Self { client })
    }

    pub async fn enqueue_request(
        &self,
        stream: &str,
        request: &OrderRequests,
    ) -> Result<(), Error> {
        let serialized = serde_json::to_string(request).map_err(|_| {
            Error::new(
                ErrorKind::Parse,
                "Failed to serialize order request layout matrix",
            )
        })?;

        let _: String = self
            .client
            .xadd(stream, false, None, "*", ("data", serialized))
            .await?;
        Ok(())
    }

    pub async fn setup_consumer_group(&self, stream: &str, group: &str) {
        let _: Result<(), Error> = self.client.xgroup_create(stream, group, "0", true).await;
    }

    pub async fn fetch_next_delivery(
        &self,
        stream: &str,
        group: &str,
        consumer: &str,
    ) -> Result<Option<(String, String)>, Error> {
        let response: Value = self
            .client
            .xreadgroup(group, consumer, Some(1), None, false, ">", stream)
            .await?;

        if response.is_null() {
            return Ok(None);
        }

        if let Value::Array(stream_list) = response {
            if let Some(Value::Array(stream_entry)) = stream_list.get(0) {
                if let Some(Value::Array(entries)) = stream_entry.get(1) {
                    if let Some(Value::Array(entry)) = entries.get(0) {
                        let id = entry.get(0).and_then(|v| v.as_string()).unwrap_or_default();
                        if let Some(Value::Array(fields)) = entry.get(1) {
                            if let Some(raw_json) = fields.get(1).and_then(|v| v.as_string()) {
                                return Ok(Some((id, raw_json)));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    pub async fn acknowledge_processed(
        &self,
        stream: &str,
        group: &str,
        id: &str,
    ) -> Result<(), Error> {
        let _: i64 = self.client.xack(stream, group, id).await?;
        Ok(())
    }

    pub async fn publish_market_update(
        &self,
        market: Market,
        feed_type: &str,
        payload: &str,
    ) -> Result<(), Error> {
        let channel = format!("market:{:?}:{}", market, feed_type);
        let _: () = self.client.publish(channel, payload).await?;
        Ok(())
    }

    pub async fn publish_user_update(&self, user_id: &str, payload: &str) -> Result<(), Error> {
        let channel = format!("user:{}:updates", user_id);
        let _: () = self.client.publish(channel, payload).await?;
        Ok(())
    }
}
