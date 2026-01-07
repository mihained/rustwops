use anyhow::Result;
use std::path::Path;

/// Ensure a directory exists
pub async fn ensure_dir(path: &str) -> Result<()> {
    tokio::fs::create_dir_all(path).await?;
    Ok(())
}

/// Copy a file
pub async fn copy_file(src: &str, dst: &str) -> Result<()> {
    tokio::fs::copy(src, dst).await?;
    Ok(())
}

/// Move a file
pub async fn move_file(src: &str, dst: &str) -> Result<()> {
    tokio::fs::rename(src, dst).await?;
    Ok(())
}

/// Delete a file
pub async fn delete_file(path: &str) -> Result<()> {
    tokio::fs::remove_file(path).await?;
    Ok(())
}

/// Delete a directory recursively
pub async fn delete_dir(path: &str) -> Result<()> {
    tokio::fs::remove_dir_all(path).await?;
    Ok(())
}

/// Check if a path exists
pub async fn exists(path: &str) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Check if a path is a directory
pub async fn is_dir(path: &str) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

/// Check if a path is a file
pub async fn is_file(path: &str) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false)
}

/// Get file size in bytes
pub async fn file_size(path: &str) -> Result<u64> {
    let metadata = tokio::fs::metadata(path).await?;
    Ok(metadata.len())
}

/// Format file size for display
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Get the filename from a path
pub fn filename(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}
