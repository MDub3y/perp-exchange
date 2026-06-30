use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};

pub mod handlers;
pub mod state;

async fn ping() -> impl Responder {
    HttpResponse::Ok().body("Pong")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_state = web::Data::new(state::AppState::new());

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/ping", web::get().to(ping))
            .service(
                web::scope("/api/v1")
                    // Auth Group
                    .route("/signup", web::post().to(handlers::auth::signup))
                    .route("/signin", web::post().to(handlers::auth::signin))
                    // Account Group
                    .route("/onramp", web::post().to(handlers::account::onramp))
                    .route(
                        "/equity/available",
                        web::get().to(handlers::account::get_available_equity),
                    )
                    .route("/fills", web::get().to(handlers::account::get_fills))
                    // Orders Group
                    .route("/order", web::post().to(handlers::orders::create_order))
                    .route(
                        "/order/{order_id}",
                        web::delete().to(handlers::orders::cancel_order),
                    )
                    .route(
                        "/orders/open/{market_id}",
                        web::get().to(handlers::orders::get_open_orders),
                    )
                    .route(
                        "/orders/{market_id}",
                        web::get().to(handlers::orders::get_order_history),
                    )
                    // Positions Group
                    .route(
                        "/positions/open/{market_id}",
                        web::get().to(handlers::positions::get_open_positions),
                    )
                    .route(
                        "/positions/closed/{market_id}",
                        web::get().to(handlers::positions::get_closed_positions),
                    ),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
