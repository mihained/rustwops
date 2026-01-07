use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};

#[derive(Debug, Clone)]
pub struct Database {
    pub id: i64,
    pub site_id: Option<i64>,
    pub domain: String,
    pub db_name: String,
    pub db_user: String,
    pub db_type: String,
    pub created_at: String,
}

pub async fn create(
    domain: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> Result<i64> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    // Get site_id if it exists
    let site_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM sites WHERE domain = ?1",
            rusqlite::params![domain],
            |row| row.get(0),
        )
        .ok();

    // Hash password (in production, use proper encryption)
    let password_hash = STANDARD.encode(db_password);

    conn.execute(
        "INSERT INTO databases (site_id, domain, db_name, db_user, db_password_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![site_id, domain, db_name, db_user, password_hash],
    )?;

    Ok(conn.last_insert_rowid())
}

pub async fn get_by_domain(domain: &str) -> Result<Option<Database>> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let result = conn.query_row(
        "SELECT id, site_id, domain, db_name, db_user, db_type, created_at FROM databases WHERE domain = ?1",
        rusqlite::params![domain],
        |row| {
            Ok(Database {
                id: row.get(0)?,
                site_id: row.get(1)?,
                domain: row.get(2)?,
                db_name: row.get(3)?,
                db_user: row.get(4)?,
                db_type: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    );

    match result {
        Ok(db) => Ok(Some(db)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn get_password(domain: &str) -> Result<Option<String>> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let result: Result<String, _> = conn.query_row(
        "SELECT db_password_hash FROM databases WHERE domain = ?1",
        rusqlite::params![domain],
        |row| row.get(0),
    );

    match result {
        Ok(hash) => {
            // Decode password (in production, use proper decryption)
            let password = String::from_utf8(STANDARD.decode(&hash)?)?;
            Ok(Some(password))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn delete(domain: &str) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        "DELETE FROM databases WHERE domain = ?1",
        rusqlite::params![domain],
    )?;

    Ok(())
}

pub async fn create_staging(domain: &str, staging_domain: &str) -> Result<i64> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    // Get production database info
    let (db_name, db_user, password_hash): (String, String, String) = conn.query_row(
        "SELECT db_name, db_user, db_password_hash FROM databases WHERE domain = ?1 AND db_type = 'production'",
        rusqlite::params![domain],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    // Create staging database record
    let staging_db_name = format!("{}_staging", db_name);
    let staging_db_user = format!("{}_staging", db_user);

    conn.execute(
        "INSERT INTO databases (domain, db_name, db_user, db_password_hash, db_type) VALUES (?1, ?2, ?3, ?4, 'staging')",
        rusqlite::params![staging_domain, staging_db_name, staging_db_user, password_hash],
    )?;

    Ok(conn.last_insert_rowid())
}
