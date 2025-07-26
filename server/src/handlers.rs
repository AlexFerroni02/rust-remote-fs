use axum::{
    extract::Path,
    body::Body,
    http::StatusCode,
    Json,
};
use std::fs;

// Lettura file
pub async fn get_file(Path(path): Path<String>) -> Result<String, StatusCode> {
    let file_path = format!("data/{}", path);
    match fs::read_to_string(&file_path) {
        Ok(content) => Ok(content),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

// Scrittura file
pub async fn put_file(Path(path): Path<String>, body: Body) -> StatusCode {
    let file_path = format!("data/{}", path);
    
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match fs::write(&file_path, &bytes) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
pub async fn list_root() -> Result<Json<Vec<String>>, StatusCode> {
    println!("chiamata a list_root");
    let mut entries = Vec::new();
    match fs::read_dir("data") {
        Ok(read_dir) => {
            for entry in read_dir {
                if let Ok(entry) = entry {
                    let file_name = entry.file_name().into_string().unwrap_or_default();
                    if entry.path().is_dir() {
                        entries.push(format!("{}/", file_name));
                    } else {
                        entries.push(file_name);
                    }
                }
            }
            println!("entries trovate: {:?}", entries);
            Ok(Json(entries))
        }
        Err(e) => {
            println!("errore lettura directory: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}
// Lista directory (versione di test)
pub async fn list_dir(path: Option<Path<String>>) -> Result<Json<Vec<String>>, StatusCode> {
    let path = match &path {
        Some(Path(p)) => p.trim_matches('/'),
        None => "",
    };
   
    let dir_path = if path.is_empty() {
        "data".to_string()
    } else {
        format!("data/{}", path)
    };

    let mut entries = Vec::new();
    match fs::read_dir(&dir_path) {
        Ok(read_dir) => {
            for entry in read_dir {
                if let Ok(entry) = entry {
                    let file_name = entry.file_name().into_string().unwrap_or_default();
                    // Se vuoi distinguere le directory, puoi aggiungere uno slash finale:
                    if entry.path().is_dir() {
                        entries.push(format!("{}/", file_name));
                    } else {
                        entries.push(file_name);
                    }
                }
            }
            Ok(Json(entries))
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

// Creazione directory
pub async fn mkdir(Path(path): Path<String>) -> StatusCode {
    let dir_path = format!("data/{}", path);
    match fs::create_dir_all(&dir_path) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// Cancellazione file o directory
pub async fn delete_file(Path(path): Path<String>) -> StatusCode {
    let file_path = format!("data/{}", path);
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