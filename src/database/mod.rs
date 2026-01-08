pub mod backups;
pub mod databases;
pub mod sites;
pub mod staging;

use anyhow::Result;
use rusqlite::Connection;
use std::sync::OnceLock;
use tokio::sync::Mutex;

const DB_PATH: &str = "/var/lib/rustwops/sites.db";

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Initialize the database
pub async fn init() -> Result<()> {
    // Ensure directory exists
    tokio::fs::create_dir_all("/var/lib/rustwops").await?;

    let conn = Connection::open(DB_PATH)?;

    // Run migrations
    conn.execute_batch(SCHEMA)?;

    DB.set(Mutex::new(conn))
        .map_err(|_| anyhow::anyhow!("Database already initialized"))?;

    Ok(())
}

/// Get database connection (auto-initializes if needed)
pub fn get_connection() -> Result<&'static Mutex<Connection>> {
    // Auto-initialize if not already done
    if DB.get().is_none() {
        // Ensure directory exists
        std::fs::create_dir_all("/var/lib/rustwops")?;

        let conn = Connection::open(DB_PATH)?;
        conn.execute_batch(SCHEMA)?;

        let _ = DB.set(Mutex::new(conn));
    }

    DB.get()
        .ok_or_else(|| anyhow::anyhow!("Database initialization failed"))
}

/// Initialize database if needed (for testing or first run)
pub async fn ensure_initialized() -> Result<()> {
    if DB.get().is_none() {
        init().await?;
    }
    Ok(())
}

const SCHEMA: &str = r#"
-- Core sites table
CREATE TABLE IF NOT EXISTS sites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT UNIQUE NOT NULL,
    site_type TEXT NOT NULL CHECK (site_type IN ('wp', 'php', 'static', 'proxy', 'node')),
    php_version TEXT,
    cache_type TEXT CHECK (cache_type IN ('none', 'fastcgi', 'redis') OR cache_type IS NULL),
    webroot TEXT NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    has_ssl BOOLEAN DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- SSL certificates
CREATE TABLE IF NOT EXISTS certificates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    domains TEXT NOT NULL,
    is_wildcard BOOLEAN DEFAULT FALSE,
    dns_provider TEXT,
    key_type TEXT DEFAULT 'ec-384',
    issued_at DATETIME,
    expires_at DATETIME,
    auto_renew BOOLEAN DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Staging environments
CREATE TABLE IF NOT EXISTS staging_sites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    production_site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    staging_subdomain TEXT NOT NULL DEFAULT 'staging',
    staging_domain TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_sync_at DATETIME,
    last_sync_direction TEXT CHECK (last_sync_direction IN ('prod_to_stage', 'stage_to_prod'))
);

-- Databases
CREATE TABLE IF NOT EXISTS databases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER REFERENCES sites(id) ON DELETE SET NULL,
    domain TEXT NOT NULL,
    db_name TEXT NOT NULL,
    db_user TEXT NOT NULL,
    db_password_hash TEXT NOT NULL,
    db_type TEXT DEFAULT 'production' CHECK (db_type IN ('production', 'staging')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Backups
CREATE TABLE IF NOT EXISTS backups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER REFERENCES sites(id) ON DELETE SET NULL,
    domain TEXT NOT NULL,
    backup_name TEXT,
    file_path TEXT NOT NULL,
    file_size INTEGER,
    includes_db BOOLEAN DEFAULT TRUE,
    includes_files BOOLEAN DEFAULT TRUE,
    metadata TEXT,
    storage_type TEXT DEFAULT 'local' CHECK (storage_type IN ('local', 's3')),
    s3_url TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Configuration key-value store
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    encrypted BOOLEAN DEFAULT FALSE,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Audit log
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    target_type TEXT,
    target_id INTEGER,
    details TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_sites_domain ON sites(domain);
CREATE INDEX IF NOT EXISTS idx_sites_type ON sites(site_type);
CREATE INDEX IF NOT EXISTS idx_certificates_site ON certificates(site_id);
CREATE INDEX IF NOT EXISTS idx_staging_production ON staging_sites(production_site_id);
CREATE INDEX IF NOT EXISTS idx_backups_site ON backups(site_id);
CREATE INDEX IF NOT EXISTS idx_backups_created ON backups(created_at);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at);
"#;
