use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MarketPath {
    pub market_id: String,
}

pub async fn get_open_positions(path: web::Path<MarketPath>) -> impl Responder {
    HttpResponse::Ok().json(format!("Open positions for market {}", path.market_id))
}

pub async fn get_closed_positions(path: web::Path<MarketPath>) -> impl Responder {
    HttpResponse::Ok().json(format!(
        "Closed positions history for market {}",
        path.market_id
    ))
}
