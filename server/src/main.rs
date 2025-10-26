// Dichiara il modulo `handlers.rs`
mod handlers;

use axum::{
    routing::{get, put, post, delete,patch},
    Router,
};
use std::net::SocketAddr;
use std::fs;
use handlers::*;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Crea la directory data se non esiste
    if let Err(e) = fs::create_dir_all("/home/luca/projects/rust-remote-fs/server/data") {
        println!("Warning: Could not create data directory: {}", e);
    }
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Ora il router è molto più leggibile
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/list", get(list_directory_contents))
        .route("/list/*path", get(list_directory_contents))
        .route("/mkdir/*path", post(mkdir))
        .route("/files/*path", get(get_file).put(put_file).delete(delete_file).patch(patch_file))
        // Applica il layer all'intero router, dopo aver definito tutte le rotte.
        .layer(TraceLayer::new_for_http());


    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    
    tracing::debug!("listening on {}", addr); // Questo log ora funzionerà
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}