use std::collections::{BTreeMap, VecDeque};

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

pub enum Market {
    SOL_PERP,
    BTC_PERP,
    ETH_PERP,
}

pub enum OrderSide {
    BUY,
    SELL,
}

pub enum OrderType {
    LIMIT,
    MARKET,
}

pub struct Order {
    pub user_id: Uuid,
    pub market: Market,
    pub order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub timestamp: i64,
}

impl Order {
    pub fn new(
        user_id: Uuid,
        market: Market,
        price: Decimal,
        quantity: Decimal,
        order_side: OrderSide,
        order_type: OrderType,
    ) -> Self {
        let order_id = Uuid::new_v4();
        let timestamp = Utc::now().timestamp_millis();
        Self {
            user_id,
            market,
            order_id,
            price,
            quantity,
            order_side,
            order_type,
            timestamp,
        }
    }
}

pub struct Orderbook {
    bids: BTreeMap<Decimal, VecDeque<Order>>,
    asks: BTreeMap<Decimal, VecDeque<Order>>,
    market: Market,
}
