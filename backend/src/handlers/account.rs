use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use bigdecimal::{BigDecimal, FromPrimitive};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct OnRampPayload {
    pub amount: BigDecimal,
    pub asset: String,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub user_id: Uuid,
    pub available_balance: String,
    pub locked_balance: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

fn extract_user_id(req: &HttpRequest) -> Result<Uuid, &'static str> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .ok_or("Missing Authorization Header")?
        .to_str()
        .map_err(|_| "Invalid Header FOrmatting!")?;

    if !auth_header.starts_with("Bearer ") {
        return Err("Authorization scheme must be bearer");
    }

    let token = &auth_header[7..];
    let jwt_secret = env::var("JWT_SECRET")
        .unwrap_or_else(|_| "default_fallback_secret_key_string_matrix_987".to_string());

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| "Cryptographic token verification failed.")?;

    Uuid::parse_str(&token_data.claims.sub).map_err(|_| "Malformed User UUID payload inside token")
}

#[post("/onramp")]
pub async fn onramp(
    req: HttpRequest,
    payload: web::Json<OnRampPayload>,
    app_state: web::Data<crate::state::AppState>,
) -> impl Responder {
    let user_id = match extract_user_id(&req) {
        Ok(id) => id,
        Err(err_msg) => return HttpResponse::Unauthorized().json(err_msg),
    };

    if payload.amount <= BigDecimal::from_f64(0.0).unwrap() {
        return HttpResponse::BadRequest()
            .json("Deposit allocation amount must be greater than zero");
    }

    if payload.asset.to_uppercase() != "USD" {
        return HttpResponse::BadRequest()
            .json("Currently, only USD assets are supported for processing");
    }

    let execution_result = sqlx::query!(
        "UPDATE collateral 
         SET available_balance = available_balance + $1, updated_at = NOW() 
         WHERE user_id = $2",
        payload.amount,
        user_id
    )
    .execute(&app_state.db)
    .await;

    match execution_result {
        Ok(res) if res.rows_affected() > 0 => {
            println!(
                "Successfully onramped ${} USD to user {}",
                payload.amount, user_id
            );
            HttpResponse::Ok().json(format!(
                "Successfully deposited {} {}",
                payload.amount, payload.asset
            ))
        }
        Ok(_) => HttpResponse::NotFound()
            .json("Collateral account entry not found for authenticated profile"),
        Err(e) => {
            eprintln!("Database exception handling balance update: {}", e);
            HttpResponse::InternalServerError()
                .json("Failed to commit onramp transaction change context")
        }
    }
}

#[post("/equity/available")]
pub async fn get_available_equity(
    req: HttpRequest,
    app_state: web::Data<crate::state::AppState>,
) -> impl Responder {
    let user_id = match extract_user_id(&req) {
        Ok(id) => id,
        Err(err_msg) => return HttpResponse::Unauthorized().json(err_msg),
    };

    let record = sqlx::query!(
        "SELECT available_balance, locked_balance FROM collateral WHERE user_id = $1",
        user_id
    )
    .fetch_optional(&app_state.db)
    .await;

    match record {
        Ok(Some(row)) => HttpResponse::Ok().json(BalanceResponse {
            user_id,
            available_balance: row.available_balance.to_string(),
            locked_balance: row.locked_balance.to_string(),
        }),
        Ok(None) => HttpResponse::NotFound()
            .json("No collateral records open for this account profile identifier"),
        Err(e) => {
            eprintln!("Database exception fetching equity: {}", e);
            HttpResponse::InternalServerError()
                .json("Database network error fetching balance records")
        }
    }
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
