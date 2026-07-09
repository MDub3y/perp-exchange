use chrono::Utc;
use rust_decimal::Decimal;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
};
pub use utils::{Fill, Market, Order, OrderBookDepth, OrderSide, OrderType, ProcessOrderResult};
use uuid::Uuid;

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

pub struct Orderbook {
    pub bids: BTreeMap<Reverse<Decimal>, VecDeque<Order>>,
    pub asks: BTreeMap<Decimal, VecDeque<Order>>,
    pub order_price_map: HashMap<Uuid, Decimal>,
    pub user_orders_map: HashMap<Uuid, HashSet<Uuid>>,
    pub market: Market,
}

impl Orderbook {
    pub fn new(market: Market) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_price_map: HashMap::new(),
            user_orders_map: HashMap::new(),
            market,
        }
    }

    pub fn get_bids(&self) -> &BTreeMap<Reverse<Decimal>, VecDeque<Order>> {
        &self.bids
    }
    pub fn get_asks(&self) -> &BTreeMap<Decimal, VecDeque<Order>> {
        &self.asks
    }

    pub fn calculate_bid_impact(&self, target_notional: Decimal) -> Option<Decimal> {
        let mut remaining_notional = target_notional;
        let mut total_quantity = Decimal::ZERO;

        for (Reverse(price), queue) in &self.bids {
            for order in queue {
                let level_notional = order.price * order.quantity;
                if level_notional >= remaining_notional {
                    let fractional_qty = remaining_notional / order.price;
                    total_quantity += fractional_qty;
                    remaining_notional = Decimal::ZERO;
                    break;
                } else {
                    total_quantity += order.quantity;
                    remaining_notional -= level_notional;
                }
            }
            if remaining_notional.is_zero() {
                break;
            }
        }

        if remaining_notional.is_zero() && !total_quantity.is_zero() {
            Some(target_notional / total_quantity)
        } else {
            None
        }
    }

    pub fn calculate_ask_impact(&self, target_notional: Decimal) -> Option<Decimal> {
        let mut remaining_notional = target_notional;
        let mut total_quantity = Decimal::ZERO;

        for (price, queue) in &self.asks {
            for order in queue {
                let level_notional = *price * order.quantity;
                if level_notional >= remaining_notional {
                    let fractional_qty = remaining_notional / *price;
                    total_quantity += fractional_qty;
                    remaining_notional = Decimal::ZERO;
                    break;
                } else {
                    total_quantity += order.quantity;
                    remaining_notional -= level_notional;
                }
            }
            if remaining_notional.is_zero() {
                break;
            }
        }

        if remaining_notional.is_zero() && !total_quantity.is_zero() {
            Some(target_notional / total_quantity)
        } else {
            None
        }
    }

    pub fn process_order(
        &mut self,
        payload: Order,
        side: OrderSide,
        order_type: OrderType,
    ) -> Result<ProcessOrderResult, String> {
        let mut result = ProcessOrderResult {
            fills: Vec::new(),
            executed_quantity: Decimal::ZERO,
        };

        let mut taker_order = payload;

        match order_type {
            OrderType::LIMIT => match side {
                OrderSide::BUY => {
                    self.match_against_asks(&mut taker_order, &mut result)?;
                    if taker_order.quantity > Decimal::ZERO {
                        self.order_price_map
                            .insert(taker_order.order_id, taker_order.price);
                        self.user_orders_map
                            .entry(taker_order.user_id)
                            .or_default()
                            .insert(taker_order.order_id);
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
                        self.user_orders_map
                            .entry(taker_order.user_id)
                            .or_default()
                            .insert(taker_order.order_id);
                        self.asks
                            .entry(taker_order.price)
                            .or_default()
                            .push_back(taker_order);
                    }
                }
            },
            OrderType::MARKET => match side {
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
            let best_ask_price = *self.asks.keys().next().unwrap();
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
                        if let Some(set) = self.user_orders_map.get_mut(&maker_order.user_id) {
                            set.remove(&maker_order.order_id);
                        }
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
            let best_bid_key = *self.bids.keys().next().unwrap();
            let best_bid_price = best_bid_key.0;

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
                        if let Some(set) = self.user_orders_map.get_mut(&maker_order.user_id) {
                            set.remove(&maker_order.order_id);
                        }
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
            depth
                .bids
                .push((price_key.0, queue.iter().map(|o| o.quantity).sum()));
        }
        for (price, queue) in &self.asks {
            depth
                .asks
                .push((*price, queue.iter().map(|o| o.quantity).sum()));
        }
        depth
    }

    pub fn get_open_orders_for_user(&self, user_id: Uuid) -> Vec<Order> {
        let mut user_orders = Vec::new();
        if let Some(order_ids) = self.user_orders_map.get(&user_id) {
            for order_id in order_ids {
                if let Some(&price) = self.order_price_map.get(order_id) {
                    if let Some(queue) = self.asks.get(&price) {
                        if let Some(order) = queue.iter().find(|o| o.order_id == *order_id) {
                            user_orders.push(*order);
                            continue;
                        }
                    }
                    if let Some(queue) = self.bids.get(&Reverse(price)) {
                        if let Some(order) = queue.iter().find(|o| o.order_id == *order_id) {
                            user_orders.push(*order);
                        }
                    }
                }
            }
        }
        user_orders
    }

    pub fn cancel_order(&mut self, order_id: Uuid) -> Result<Option<Order>, String> {
        let price_val = match self.order_price_map.remove(&order_id) {
            Some(p) => p,
            None => return Ok(None),
        };

        let mut removed_order = None;
        if let Some(queue) = self.asks.get_mut(&price_val) {
            if let Some(pos) = queue.iter().position(|o| o.order_id == order_id) {
                removed_order = queue.remove(pos);
            }
        }
        if removed_order.is_none() {
            if let Some(queue) = self.bids.get_mut(&Reverse(price_val)) {
                if let Some(pos) = queue.iter().position(|o| o.order_id == order_id) {
                    removed_order = queue.remove(pos);
                }
            }
        }

        if let Some(order) = removed_order {
            if let Some(set) = self.user_orders_map.get_mut(&order.user_id) {
                set.remove(&order_id);
                if set.is_empty() {
                    self.user_orders_map.remove(&order.user_id);
                }
            }
            if self.asks.get(&price_val).map_or(false, |q| q.is_empty()) {
                self.asks.remove(&price_val);
            }
            if self
                .bids
                .get(&Reverse(price_val))
                .map_or(false, |q| q.is_empty())
            {
                self.bids.remove(&Reverse(price_val));
            }
            return Ok(Some(order));
        }

        Ok(None)
    }
}
