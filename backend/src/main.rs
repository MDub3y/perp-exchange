use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use sqlx::postgres::PgPoolOptions;
use std::env;

pub mod handlers;
pub mod state;

async fn ping() -> impl Responder {
    HttpResponse::Ok().body("Pong")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").unwrap();

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to establish database connection pool");
    println!("Database connection extablished!");

    let app_state = web::Data::new(state::AppState::new(db_pool));

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/ping", web::get().to(ping))
            .service(
                web::scope("/api/v1")
                    .configure(handlers::auth::config)
                    .configure(handlers::account::config)
                    .configure(handlers::orders::config)
                    .configure(handlers::positions::config),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
