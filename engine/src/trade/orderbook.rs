use std::{
    cmp::Reverse,
    collections::{BTreeMap, VecDeque},
};

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

pub enum OrderStatus {
    Pending,
    Filled,
    PartiallyFilled,
    Cancelled,
}

pub struct Order {
    pub order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
}

pub struct OrderPayload {
    pub user_id: Uuid,
    pub market: Market,
    pub order: Order,
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub timestamp: i64,
}

impl OrderPayload {
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
            order: Order {
                order_id,
                price,
                quantity,
            },
            order_side,
            order_type,
            timestamp,
        }
    }
}

pub struct Fill {
    pub price: Decimal,
    pub quantity: Decimal,
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub matching_user_id: Uuid,
}

pub struct ProcessOrderResult {
    pub fills: Vec<Fill>,
    pub executed_quantity: Decimal,
}

pub struct Orderbook {
    bids: BTreeMap<Reverse<Decimal>, VecDeque<Order>>,
    asks: BTreeMap<Decimal, VecDeque<Order>>,
    market: Market,
}

impl Orderbook {
    pub fn new(market: Market) -> Self {
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();

        Self { bids, asks, market }
    }

    pub fn get_bids(&self) -> &BTreeMap<Reverse<Decimal>, VecDeque<Order>> {
        &self.bids
    }

    pub fn process_order(&mut self, payload: OrderPayload) -> Result<(), String> {
        match payload.order_type {
            OrderType::LIMIT => {
                match payload.order_side {
                    // TODO: matching logic if awailable
                    OrderSide::BUY => self
                        .bids
                        .entry(Reverse(payload.order.price))
                        .or_default()
                        .push_back(payload.order),
                    OrderSide::SELL => self
                        .asks
                        .entry(payload.order.price)
                        .or_default()
                        .push_back(payload.order),
                }
            }
            OrderType::MARKET => {
                match payload.order_side {
                    OrderSide::BUY => {
                        // walk the asks side and fill
                    }
                    OrderSide::SELL => {
                        // walk the bids side and fill
                    }
                }
            }
        }

        Ok(())
    }

    pub fn match_asks() -> Result<(), String> {
        Ok(())
    }

    pub fn match_bids() -> Result<(), String> {
        Ok(())
    }

    pub fn get_depth() -> Result<(), String> {
        // asks + bids
        Ok(())
    }

    pub fn cancel_order() -> Result<(), String> {
        Ok(())
    }

    pub fn cancel_all_orders() -> Result<(), String> {
        Ok(())
    }

    pub fn get_open_order() -> Result<(), String> {
        // not sure if this needs to list all the orders, as how will the user query using the order_id
        // but search by user id is possible
        Ok(())
    }
}
