use anyhow::Result;

use crate::commands::site::{CacheType, SiteType};

#[derive(Debug, Clone)]
pub struct Site {
    pub id: i64,
    pub domain: String,
    pub site_type: String,
    pub php_version: Option<String>,
    pub cache_type: Option<String>,
    pub webroot: String,
    pub enabled: bool,
    pub has_ssl: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn create(
    domain: &str,
    site_type: SiteType,
    php_version: &str,
    cache: Option<CacheType>,
) -> Result<i64> {
    let webroot = format!("/var/www/{}/prod/public", domain);
    create_with_webroot(domain, site_type, php_version, cache, &webroot).await
}

pub async fn create_with_webroot(
    domain: &str,
    site_type: SiteType,
    php_version: &str,
    cache: Option<CacheType>,
    webroot: &str,
) -> Result<i64> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let cache_str = cache.map(|c| match c {
        CacheType::None => "none",
        CacheType::Fastcgi => "fastcgi",
        CacheType::Redis => "redis",
    });

    conn.execute(
        "INSERT INTO sites (domain, site_type, php_version, cache_type, webroot) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![domain, site_type.to_string(), php_version, cache_str, webroot],
    )?;

    Ok(conn.last_insert_rowid())
}

pub async fn get(domain: &str) -> Result<Site> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let site = conn.query_row(
        "SELECT id, domain, site_type, php_version, cache_type, webroot, enabled, has_ssl, created_at, updated_at FROM sites WHERE domain = ?1",
        rusqlite::params![domain],
        |row| {
            Ok(Site {
                id: row.get(0)?,
                domain: row.get(1)?,
                site_type: row.get(2)?,
                php_version: row.get(3)?,
                cache_type: row.get(4)?,
                webroot: row.get(5)?,
                enabled: row.get(6)?,
                has_ssl: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )?;

    Ok(site)
}

pub async fn get_by_domain(domain: &str) -> Result<Option<Site>> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let result = conn.query_row(
        "SELECT id, domain, site_type, php_version, cache_type, webroot, enabled, has_ssl, created_at, updated_at FROM sites WHERE domain = ?1",
        rusqlite::params![domain],
        |row| {
            Ok(Site {
                id: row.get(0)?,
                domain: row.get(1)?,
                site_type: row.get(2)?,
                php_version: row.get(3)?,
                cache_type: row.get(4)?,
                webroot: row.get(5)?,
                enabled: row.get(6)?,
                has_ssl: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    );

    match result {
        Ok(site) => Ok(Some(site)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn exists(domain: &str) -> Result<bool> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sites WHERE domain = ?1",
        rusqlite::params![domain],
        |row| row.get(0),
    )?;

    Ok(count > 0)
}

pub async fn list() -> Result<Vec<Site>> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let mut stmt = conn.prepare(
        "SELECT id, domain, site_type, php_version, cache_type, webroot, enabled, has_ssl, created_at, updated_at FROM sites ORDER BY domain",
    )?;

    let sites = stmt
        .query_map([], |row| {
            Ok(Site {
                id: row.get(0)?,
                domain: row.get(1)?,
                site_type: row.get(2)?,
                php_version: row.get(3)?,
                cache_type: row.get(4)?,
                webroot: row.get(5)?,
                enabled: row.get(6)?,
                has_ssl: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(sites)
}

pub async fn delete(domain: &str) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        "DELETE FROM sites WHERE domain = ?1",
        rusqlite::params![domain],
    )?;

    Ok(())
}

pub async fn update_ssl(domain: &str, has_ssl: bool) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        "UPDATE sites SET has_ssl = ?1, updated_at = CURRENT_TIMESTAMP WHERE domain = ?2",
        rusqlite::params![has_ssl, domain],
    )?;

    Ok(())
}

pub async fn update_enabled(domain: &str, enabled: bool) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        "UPDATE sites SET enabled = ?1, updated_at = CURRENT_TIMESTAMP WHERE domain = ?2",
        rusqlite::params![enabled, domain],
    )?;

    Ok(())
}

pub async fn update_php_version(domain: &str, php_version: &str) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    conn.execute(
        "UPDATE sites SET php_version = ?1, updated_at = CURRENT_TIMESTAMP WHERE domain = ?2",
        rusqlite::params![php_version, domain],
    )?;

    Ok(())
}

pub async fn update_cache(domain: &str, cache_type: Option<CacheType>) -> Result<()> {
    let conn = super::get_connection()?;
    let conn = conn.lock().await;

    let cache_str = cache_type.map(|c| match c {
        CacheType::None => "none",
        CacheType::Fastcgi => "fastcgi",
        CacheType::Redis => "redis",
    });

    conn.execute(
        "UPDATE sites SET cache_type = ?1, updated_at = CURRENT_TIMESTAMP WHERE domain = ?2",
        rusqlite::params![cache_str, domain],
    )?;

    Ok(())
}
