use std::path::Path;

use anyhow::{Context, Result};

/// Read a text file from the Windows filesystem.
pub async fn read_text_file(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read file: {}", path.display()))
}

/// Write a text file to the Windows filesystem.
/// Creates parent directories if they don't exist.
pub async fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    tokio::fs::write(path, content)
        .await
        .with_context(|| format!("Failed to write file: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_text_file_returns_content() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").expect("failed to write");

        let content = read_text_file(&file_path).await.expect("failed to read");
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn read_text_file_missing_returns_error() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("nonexistent.txt");

        let result = read_text_file(&file_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn write_text_file_creates_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("output.txt");

        write_text_file(&file_path, "test content")
            .await
            .expect("failed to write");

        let content = std::fs::read_to_string(&file_path).expect("failed to read back");
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn write_text_file_creates_parent_directories() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("nested").join("deep").join("file.txt");

        write_text_file(&file_path, "nested content")
            .await
            .expect("failed to write");

        let content = std::fs::read_to_string(&file_path).expect("failed to read back");
        assert_eq!(content, "nested content");
    }
}
