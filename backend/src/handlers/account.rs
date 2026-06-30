use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct OnRampPayload {
    pub amount: f64,
    pub asset: String,
}

pub async fn onramp(payload: web::Json<OnRampPayload>) -> impl Responder {
    HttpResponse::Ok().json("Deposit initialized")
}

pub async fn get_available_equity() -> impl Responder {
    HttpResponse::Ok().json("Available equity data")
}

pub async fn get_fills() -> impl Responder {
    HttpResponse::Ok().json("Trade fills history")
}
