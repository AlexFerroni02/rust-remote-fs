use axum::{
    extract::Path,
    body::Body,
    http::StatusCode,
    Json,
};
use std::time::UNIX_EPOCH;
use std::os::unix::fs::PermissionsExt;
use std::fs;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use http_body_util::BodyExt;
use hyper::body::Frame;

/// Represents a single file or directory entry returned by the `/list` endpoint.
/// This is serialized to JSON for the client.
#[derive(Serialize,Deserialize)]
pub struct RemoteEntry {
    name: String,
    kind: String,
    size: u64,
    mtime: i64,
    perm: String,
}

/// Represents the JSON payload for a `PATCH` request to change permissions.
/// The `perm` field is expected to be an octal string (e.g., "755").
#[derive(Deserialize)]
pub struct UpdatePermissions {
    perm: String,
}

/// Defines the root directory on the server where all remote files are stored.
/// It's set to a `data` subdirectory within the project's root.
const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data");

/// Handles `GET /files/<path>`.
///
/// Reads a file from the server's data directory and streams its content
/// back to the client. This is a streaming response, capable of
/// handling large files without loading them entirely into memory.
///
/// # Arguments
/// * `Path(path)` - The relative path of the file to read, extracted from the URL.
///
/// # Returns
/// * `Ok(Body)` containing the file's data stream on success.
/// * `Err(StatusCode::NOT_FOUND)` if the file does not exist.
pub async fn get_file(Path(path): Path<String>) -> Result<Body, StatusCode> {
    let file_path = format!("{}/{}",DATA_DIR, path);
    let file = File::open(&file_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    Ok(Body::from_stream(stream))
}

/// Handles `PUT /files/<path>`.
///
/// Receives a streaming request body from the client and writes the data
/// to a file in the server's data directory. This overwrites any existing file.
/// This handler is capable of receiving large files without buffering them
/// entirely in memory.
///
/// # Arguments
/// * `Path(path)` - The relative path of the file to write.
/// * `body` - The streaming `Body` of the `PUT` request.
///
/// # Returns
/// * `StatusCode::OK` on success.
/// * `StatusCode::INTERNAL_SERVER_ERROR` if creating or writing the file fails.
/// * `StatusCode::BAD_REQUEST` if the request body stream is invalid.
pub async fn put_file(Path(path): Path<String>, mut body: Body) -> StatusCode {
    let file_path = format!("{}/{}", DATA_DIR, path);
    let mut file = match File::create(&file_path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    // Stream the body chunk by chunk
    while let Some(result) = body.frame().await {
        let frame = match result {
            Ok(frame) => frame,
            Err(_) => return StatusCode::BAD_REQUEST, // Error streaming the request body
        };

        // Write the data frame to the file
        if let Some(data) = frame.data_ref() {
            if file.write_all(data).await.is_err() {
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
    }

    StatusCode::OK
}

/// Handles `GET /list` and `GET /list/<path>`.
///
/// Lists the contents of a directory specified by the optional `path`.
/// If `path` is `None` (from the `/list` route), it lists the root of `DATA_DIR`.
///
/// It iterates the directory, reads metadata for each entry, and constructs
/// a `RemoteEntry` struct containing name, kind, size, mtime, and permissions.
///
/// # Arguments
/// * `path` - An `Option<Path<String>>` extracted from the URL.
///
/// # Returns
/// * `Ok(Json<Vec<RemoteEntry>>)` with the list of directory entries.
/// * `Err(StatusCode::NOT_FOUND)` if the specified directory does not exist.
pub async fn list_directory_contents(path: Option<Path<String>>) -> Result<Json<Vec<RemoteEntry>>, StatusCode> {
    // Determine the relative path
    let relative_path = path.map_or("".to_string(), |Path(p)| p);
    let full_path =  format!("{}/{}",DATA_DIR, relative_path);

    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(&full_path) {
        Ok(rd) => rd,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    for entry_result in read_dir {
        if let Ok(entry) = entry_result {
            if let Ok(metadata) = entry.metadata() {
                // Extract real metadata from the file
                let kind = if metadata.is_dir() { "directory".to_string() } else { "file".to_string() };

                let mtime = metadata.modified()
                    .unwrap_or(UNIX_EPOCH)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Convert permissions to octal format (e.g., "755")
                let perm = format!("{:o}", metadata.permissions().mode() & 0o777);

                // Create the object to send
                entries.push(RemoteEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    kind,
                    size: metadata.len(),
                    mtime,
                    perm,
                });
            }
        }
    }

    Ok(Json(entries))
}

/// Handles `POST /mkdir/<path>`.
///
/// Creates a new directory (and any necessary parent directories, like `mkdir -p`)
/// at the specified path within `DATA_DIR`.
///
/// # Arguments
/// * `Path(path)` - The relative path of the directory to create.
///
/// # Returns
/// * `StatusCode::OK` on success.
/// * `StatusCode::INTERNAL_SERVER_ERROR` if directory creation fails.
pub async fn mkdir(Path(path): Path<String>) -> StatusCode {
    let dir_path =  format!("{}/{}",DATA_DIR, path);
    match fs::create_dir_all(&dir_path) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Handles `DELETE /files/<path>`.
///
/// Deletes a file or directory at the specified path.
/// - If the path is a directory, it is removed recursively (`rm -r`).
/// - If the path is a file, it is removed.
///
/// # Arguments
/// * `Path(path)` - The relative path of the item to delete.
///
/// # Returns
/// * `StatusCode::OK` on success.
/// * `StatusCode::NOT_FOUND` if the path does not exist.
/// * `StatusCode::INTERNAL_SERVER_ERROR` if the deletion fails.
pub async fn delete_file(Path(path): Path<String>) -> StatusCode {
    let file_path =  format!("{}/{}",DATA_DIR, path);
    if let Ok(meta) = fs::metadata(&file_path) {
        if meta.is_dir() {
            match fs::remove_dir_all(&file_path) {
                Ok(_) => StatusCode::OK,
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
            }
        } else {
            match fs::remove_file(&file_path) {
                Ok(_) => StatusCode::OK,
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
            }
        }
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Handles `PATCH /files/<path>`.
///
/// Updates the file permissions (mode) of a file or directory.
/// This is used by the FUSE client to implement `chmod`.
///
/// # Arguments
/// * `Path(path)` - The relative path of the item to modify.
/// * `Json(payload)` - A JSON body `{"perm": "755"}` with the new octal permissions.
///
/// # Returns
/// * `StatusCode::OK` on success.
/// * `StatusCode::BAD_REQUEST` if the octal string in the payload is invalid.
/// * `StatusCode::NOT_FOUND` if the path does not exist.
/// * `StatusCode::INTERNAL_SERVER_ERROR` if setting permissions fails.
pub async fn patch_file(Path(path): Path<String>, Json(payload): Json<UpdatePermissions>) -> StatusCode {
    let file_path = format!("{}/{}", DATA_DIR, path);

    // Convert permissions from octal string to u32
    let mode = match u32::from_str_radix(&payload.perm, 8) {
        Ok(m) => m,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    // Read current permissions and set the new ones
    match fs::metadata(&file_path) {
        Ok(metadata) => {
            let mut perms = metadata.permissions();
            perms.set_mode(mode); // Set the new permissions
            if fs::set_permissions(&file_path, perms).is_ok() {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
        Err(_) => StatusCode::NOT_FOUND,
    }
}