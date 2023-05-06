use actix_web::{web, App, HttpServer};
use ffmtrans::serve::route::{trans_handler, ThreadChannel};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // thread controller
    let thread_channel = web::Data::new(ThreadChannel::new());
    // route init
    HttpServer::new(move || {
        App::new()
            .app_data(thread_channel.clone())
            .route("/setosd", web::post().to(trans_handler))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}
