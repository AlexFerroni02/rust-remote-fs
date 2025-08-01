#[cfg(test)]
mod endpoints_tests  {
    use reqwest::{Client, StatusCode};

    const BASE_URL: &str = "http://127.0.0.1:8080";

    #[tokio::test]
    async fn test_health_endpoint() {
        let response = reqwest::get(format!("{}/health", BASE_URL))
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await.expect("Failed to read response body");
        assert_eq!(body, "OK");
    }

    #[tokio::test]
    async fn test_list_root_directory() {
        let response = reqwest::get(format!("{}/list/", BASE_URL))
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
        let body: Vec<String> = response.json().await.expect("Failed to parse response body");
        println!("Root directory contents: {:?}", body);
    }

    #[tokio::test]
    async fn test_list_nested_directory() {
        let response = reqwest::get(format!("{}/list/test_dir", BASE_URL))
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
        let body: Vec<String> = response.json().await.expect("Failed to parse response body");
        println!("Nested directory contents: {:?}", body);
    }

    #[tokio::test]
    async fn test_read_file() {
        let response = reqwest::get(format!("{}/files/test_file.txt", BASE_URL))
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await.expect("Failed to read response body");
        assert_eq!(body, "Hello, world!");
    }

    #[tokio::test]
    async fn test_write_file() {
        let client = Client::new();
        let response = client
            .put(format!("{}/files/new_file.txt", BASE_URL))
            .body("New file content")
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_overwrite_file() {
        let client = Client::new();

        // Create a file
        let create_response = client
            .put(format!("{}/files/overwrite_test.txt", BASE_URL))
            .body("Initial content")
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(create_response.status(), StatusCode::OK);

        // Overwrite the file
        let overwrite_response = client
            .put(format!("{}/files/overwrite_test.txt", BASE_URL))
            .body("Overwritten content")
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(overwrite_response.status(), StatusCode::OK);

        // Read the file
        let read_response = client
            .get(format!("{}/files/overwrite_test.txt", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(read_response.status(), StatusCode::OK);
        let body = read_response.text().await.expect("Failed to read response body");
        assert_eq!(body, "Overwritten content");
    }

    #[tokio::test]
    async fn test_create_directory() {
        let client = Client::new();
        let response = client
            .post(format!("{}/mkdir/new_directory", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_delete_file() {
        let client = Client::new();
        let response = client
            .delete(format!("{}/files/new_file.txt", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_delete_directory() {
        let client = Client::new();

        // Create a directory
        let create_response = client
            .post(format!("{}/mkdir/test_delete_dir", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(create_response.status(), StatusCode::OK);

        // Delete the directory
        let delete_response = client
            .delete(format!("{}/files/test_delete_dir", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(delete_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_path() {
        let client = Client::new();

        // Attempt to read a non-existent file
        let response = client
            .get(format!("{}/files/non_existent_file.txt", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Attempt to delete a non-existent file
        let delete_response = client
            .delete(format!("{}/files/non_existent_file.txt", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(delete_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_empty_directory() {
        let client = Client::new();

        // Create an empty directory
        let create_response = client
            .post(format!("{}/mkdir/empty_dir", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(create_response.status(), StatusCode::OK);

        // List the empty directory
        let list_response = client
            .get(format!("{}/list/empty_dir", BASE_URL))
            .send()
            .await
            .expect("Failed to send request");
        assert_eq!(list_response.status(), StatusCode::OK);
        let body: Vec<String> = list_response.json().await.expect("Failed to parse response body");
        assert!(body.is_empty());
    }
}