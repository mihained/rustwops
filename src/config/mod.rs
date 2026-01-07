pub mod nginx;
pub mod php;
pub mod security;
pub mod stack;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CONFIG_PATH: &str = "/etc/rustwops/config.toml";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub nginx: NginxConfig,
    #[serde(default)]
    pub php: PhpConfig,
    #[serde(default)]
    pub mysql: MysqlConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub nodejs: NodejsConfig,
    #[serde(default)]
    pub ssl: SslConfig,
    #[serde(default)]
    pub backup: BackupConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub default_php_version: String,
    pub default_site_type: String,
    pub webroot_base: String,
    pub log_level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_php_version: "8.3".to_string(),
            default_site_type: "php".to_string(),
            webroot_base: "/var/www".to_string(),
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NginxConfig {
    pub custom_build: bool,
    pub worker_processes: String,
    pub worker_connections: u32,
    pub brotli: bool,
    pub http3: bool,
}

impl Default for NginxConfig {
    fn default() -> Self {
        Self {
            custom_build: false,
            worker_processes: "auto".to_string(),
            worker_connections: 4096,
            brotli: false,
            http3: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhpConfig {
    pub installed_versions: Vec<String>,
    pub pm_mode: String,
    pub pm_max_children: u32,
    pub pm_start_servers: u32,
    pub pm_min_spare_servers: u32,
    pub pm_max_spare_servers: u32,
}

impl Default for PhpConfig {
    fn default() -> Self {
        Self {
            installed_versions: vec!["8.3".to_string()],
            pm_mode: "dynamic".to_string(),
            pm_max_children: 10,
            pm_start_servers: 2,
            pm_min_spare_servers: 1,
            pm_max_spare_servers: 3,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MysqlConfig {
    pub db_type: String,
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            db_type: "mariadb".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisConfig {
    pub port: u16,
    pub maxmemory: String,
    pub maxmemory_policy: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            port: 6379,
            maxmemory: "256mb".to_string(),
            maxmemory_policy: "allkeys-lru".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodejsConfig {
    pub default_version: String,
    pub installed_versions: Vec<String>,
}

impl Default for NodejsConfig {
    fn default() -> Self {
        Self {
            default_version: "20".to_string(),
            installed_versions: vec!["20".to_string()],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SslConfig {
    pub default_key_type: String,
    pub renew_days_before: u32,
    pub ocsp_stapling: bool,
    pub hsts: bool,
    pub hsts_max_age: u32,
}

impl Default for SslConfig {
    fn default() -> Self {
        Self {
            default_key_type: "ec-384".to_string(),
            renew_days_before: 30,
            ocsp_stapling: true,
            hsts: true,
            hsts_max_age: 31536000,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupConfig {
    pub directory: String,
    pub retention_days: u32,
    pub compression_level: u32,
    #[serde(default)]
    pub s3: S3Config,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            directory: "/var/lib/rustwops/backups".to_string(),
            retention_days: 30,
            compression_level: 6,
            s3: S3Config::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
}

impl Config {
    /// Load configuration from file
    pub async fn load() -> Result<Self> {
        if Path::new(CONFIG_PATH).exists() {
            let content = tokio::fs::read_to_string(CONFIG_PATH).await?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save configuration to file
    pub async fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        tokio::fs::write(CONFIG_PATH, content).await?;
        Ok(())
    }
}

/// Load or create default configuration
pub async fn load_or_create() -> Result<Config> {
    if Path::new(CONFIG_PATH).exists() {
        Config::load().await
    } else {
        let config = Config::default();
        // Ensure directory exists
        tokio::fs::create_dir_all("/etc/rustwops").await?;
        config.save().await?;
        Ok(config)
    }
}
