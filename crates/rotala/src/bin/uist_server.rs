use std::env;
use std::sync::Mutex;

use actix_web::{web, App, HttpServer};
use rotala::exchange::uist::random_uist_generator;
use rotala::server::uist::{delete_order, fetch_quotes, init, insert_order, tick};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let address: String = args[1].clone();
    let port: u16 = args[2].parse().unwrap();

    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(Mutex::new(random_uist_generator(3000).0)))
            .route("/init", web::get().to(init))
            .route("/fetch_quotes", web::get().to(fetch_quotes))
            .route("/tick", web::get().to(tick))
            .route("/insert_order", web::post().to(insert_order))
            .route("/delete_order", web::post().to(delete_order))
    })
    .bind((address, port))?
    .run()
    .await
}