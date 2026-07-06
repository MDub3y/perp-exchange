use fred::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    BUY,
    SELL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Market {
    SOL_PERP,
    BTC_PERP,
    ETH_PERP,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    LIMIT,
    MARKET,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrder {
    pub market: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: OrderSide,
    pub user_id: String,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrder {
    pub order_id: String,
    pub user_id: String,
    pub price: Decimal,
    pub side: OrderSide,
    pub market: String,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAllOrders {
    pub user_id: String,
    pub market: String,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOpenOrders {
    pub user_id: String,
    pub market: String,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum OrderRequests {
    CreateOrder(CreateOrder),
    CancelOrder(CancelOrder),
    CancelAllOrders(CancelAllOrders),
    GetOpenOrders(GetOpenOrders),
}

#[derive(Clone)]
pub struct RedisManager {
    pub client: Client,
}

impl RedisManager {
    pub async fn new() -> Result<Self, Error> {
        let redis_url =
            env::var("REDIS_CLIENT").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
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
            Error::new(ErrorKind::Parse, "Failed to serialize order request matrix")
        })?;

        let _: String = self
            .client
            .xadd(stream, false, None, "*", ("data", serialized))
            .await?;
        Ok(())
    }
}
