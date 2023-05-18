use std::io;

use actix_cors::Cors;
use actix_web::{http, web, App, HttpResponse, HttpServer};
use ffmtrans::serve::route::{close_handler, trans_handler, ThreadChannel};

async fn preflight() -> io::Result<HttpResponse> {
    Ok(HttpResponse::Ok().finish())
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
            .route("/close", web::get().to(close_handler))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}
