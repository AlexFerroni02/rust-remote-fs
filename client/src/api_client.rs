use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct RemoteEntry {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub mtime: i64,
    pub perm: String,
}

pub async fn get_files_from_server(client: &Client, path: &str) -> Result<Vec<RemoteEntry>, reqwest::Error> {
    let url = if path.is_empty() {
        "http://localhost:8080/files".to_string()
    } else {
        format!("http://localhost:8080/files/{}", path)
    };
    println!("API Client: requesting file list from {}", url);
    let response = client.get(&url).send().await?;
    response.json::<Vec<RemoteEntry>>().await
}

pub async fn get_file_content_from_server(client: &Client, path: &str) -> Result<String, reqwest::Error> {
    let url = format!("http://localhost:8080/file/{}", path);
    let response = client.get(&url).send().await?;
    response.text().await
}

pub async fn put_file_content_to_server(client: &Client, path: &str, content: &str) -> Result<(), reqwest::Error> {
    let url = format!("http://localhost:8080/file/{}", path);
    client.put(&url)
        .body(content.to_string())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

