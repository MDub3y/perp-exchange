use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};

pub mod handlers;
pub mod state;

#[get("/")]
async fn ping() -> impl Responder {
    HttpResponse::Ok().body("Pong")
}

async fn manual_ping() -> impl Responder {
    HttpResponse::Ok().body("Heyy")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(ping)
            .route("/ping", web::get().to(manual_ping))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
