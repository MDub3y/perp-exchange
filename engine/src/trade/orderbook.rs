use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, VecDeque},
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
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
}

pub struct OrderPayload {
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
            market,
            order: Order {
                user_id,
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
    pub taker_user_id: Uuid,
    pub taker_order_id: Uuid,
    pub maker_user_id: Uuid,
    pub maker_order_id: Uuid,
}

pub struct ProcessOrderResult {
    pub fills: Vec<Fill>,
    pub executed_quantity: Decimal,
}

pub struct OrderBookDepth {
    pub bids: Vec<(Decimal, Decimal)>,
    pub asks: Vec<(Decimal, Decimal)>,
}

pub struct Orderbook {
    bids: BTreeMap<Reverse<Decimal>, VecDeque<Order>>,
    asks: BTreeMap<Decimal, VecDeque<Order>>,
    order_price_map: HashMap<Uuid, Decimal>,
    market: Market,
}

impl Orderbook {
    pub fn new(market: Market) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_price_map: HashMap::new(),
            market,
        }
    }

    pub fn get_bids(&self) -> &BTreeMap<Reverse<Decimal>, VecDeque<Order>> {
        &self.bids
    }

    pub fn get_asks(&self) -> &BTreeMap<Decimal, VecDeque<Order>> {
        &self.asks
    }

    pub fn process_order(&mut self, payload: OrderPayload) -> Result<ProcessOrderResult, String> {
        let mut result = ProcessOrderResult {
            fills: Vec::new(),
            executed_quantity: Decimal::ZERO,
        };

        let mut taker_order = payload.order;

        match payload.order_type {
            OrderType::LIMIT => match payload.order_side {
                OrderSide::BUY => {
                    self.match_against_asks(&mut taker_order, &mut result)?;

                    if taker_order.quantity > Decimal::ZERO {
                        self.order_price_map
                            .insert(taker_order.order_id, taker_order.price);
                        self.bids
                            .entry(Reverse(taker_order.price))
                            .or_default()
                            .push_back(taker_order);
                    }
                }
                OrderSide::SELL => {
                    self.match_against_bids(&mut taker_order, &mut result)?;

                    if taker_order.quantity > Decimal::ZERO {
                        self.order_price_map
                            .insert(taker_order.order_id, taker_order.price);
                        self.asks
                            .entry(taker_order.price)
                            .or_default()
                            .push_back(taker_order);
                    }
                }
            },
            OrderType::MARKET => match payload.order_side {
                OrderSide::BUY => self.match_against_asks(&mut taker_order, &mut result)?,
                OrderSide::SELL => self.match_against_bids(&mut taker_order, &mut result)?,
            },
        }

        Ok(result)
    }

    pub fn match_against_asks(
        &mut self,
        taker_order: &mut Order,
        result: &mut ProcessOrderResult,
    ) -> Result<(), String> {
        while taker_order.quantity > Decimal::ZERO && !self.asks.is_empty() {
            let mut best_ask_price = *self.asks.keys().next().unwrap();

            if taker_order.price < best_ask_price && taker_order.price != Decimal::ZERO {
                break;
            }

            if let Some(queue) = self.asks.get_mut(&best_ask_price) {
                while taker_order.quantity > Decimal::ZERO && !queue.is_empty() {
                    let mut maker_order = queue.pop_front().unwrap();
                    let match_quantity = taker_order.quantity.min(maker_order.quantity);

                    taker_order.quantity -= match_quantity;
                    maker_order.quantity -= match_quantity;
                    result.executed_quantity += match_quantity;

                    result.fills.push(Fill {
                        price: best_ask_price,
                        quantity: match_quantity,
                        taker_order_id: taker_order.order_id,
                        taker_user_id: taker_order.user_id,
                        maker_order_id: maker_order.order_id,
                        maker_user_id: maker_order.user_id,
                    });

                    if maker_order.quantity > Decimal::ZERO {
                        queue.push_front(maker_order);
                    } else {
                        self.order_price_map.remove(&maker_order.order_id);
                    }
                }

                if queue.is_empty() {
                    self.asks.remove(&best_ask_price);
                }
            }
        }
        Ok(())
    }

    pub fn match_against_bids(
        &mut self,
        taker_order: &mut Order,
        result: &mut ProcessOrderResult,
    ) -> Result<(), String> {
        while taker_order.quantity > Decimal::ZERO && !self.bids.is_empty() {
            // Peek at the highest bid price level
            let best_bid_key = *self.bids.keys().next().unwrap();
            let best_bid_price = best_bid_key.0;

            // Limit Order boundary protection: stop execution if bid price is lower than our sell limit
            if taker_order.price > best_bid_price && taker_order.price != Decimal::ZERO {
                break;
            }

            if let Some(queue) = self.bids.get_mut(&best_bid_key) {
                while taker_order.quantity > Decimal::ZERO && !queue.is_empty() {
                    let mut maker_order = queue.pop_front().unwrap();
                    let match_quantity = taker_order.quantity.min(maker_order.quantity);

                    taker_order.quantity -= match_quantity;
                    maker_order.quantity -= match_quantity;
                    result.executed_quantity += match_quantity;

                    result.fills.push(Fill {
                        price: best_bid_price,
                        quantity: match_quantity,
                        taker_order_id: taker_order.order_id,
                        taker_user_id: taker_order.user_id,
                        maker_order_id: maker_order.order_id,
                        maker_user_id: maker_order.user_id,
                    });

                    if maker_order.quantity > Decimal::ZERO {
                        queue.push_front(maker_order);
                    } else {
                        self.order_price_map.remove(&maker_order.order_id);
                    }
                }

                if queue.is_empty() {
                    self.bids.remove(&best_bid_key);
                }
            }
        }
        Ok(())
    }

    pub fn get_depth(&self) -> OrderBookDepth {
        let mut depth = OrderBookDepth {
            bids: Vec::new(),
            asks: Vec::new(),
        };

        for (price_key, queue) in &self.bids {
            let total_volume: Decimal = queue.iter().map(|o| o.quantity).sum();
            depth.bids.push((price_key.0, total_volume));
        }

        for (price, queue) in &self.asks {
            let total_volume: Decimal = queue.iter().map(|o| o.quantity).sum();
            depth.asks.push((*price, total_volume));
        }

        depth
    }

    pub fn cancel_order(&mut self, order_id: Uuid) -> Result<(), String> {
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
