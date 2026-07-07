use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Market {
    SOL_PERP,
    BTC_PERP,
    ETH_PERP,
}

impl Market {
    pub fn from_str(market_str: &str) -> Result<Self, &'static str> {
        match market_str.to_uppercase().as_str() {
            "SOL_PERP" | "SOL" => Ok(Market::SOL_PERP),
            "BTC_PERP" | "BTC" => Ok(Market::BTC_PERP),
            "ETH_PERP" | "ETH" => Ok(Market::ETH_PERP),
            _ => Err("Requested market asset vector is unmapped or unsupported"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    BUY,
    SELL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    LIMIT,
    MARKET,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Filled,
    PartiallyFilled,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub price: Decimal,
    pub quantity: Decimal,
    pub taker_user_id: Uuid,
    pub taker_order_id: Uuid,
    pub maker_user_id: Uuid,
    pub maker_order_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOrderResult {
    pub fills: Vec<Fill>,
    pub executed_quantity: Decimal,
}

// Inbound Request Payload Payloads Mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrder {
    pub market: Market,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: OrderSide,
    pub user_id: Uuid,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrder {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub price: Decimal,
    pub side: OrderSide,
    pub market: Market,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelAllOrders {
    pub user_id: Uuid,
    pub market: Market,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOpenOrders {
    pub user_id: Uuid,
    pub market: Market,
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
