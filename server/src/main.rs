//! The main entry point for the remote filesystem server.
//!
//! This binary initializes the Axum web server, sets up logging/tracing,
//! and defines all the API routes required by the FUSE client.
//! All route logic is forwarded to functions in the `handlers` module.

// Declares the module containing all HTTP request handlers.
mod handlers;

use axum::{
    routing::{get, put, post, delete,patch},
    Router,
};
use std::net::SocketAddr;
use std::fs;
use handlers::*; // Import all handlers from the handlers module
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Ensure the data directory exists.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    if let Err(e) = fs::create_dir_all(manifest_dir.to_owned() + "/data"){
        println!("Warning: Could not create data directory: {}", e);
    }

    // Initialize the logging and tracing subscriber.
    // Uses `RUST_LOG` env var or defaults to "server=debug,tower_http=debug".
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Define the application's routes.
    let app = Router::new()
        // A simple health check endpoint.
        .route("/health", get(|| async { "OK" }))

        // Routes for listing directory contents.
        // Both `/list` (for root) and `/list/*path` (for subdirs)
        // are handled by the same `list_directory_contents` handler.
        .route("/list", get(list_directory_contents))
        .route("/list/*path", get(list_directory_contents))

        // Route for creating a new directory.
        .route("/mkdir/*path", post(mkdir))

        // Routes for file operations (Read, Write, Delete, Chmod).
        // All file-based operations are grouped under the `/files/` path.
        .route("/files/*path", get(get_file).put(put_file).delete(delete_file).patch(patch_file))

        // Apply a logging layer to trace all HTTP requests.
        .layer(TraceLayer::new_for_http());


    // Bind the server to the loopback address on port 8080.
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    // Start the Axum server.
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}