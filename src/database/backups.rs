use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::get_connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub id: i64,
    pub site_id: Option<i64>,
    pub domain: String,
    pub backup_name: Option<String>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub includes_db: bool,
    pub includes_files: bool,
    pub metadata: Option<String>,
    pub storage_type: String,
    pub s3_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub domain: String,
    pub site_type: String,
    pub php_version: Option<String>,
    pub db_name: Option<String>,
    pub backup_date: String,
    pub files_included: bool,
    pub db_included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wp_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wp_plugins: Option<Vec<String>>,
}

/// Create a backup record in the database
#[allow(clippy::too_many_arguments)]
pub async fn create(
    site_id: Option<i64>,
    domain: &str,
    backup_name: Option<&str>,
    file_path: &str,
    file_size: Option<i64>,
    includes_db: bool,
    includes_files: bool,
    metadata: Option<&str>,
) -> Result<i64> {
    let conn = get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        "INSERT INTO backups (site_id, domain, backup_name, file_path, file_size, includes_db, includes_files, metadata, storage_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'local')",
        rusqlite::params![site_id, domain, backup_name, file_path, file_size, includes_db, includes_files, metadata],
    )?;

    Ok(conn.last_insert_rowid())
}

/// List all backups, optionally filtered by domain
pub async fn list(domain: Option<&str>) -> Result<Vec<Backup>> {
    let conn = get_connection()?;
    let conn = conn.lock().await;

    let query = if domain.is_some() {
        "SELECT id, site_id, domain, backup_name, file_path, file_size, includes_db, includes_files, metadata, storage_type, s3_url, created_at FROM backups WHERE domain = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, site_id, domain, backup_name, file_path, file_size, includes_db, includes_files, metadata, storage_type, s3_url, created_at FROM backups ORDER BY created_at DESC"
    };

    let mut stmt = conn.prepare(query)?;

    let rows = if let Some(d) = domain {
        stmt.query_map([d], row_to_backup)?
    } else {
        stmt.query_map([], row_to_backup)?
    };

    let mut backups = Vec::new();
    for row in rows {
        backups.push(row?);
    }

    Ok(backups)
}

/// Get a backup by ID
pub async fn get_by_id(id: i64) -> Result<Option<Backup>> {
    let conn = get_connection()?;
    let conn = conn.lock().await;

    let mut stmt = conn.prepare("SELECT id, site_id, domain, backup_name, file_path, file_size, includes_db, includes_files, metadata, storage_type, s3_url, created_at FROM backups WHERE id = ?1")?;

    let mut rows = stmt.query_map([id], row_to_backup)?;

    match rows.next() {
        Some(Ok(backup)) => Ok(Some(backup)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// Delete a backup record
pub async fn delete(id: i64) -> Result<()> {
    let conn = get_connection()?;
    let conn = conn.lock().await;

    conn.execute("DELETE FROM backups WHERE id = ?1", [id])?;
    Ok(())
}

/// Delete backups older than N days
pub async fn delete_older_than(days: u32) -> Result<u64> {
    let conn = get_connection()?;
    let conn = conn.lock().await;

    let deleted = conn.execute(
        "DELETE FROM backups WHERE created_at < datetime('now', ?1)",
        [format!("-{} days", days)],
    )?;

    Ok(deleted as u64)
}

/// Get backups older than N days (for file cleanup before deletion)
pub async fn get_older_than(days: u32) -> Result<Vec<Backup>> {
    let conn = get_connection()?;
    let conn = conn.lock().await;

    let mut stmt = conn.prepare(
        "SELECT id, site_id, domain, backup_name, file_path, file_size, includes_db, includes_files, metadata, storage_type, s3_url, created_at FROM backups WHERE created_at < datetime('now', ?1)",
    )?;

    let rows = stmt.query_map([format!("-{} days", days)], row_to_backup)?;

    let mut backups = Vec::new();
    for row in rows {
        backups.push(row?);
    }

    Ok(backups)
}

fn row_to_backup(row: &rusqlite::Row) -> rusqlite::Result<Backup> {
    Ok(Backup {
        id: row.get(0)?,
        site_id: row.get(1)?,
        domain: row.get(2)?,
        backup_name: row.get(3)?,
        file_path: row.get(4)?,
        file_size: row.get(5)?,
        includes_db: row.get(6)?,
        includes_files: row.get(7)?,
        metadata: row.get(8)?,
        storage_type: row.get(9)?,
        s3_url: row.get(10)?,
        created_at: row.get(11)?,
    })
}
