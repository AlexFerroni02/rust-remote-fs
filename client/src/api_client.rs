use reqwest::Body;
use reqwest::Client;
use serde::Deserialize;
use bytes::Bytes; // <-- AGGIUNTO

// --- RIMOSSI ---
// use tokio::fs::File;
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
// use tokio_util::io::ReaderStream;
// use futures_util::StreamExt;
// ---------------

#[derive(Deserialize, Debug)]
pub struct RemoteEntry {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub mtime: i64,
    pub perm: String,
}

type ClientResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub async fn get_files_from_server(client: &Client, path: &str) -> Result<Vec<RemoteEntry>, reqwest::Error> {
    let url = if path.is_empty() {
        "http://localhost:8080/list".to_string()
    } else {
        format!("http://localhost:8080/list/{}", path)
    };
    println!("API Client: requesting file list from {}", url);
    let response = client.get(&url).send().await?;
    response.json::<Vec<RemoteEntry>>().await
}

pub async fn get_file_content_from_server(client: &Client, path: &str) -> ClientResult<Bytes> {
    let url = format!("http://localhost:8080/files/{}", path);
    let response = client.get(&url).send().await?.error_for_status()?;

    // Legge l'intero corpo della risposta in memoria come Bytes
    // Questo risolve E0599 (bytes_stream) e E0277
    let data = response.bytes().await?;

    Ok(data)
}

pub async fn put_file_content_to_server(client: &Client, path: &str, data: Bytes) -> ClientResult<()> {
    let url = format!("http://localhost:8080/files/{}", path);

    // reqwest::Body può essere creato direttamente da Bytes
    // Questo risolve E0599 (wrap_stream) e E0433 (tokio_util)
    let body = Body::from(data);

    client.put(&url).body(body).send().await?.error_for_status()?;
    Ok(())
}