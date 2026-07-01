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
