use actix_web::{HttpResponse, Responder, get, post, web};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct OnRampPayload {
    pub amount: f64,
    pub asset: String,
}

#[post("/onramp")]
pub async fn onramp(payload: web::Json<OnRampPayload>) -> impl Responder {
    HttpResponse::Ok().json("Deposit initialized")
}

#[post("/equity/available")]
pub async fn get_available_equity() -> impl Responder {
    HttpResponse::Ok().json("Available equity data")
}

#[get("/fills")]
pub async fn get_fills() -> impl Responder {
    HttpResponse::Ok().json("Trade fills history")
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(onramp)
        .service(get_available_equity)
        .service(get_fills);
}
