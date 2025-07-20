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

// Lista directory (versione di test)
pub async fn list_dir() -> Json<Vec<String>> {
    println!("--- HANDLER 'list_dir' ESEGUITO per la root ---");
    Json(vec![
        "file_di_test_1.txt".to_string(),
        "cartella_di_prova".to_string(),
        "debug.log".to_string(),
    ])
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