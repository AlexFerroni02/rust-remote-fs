use axum::{
    extract::Path,
    body::Body,
    http::StatusCode,
    Json,
};
use std::time::UNIX_EPOCH;
use std::os::unix::fs::PermissionsExt;
use std::fs;
use serde::Serialize;
#[derive(Serialize)]
pub struct RemoteEntry {
    name: String,
    kind: String,
    size: u64,
    mtime: i64,
    perm: String,
}
const DATA_DIR: &str= "/home/luca/projects/rust-remote-fs/server/data";
// Lettura file
pub async fn get_file(Path(path): Path<String>) -> Result<String, StatusCode> {
    let file_path = format!("{}/{}",DATA_DIR, path);
    match fs::read_to_string(&file_path) {
        Ok(content) => Ok(content),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

// Scrittura file
pub async fn put_file(Path(path): Path<String>, body: Body) -> StatusCode {
    let file_path =  format!("{}/{}",DATA_DIR, path);
    
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match fs::write(&file_path, &bytes) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
pub async fn list_directory_contents(path: Option<Path<String>>) -> Result<Json<Vec<RemoteEntry>>, StatusCode> {
    // Determina il percorso relativo
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
                // Estrae i metadati reali dal file
                let kind = if metadata.is_dir() { "directory".to_string() } else { "file".to_string() };

                let mtime = metadata.modified()
                    .unwrap_or(UNIX_EPOCH)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Converte i permessi in formato ottale (es. "755")
                let perm = format!("{:o}", metadata.permissions().mode() & 0o777);

                // Crea l'oggetto da inviare
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

// Creazione directory
pub async fn mkdir(Path(path): Path<String>) -> StatusCode {
    let dir_path =  format!("{}/{}",DATA_DIR, path);
    match fs::create_dir_all(&dir_path) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// Cancellazione file o directory
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