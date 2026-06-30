use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;

#[derive(Deserialize)]
pub enum Side {
    Long,
    Sohrt,
}

#[derive(Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Deserialize)]
pub struct OrderPayload {
    pub market_id: String,
    pub side: Side,
    pub price: f64,
    pub size: f64,
    pub order_type: OrderType,
}

#[derive(Deserialize)]
pub struct MarketPath {
    pub market_id: String,
}

pub async fn create_order(payload: web::Json<OrderPayload>) -> impl Responder {
    HttpResponse::Ok().json("Order placed successfully")
}

pub async fn cancel_order(path: web::Path<String>) -> impl Responder {
    let order_id = path.into_inner();
    HttpResponse::Ok().json(format!("Order {} cancelled", order_id))
}

pub async fn get_open_orders(path: web::Path<MarketPath>) -> impl Responder {
    HttpResponse::Ok().json(format!("Open orders for market {}", path.market_id))
}

pub async fn get_order_history(path: web::Path<MarketPath>) -> impl Responder {
    HttpResponse::Ok().json(format!("All orders for market {}", path.market_id))
}
