//! The main entry point for the remote filesystem server.
//!
//! This binary initializes the Axum web server, sets up logging/tracing,
//! and defines all the API routes required by the FUSE client.
//! All route logic is forwarded to functions in the `handlers` module.

// Declares the module containing all HTTP request handlers.

mod handlers;

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
    routing::{get, put, post, delete,patch},
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use notify::{RecursiveMode, Watcher};
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tokio::sync::broadcast;
use std::net::SocketAddr;
use std::fs;
use std::time::{Duration, Instant};
use handlers::*; 
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
         // --- LOGICA DEL WATCHER E WEBSOCKET ---
    let (tx, _) = broadcast::channel(100);
    let recent_mods = Arc::new(Mutex::new(HashMap::new()));
   
    let app_state = AppState { 
        tx: Arc::new(tx),
        recent_mods: recent_mods.clone(),
    };

    let watcher_tx = app_state.tx.clone();
    let watcher_mods = recent_mods.clone();

    tokio::spawn(async move {
        let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if let Ok(relative_path) = path.strip_prefix(DATA_DIR) {
                        let path_str = relative_path.to_string_lossy().to_string();
                        
                        // --- LOGICA DI FIRMA CON DEBUG ---
                         let mut source_tag = String::new();
                        {
                            let mut mods = watcher_mods.lock().unwrap();
                            
                            // DECOMMENTA QUESTA RIGA:
                            println!("[DEBUG WATCHER] Cerco chiave '{}' nella mappa...", path_str);
                            
                            if let Some((client_id, time)) = mods.get(&path_str) {
                                if time.elapsed() < Duration::from_millis(500) {
                                    source_tag = format!("|BY:{}", client_id);
                                    println!("[DEBUG WATCHER] TROVATO! Modifica di {}", client_id);
                                } else {
                                    println!("[DEBUG WATCHER] Trovato ma SCADUTO (>500ms)");
                                }
                            } else {
                                // DECOMMENTA QUESTA RIGA:
                                println!("[DEBUG WATCHER] Chiave '{}' NON trovata. Chiavi presenti: {:?}", path_str, mods.keys());
                            }
                            
                            mods.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(5));
                        }
                        
                        let msg = format!("CHANGE:{}{}", path_str, source_tag);
                        println!("[WATCHER] Rilevato cambiamento: {}", msg);
                        let _ = watcher_tx.send(msg);
                    }
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[WATCHER] Errore nell'avviare il watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(std::path::Path::new(DATA_DIR), RecursiveMode::Recursive) {
            eprintln!("[WATCHER] Errore nel monitorare la directory {}: {}", DATA_DIR, e);
            return;
        }

        println!("[WATCHER] Watcher del filesystem avviato sulla directory: {}", DATA_DIR);
        std::future::pending::<()>().await;
    });
    // Define the application's routes.
    let app = Router::new()
    // A simple health check endpoint.
        .route("/health", get(|| async { "OK" }))
        .route("/ws", get(websocket_handler))
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
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::debug!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = stream.split();
    let mut rx = state.tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Close(_))) = receiver.next().await {
            break;
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
    println!("[WEBSOCKET] Client disconnesso.");
}