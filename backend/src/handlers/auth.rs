use actix_web::{HttpResponse, Responder, post, web};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthPayload {
    pub wallet_address: String,
    pub signature: String,
}

#[post("/signup")]
pub async fn signup(payload: web::Json<AuthPayload>) -> impl Responder {
    // TODO: wallet registration logic
    HttpResponse::Ok().json("User registered successfully")
}

#[post("/signin")]
pub async fn signin(payload: web::Json<AuthPayload>) -> impl Responder {
    // TODO: validate signature and issue JWT
    HttpResponse::Ok().json("User authenticated")
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(signup).service(signin);
}
