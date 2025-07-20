// Dichiara il modulo `handlers.rs`
mod handlers;

use axum::{
    routing::{get, put, post, delete},
    Router,
};
use std::net::SocketAddr;
use std::fs;
// Importa tutti gli handler pubblici dal modulo
use handlers::*;

#[tokio::main]
async fn main() {
    // Crea la directory data se non esiste
    if let Err(e) = fs::create_dir_all("data") {
        println!("Warning: Could not create data directory: {}", e);
    }

    // Ora il router è molto più leggibile
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/list/", get(list_dir)) // La tua rotta statica che ora funziona
        .route("/files/*path", get(get_file).put(put_file).delete(delete_file))
        .route("/mkdir/*path", post(mkdir));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}