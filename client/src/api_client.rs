//! This module defines the asynchronous API client for interacting with the remote server.
//!
//! All functions here use `reqwest` to perform HTTP requests and are intended to be
//! called from within the Tokio runtime (e.g., using `runtime.block_on` in the
//! synchronous FUSE implementation).

use reqwest::Body;
use reqwest::Client;
use serde::Deserialize;
use bytes::Bytes;
use serde_json::json; // Aggiunto per gestire il JSON del metodo PATCH

/// Represents a single file or directory entry returned by the server's `/list` endpoint.
///
/// This struct is deserialized directly from the server's JSON response.
#[derive(Deserialize, Debug)]
pub struct RemoteEntry {
    /// The name of the file or directory (e.g., "file.txt").
    pub name: String,
    /// The type of the entry ("file" or "directory").
    pub kind: String,
    /// The size of the file in bytes.
    pub size: u64,
    /// The modification time (mtime) as a Unix timestamp (seconds since epoch).
    pub mtime: i64,
    /// The file permissions as an octal string (e.g., "644").
    pub perm: String,
}

/// A generic `Result` type for API client functions, using a dynamic Error.
///
/// This simplifies error handling by boxing any error that occurs
/// (e.g., `reqwest::Error`, `std::io::Error`).
type ClientResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Fetches the list of directory entries from the server's `/list` endpoint.
///
/// This corresponds to a `readdir` operation. It handles both the root directory
/// (when `path` is empty) and subdirectories.
///
/// # Arguments
/// * `client` - The shared `reqwest::Client` instance.
/// * `path` - The relative path of the directory to list. An empty string signifies the root.
///
/// # Returns
/// A `Result` containing a `Vec<RemoteEntry>` on success, or a `reqwest::Error`.
pub async fn get_files_from_server(client: &Client, path: &str, base_url: &str) -> Result<Vec<RemoteEntry>, reqwest::Error> {
    let url = if path.is_empty() {
        format!("{}/list", base_url)
    } else {
        format!("{}/list/{}", base_url, path)
    };
    println!("API Client: requesting file list from {}", url);
    let response = client.get(&url).send().await?;
    response.json::<Vec<RemoteEntry>>().await
}

/// Fetches the entire content of a file from the server's `/files` endpoint.
///
/// This corresponds to a `read` operation. It reads the *entire* file into memory
/// at once. The FUSE `read` handler is responsible for slicing this `Bytes` object
/// to satisfy the kernel's specific offset and size request.
///
/// # Arguments
/// * `client` - The shared `reqwest::Client` instance.
/// * `path` - The relative path of the file to read.
///
/// # Returns
/// A `ClientResult` containing the file's content as `Bytes` on success.
pub async fn get_file_content_from_server(client: &Client, path: &str, base_url: &str) -> ClientResult<Bytes> {
    let url = format!("{}/files/{}", base_url, path);
    let response = client.get(&url).send().await?.error_for_status()?;

    // Reads the entire response body into memory as Bytes
    let data = response.bytes().await?;

    Ok(data)
}

/// Uploads (or overwrites) the entire content of a file to the server's `/files` endpoint.
///
/// This function is used by `create` (to create an empty file) and `release` (to
/// upload the final, merged content after writes). It performs a `PUT` request
/// with the provided `Bytes` as the request body.
///
/// # Arguments
/// * `client` - The shared `reqwest::Client` instance.
/// * `path` - The relative path of the file to write.
/// * `data` - The complete byte content to upload.
///
/// # Returns
/// A `ClientResult<()>` indicating success or failure.
pub async fn put_file_content_to_server(client: &Client, path: &str, data: Bytes, base_url: &str) -> ClientResult<()> {
    let url = format!("{}/files/{}", base_url, path);

    // reqwest::Body can be created directly from Bytes
    let body = Body::from(data);

    // Send the PUT request and check for HTTP errors (4xx, 5xx)
    client.put(&url).body(body).send().await?.error_for_status()?;
    Ok(())
}

/// Deletes a file or directory on the server via the `/files` endpoint.
///
/// This corresponds to `unlink` or `rmdir` operations.
///
/// # Arguments
/// * `client` - The shared `reqwest::Client` instance.
/// * `path` - The relative path of the resource to delete.
pub async fn delete_resource(client: &Client, path: &str, base_url: &str) -> ClientResult<()> {
    let url = format!("{}/files/{}", base_url, path);
    client.delete(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Creates a new directory on the server via the `/mkdir` endpoint.
///
/// This corresponds to the `mkdir` operation.
///
/// # Arguments
/// * `client` - The shared `reqwest::Client` instance.
/// * `path` - The relative path of the directory to create.
pub async fn create_directory(client: &Client, path: &str, base_url: &str) -> ClientResult<()> {
    let url = format!("{}/mkdir/{}", base_url, path);
    client.post(&url).send().await?.error_for_status()?;
    Ok(())
}

/// Updates file permissions via a `PATCH` request to the `/files` endpoint.
///
/// This is used by `setattr` (chmod). It sends a JSON payload containing
/// the new octal permission string (e.g., `{ "perm": "755" }`).
///
/// # Arguments
/// * `client` - The shared `reqwest::Client` instance.
/// * `path` - The relative path of the file.
/// * `mode` - The new mode (u32) from which permissions are extracted.
pub async fn update_permissions(client: &Client, path: &str, mode: u32, base_url: &str) -> ClientResult<()> {
    let perm_str = format!("{:o}", mode & 0o777);
    let url = format!("{}/files/{}", base_url, path);
    let payload = json!({ "perm": perm_str });

    client.patch(&url).json(&payload).send().await?.error_for_status()?;
    Ok(())
}