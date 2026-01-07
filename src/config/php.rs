use crate::utils::shell;
use anyhow::Result;

const POOL_DIR_TEMPLATE: &str = "/etc/php/{version}/fpm/pool.d";

/// Detect installed PHP-FPM versions and return the latest one
pub async fn detect_latest_version() -> Result<String> {
    let versions = ["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];

    for version in versions {
        let service = format!("php{}-fpm", version);
        if shell::run_command("systemctl", &["is-enabled", &service])
            .await
            .is_ok()
        {
            return Ok(version.to_string());
        }
    }

    anyhow::bail!("No PHP-FPM version installed. Run 'rw stack install php' first.")
}

/// Get list of all installed PHP-FPM versions
pub async fn get_installed_versions() -> Vec<String> {
    let versions = ["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];
    let mut installed = Vec::new();

    for version in versions {
        let service = format!("php{}-fpm", version);
        if shell::run_command("systemctl", &["is-enabled", &service])
            .await
            .is_ok()
        {
            installed.push(version.to_string());
        }
    }

    installed
}

pub async fn create_pool(domain: &str, php_version: &str) -> Result<()> {
    create_pool_with_webroot(domain, php_version, None).await
}

pub async fn create_pool_with_webroot(
    domain: &str,
    php_version: &str,
    webroot: Option<&str>,
) -> Result<()> {
    let pool_dir = POOL_DIR_TEMPLATE.replace("{version}", php_version);
    let pool_path = format!("{}/{}.conf", pool_dir, domain);

    // Ensure pool directory exists
    tokio::fs::create_dir_all(&pool_dir).await?;

    // Also ensure log directory exists
    let log_dir = format!("/var/log/php{}-fpm", php_version);
    tokio::fs::create_dir_all(&log_dir).await?;

    let config = generate_pool_config(domain, php_version, webroot);
    tokio::fs::write(&pool_path, config).await?;

    Ok(())
}

fn generate_pool_config(domain: &str, php_version: &str, custom_webroot: Option<&str>) -> String {
    let webroot = custom_webroot
        .map(|w| w.to_string())
        .unwrap_or_else(|| format!("/var/www/{}/prod", domain));

    format!(
        r#"; RustWops managed - {domain}
; Generated for PHP {php_version}

[{domain}]
user = www-data
group = www-data

listen = /run/php/php{php_version}-fpm-{domain}.sock
listen.owner = www-data
listen.group = www-data
listen.mode = 0660

pm = dynamic
pm.max_children = 10
pm.start_servers = 2
pm.min_spare_servers = 1
pm.max_spare_servers = 3
pm.max_requests = 500
pm.status_path = /status

chdir = {webroot}

; Logging
access.log = /var/log/php{php_version}-fpm/{domain}.access.log
slowlog = /var/log/php{php_version}-fpm/{domain}.slow.log
request_slowlog_timeout = 5s

; Security
security.limit_extensions = .php

; Environment
env[PATH] = /usr/local/bin:/usr/bin:/bin
env[TMP] = /tmp
env[TMPDIR] = /tmp
env[TEMP] = /tmp

; PHP settings
php_admin_value[error_log] = /var/log/php{php_version}-fpm/{domain}.error.log
php_admin_flag[log_errors] = on
php_value[session.save_handler] = files
php_value[upload_max_filesize] = 64M
php_value[post_max_size] = 64M
php_value[memory_limit] = 256M
php_value[max_execution_time] = 300
php_value[max_input_vars] = 3000
"#
    )
}

pub async fn delete_pool(domain: &str, php_version: &str) -> Result<()> {
    let pool_dir = POOL_DIR_TEMPLATE.replace("{version}", php_version);
    let pool_path = format!("{}/{}.conf", pool_dir, domain);

    if tokio::fs::metadata(&pool_path).await.is_ok() {
        tokio::fs::remove_file(&pool_path).await?;
    }

    Ok(())
}

pub async fn update_pool_setting(
    domain: &str,
    php_version: &str,
    setting: &str,
    value: &str,
) -> Result<()> {
    let pool_dir = POOL_DIR_TEMPLATE.replace("{version}", php_version);
    let pool_path = format!("{}/{}.conf", pool_dir, domain);

    let content = tokio::fs::read_to_string(&pool_path).await?;

    // Find and replace the setting
    let mut new_lines: Vec<String> = Vec::new();
    let mut found = false;

    for line in content.lines() {
        if line.starts_with(setting) || line.starts_with(&format!("php_value[{}]", setting)) {
            new_lines.push(format!("{} = {}", setting, value));
            found = true;
        } else {
            new_lines.push(line.to_string());
        }
    }

    if !found {
        // Add the setting at the end
        new_lines.push(format!("{} = {}", setting, value));
    }

    tokio::fs::write(&pool_path, new_lines.join("\n")).await?;

    Ok(())
}
