use anyhow::Result;

#[derive(Debug, Clone)]
pub struct StagingSite {
    pub id: i64,
    pub production_site_id: i64,
    pub staging_subdomain: String,
    pub staging_domain: String,
    pub created_at: String,
    pub last_sync_at: Option<String>,
    pub last_sync_direction: Option<String>,
}

pub async fn create(
    production_domain: &str,
    staging_subdomain: &str,
) -> Result<i64> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    // Get production site ID
    let production_site_id: i64 = conn.query_row(
        "SELECT id FROM sites WHERE domain = ?1",
        rusqlite::params![production_domain],
        |row| row.get(0),
    )?;

    let staging_domain = format!("{}.{}", staging_subdomain, production_domain);

    conn.execute(
        "INSERT INTO staging_sites (production_site_id, staging_subdomain, staging_domain) VALUES (?1, ?2, ?3)",
        rusqlite::params![production_site_id, staging_subdomain, staging_domain],
    )?;

    Ok(conn.last_insert_rowid())
}

pub async fn get_by_production(production_domain: &str) -> Result<Option<StagingSite>> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let result = conn.query_row(
        r#"
        SELECT s.id, s.production_site_id, s.staging_subdomain, s.staging_domain,
               s.created_at, s.last_sync_at, s.last_sync_direction
        FROM staging_sites s
        JOIN sites p ON s.production_site_id = p.id
        WHERE p.domain = ?1
        "#,
        rusqlite::params![production_domain],
        |row| {
            Ok(StagingSite {
                id: row.get(0)?,
                production_site_id: row.get(1)?,
                staging_subdomain: row.get(2)?,
                staging_domain: row.get(3)?,
                created_at: row.get(4)?,
                last_sync_at: row.get(5)?,
                last_sync_direction: row.get(6)?,
            })
        },
    );

    match result {
        Ok(staging) => Ok(Some(staging)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn list() -> Result<Vec<StagingSite>> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let mut stmt = conn.prepare(
        r#"
        SELECT id, production_site_id, staging_subdomain, staging_domain,
               created_at, last_sync_at, last_sync_direction
        FROM staging_sites
        ORDER BY staging_domain
        "#,
    )?;

    let staging_sites = stmt
        .query_map([], |row| {
            Ok(StagingSite {
                id: row.get(0)?,
                production_site_id: row.get(1)?,
                staging_subdomain: row.get(2)?,
                staging_domain: row.get(3)?,
                created_at: row.get(4)?,
                last_sync_at: row.get(5)?,
                last_sync_direction: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(staging_sites)
}

pub async fn delete(production_domain: &str) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        r#"
        DELETE FROM staging_sites
        WHERE production_site_id = (SELECT id FROM sites WHERE domain = ?1)
        "#,
        rusqlite::params![production_domain],
    )?;

    Ok(())
}

pub async fn update_sync(
    production_domain: &str,
    direction: &str,
) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        r#"
        UPDATE staging_sites
        SET last_sync_at = CURRENT_TIMESTAMP, last_sync_direction = ?1
        WHERE production_site_id = (SELECT id FROM sites WHERE domain = ?2)
        "#,
        rusqlite::params![direction, production_domain],
    )?;

    Ok(())
}
