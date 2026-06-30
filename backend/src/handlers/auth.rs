use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthPayload {
    pub wallet_address: String,
    pub signature: String,
}

pub async fn signup(payload: web::Json<AuthPayload>) -> impl Responder {
    // TODO: wallet registration logic
    HttpResponse::Ok().json("User registered successfully")
}

pub async fn signin(payload: web::Json<AuthPayload>) -> impl Responder {
    // TODO: validate signature and issue JWT
    HttpResponse::Ok().json("User authenticated")
}
