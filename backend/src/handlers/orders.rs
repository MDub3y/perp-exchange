use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, web};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use redis::{OrderRequests, RedisManager};
use utils::{CancelOrderArgs, CreateOrderArgs, GetOpenOrders, Market, OrderSide, OrderType};

const INGESTION_STREAM: &str = "exchange:ingestion:stream";

#[derive(Deserialize)]
pub struct OrderInputPayload {
    pub order_id: Uuid,
    pub market_id: String,
    pub side: String,
    pub order_type: String,
    pub price: Decimal,
    pub size: Decimal,
    pub leverage: Decimal,
}

#[derive(Deserialize)]
pub struct CancelOrderPayload {
    pub market_id: String,
    pub side: String,
    pub price: Decimal,
}

#[derive(Deserialize)]
pub struct MarketPath {
    pub market_id: String,
}

#[post("/order")]
pub async fn create_order(
    req: HttpRequest,
    payload: web::Json<OrderInputPayload>,
    redis: web::Data<RedisManager>,
) -> impl Responder {
    let user_id = match crate::handlers::account::extract_user_id(&req) {
        Ok(id) => id,
        Err(err) => return HttpResponse::Unauthorized().json(err),
    };

    let market = match Market::from_str(&payload.market_id) {
        Ok(m) => m,
        Err(e) => return HttpResponse::BadRequest().json(e),
    };

    let side = match payload.side.to_uppercase().as_str() {
        "BUY" => OrderSide::BUY,
        "SELL" => OrderSide::SELL,
        _ => return HttpResponse::BadRequest().json("Execution side mapping must be BUY or SELL"),
    };

    let order_type = match payload.order_type.to_uppercase().as_str() {
        "LIMIT" => OrderType::LIMIT,
        "MARKET" => OrderType::MARKET,
        _ => return HttpResponse::BadRequest().json("Unsupported order type specification"),
    };

    if payload.size <= Decimal::ZERO {
        return HttpResponse::BadRequest()
            .json("Order allocation quantity must be greater than zero");
    }

    let execution_price = match payload.order_type.to_uppercase().as_str() {
        "LIMIT" => {
            if payload.price <= Decimal::ZERO {
                return HttpResponse::BadRequest()
                    .json("Limit orders require a valid price threshold above zero");
            }
            payload.price
        }
        "MARKET" => Decimal::ZERO,
        _ => {
            return HttpResponse::BadRequest()
                .json("Unsupported order_type. Must be LIMIT or MARKET");
        }
    };

    let request = OrderRequests::CreateOrder(CreateOrderArgs {
        order_id: payload.order_id,
        market,
        price: execution_price,
        quantity: payload.size,
        side,
        order_type,
        user_id,
        pubsub_id: Some(Uuid::new_v4()),
        leverage: payload.leverage,
    });

    match redis.enqueue_request(INGESTION_STREAM, &request).await {
        Ok(_) => HttpResponse::Accepted().json("Order logged cleanly to processing log stream"),
        Err(_) => HttpResponse::InternalServerError()
            .json("Spine connection interface communication exception"),
    }
}

#[delete("/order/{order_id}")]
pub async fn cancel_order(
    req: HttpRequest,
    path: web::Path<String>,
    payload: web::Json<CancelOrderPayload>,
    redis: web::Data<RedisManager>,
) -> impl Responder {
    let user_id = match crate::handlers::account::extract_user_id(&req) {
        Ok(id) => id,
        Err(err) => return HttpResponse::Unauthorized().json(err),
    };

    let order_uuid = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().json("Malformed Order UUID string parameter"),
    };

    let market = match Market::from_str(&payload.market_id) {
        Ok(m) => m,
        Err(e) => return HttpResponse::BadRequest().json(e),
    };

    let side = match payload.side.to_uppercase().as_str() {
        "BUY" => OrderSide::BUY,
        "SELL" => OrderSide::SELL,
        _ => return HttpResponse::BadRequest().json("Invalid execution side parameter"),
    };

    let request = OrderRequests::CancelOrder(CancelOrderArgs {
        order_id: order_uuid,
        user_id,
        price: payload.price,
        side,
        market,
        pubsub_id: Some(Uuid::new_v4()),
    });

    match redis.enqueue_request(INGESTION_STREAM, &request).await {
        Ok(_) => HttpResponse::Accepted().json("Cancellation request dispatched to matcher core"),
        Err(_) => HttpResponse::InternalServerError()
            .json("Failed to forward transaction down to the engine spine"),
    }
}

#[get("/orders/open/{market_id}")]
pub async fn get_open_orders(
    req: HttpRequest,
    path: web::Path<MarketPath>,
    redis: web::Data<RedisManager>,
) -> impl Responder {
    let user_id = match crate::handlers::account::extract_user_id(&req) {
        Ok(id) => id,
        Err(err) => return HttpResponse::Unauthorized().json(err),
    };

    let market = match Market::from_str(&path.market_id) {
        Ok(m) => m,
        Err(e) => return HttpResponse::BadRequest().json(e),
    };

    let request = OrderRequests::GetOpenOrders(GetOpenOrders {
        user_id,
        market,
        pubsub_id: Some(Uuid::new_v4()),
    });

    match redis.enqueue_request(INGESTION_STREAM, &request).await {
        Ok(_) => HttpResponse::Accepted()
            .json("Fetch open orders transaction logged into stream context"),
        Err(_) => {
            HttpResponse::InternalServerError().json("Processing pipeline query drop exception")
        }
    }
}

#[get("/orders/{market_id}")]
pub async fn get_order_history(
    req: HttpRequest,
    path: web::Path<MarketPath>,
    app_state: web::Data<crate::state::AppState>,
) -> impl Responder {
    let user_id = match crate::handlers::account::extract_user_id(&req) {
        Ok(id) => id,
        Err(err) => return HttpResponse::Unauthorized().json(err),
    };

    let records = sqlx::query!(
        "SELECT order_id, side::text, order_type::text, quantity, price, margin, status::text, created_at 
         FROM orders 
         WHERE user_id = $1 AND market_id = $2 
         ORDER BY created_at DESC LIMIT 100",
        user_id,
        path.market_id
    )
    .fetch_all(&app_state.db)
    .await;

    match records {
        Ok(rows) => {
            let clean_history: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "order_id": row.order_id,
                        "side": row.side,
                        "order_type": row.order_type,
                        "quantity": row.quantity.to_string(),
                        "price": row.price.to_string(),
                        "margin": row.margin.to_string(),
                        "status": row.status,
                        "created_at": row.created_at
                    })
                })
                .collect();

            HttpResponse::Ok().json(clean_history)
        }
        Err(e) => {
            eprintln!("Database analysis parsing exception: {}", e);
            HttpResponse::InternalServerError()
                .json("Failed to extract analytical account order logs")
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_order)
        .service(cancel_order)
        .service(get_open_orders)
        .service(get_order_history);
}
