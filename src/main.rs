use std::io;

use actix_cors::Cors;
use actix_web::{http, middleware, web, App, HttpResponse, HttpServer};
use ffmtrans::serve::route::{trans_handler, ThreadChannel};

async fn preflight() -> io::Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .append_header(("Access-Control-Allow-Origin", "*"))
        .append_header(("Access-Control-Allow-Methods", "POST, OPTIONS"))
        .append_header((
            "Access-Control-Allow-Headers",
            "Content-Type, x-requested-with",
        ))
        .finish())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // thread controller
    let thread_channel = web::Data::new(ThreadChannel::new());
    // route init
    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["POST", "OPTIONS"])
                    .allow_any_header()
                    .max_age(3600),
            )
            .app_data(thread_channel.clone())
            .route("/setosd", web::post().to(trans_handler))
            .route("/setosd", web::method(http::Method::OPTIONS).to(preflight))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}
