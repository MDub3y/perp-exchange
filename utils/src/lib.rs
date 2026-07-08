use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Eq, Hash, PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
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
            _ => Err("Unsupported or unmapped market asset indicator"),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserBalance {
    pub available_balance: Decimal,
    pub locked_balance: Decimal,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Order {
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderPayload {
    pub market: Market,
    pub order: Order,
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fill {
    pub price: Decimal,
    pub quantity: Decimal,
    pub taker_user_id: Uuid,
    pub taker_order_id: Uuid,
    pub maker_user_id: Uuid,
    pub maker_order_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessOrderResult {
    pub fills: Vec<Fill>,
    pub executed_quantity: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookDepth {
    pub bids: Vec<(Decimal, Decimal)>,
    pub asks: Vec<(Decimal, Decimal)>,
}

// Inbound API Gateway Request Layouts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderArgs {
    pub order_id: Uuid,
    pub market: Market,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub user_id: Uuid,
    pub pubsub_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderArgs {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub price: Decimal,
    pub side: OrderSide,
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
    CreateOrder(CreateOrderArgs),
    CancelOrder(CancelOrderArgs),
    GetOpenOrders(GetOpenOrders),
}
