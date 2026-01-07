use anyhow::Result;
use colored::Colorize;

use super::{CacheType, DnsProvider, SiteType};
use crate::config::nginx;
use crate::config::php;
use crate::database;
use crate::utils::{password, shell};
use crate::Cli;

pub async fn execute(
    domain: &str,
    site_type: SiteType,
    php_version: &str,
    mysql: bool,
    cache: Option<CacheType>,
    ssl: bool,
    wildcard: bool,
    dns: Option<DnsProvider>,
    upstream: Option<u16>,
    cli: &Cli,
) -> Result<()> {
    println!(
        "{} Creating site: {}\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Validate domain
    validate_domain(domain)?;

    // Validate PHP version is installed (for PHP/WP sites)
    if matches!(site_type, SiteType::Wp | SiteType::Php) {
        validate_php_version(php_version).await?;
    }

    // Check if domain already exists
    if database::sites::exists(domain).await? {
        anyhow::bail!("Site {} already exists", domain);
    }

    // Create directory structure
    create_directories(domain).await?;
    println!("  {} Created directory structure", "✓".green());

    // Create database if needed
    let db_info = if mysql || matches!(site_type, SiteType::Wp) {
        Some(create_database(domain).await?)
    } else {
        None
    };

    if db_info.is_some() {
        println!("  {} Created MySQL database", "✓".green());
    }

    // Generate PHP-FPM pool (for PHP/WP sites)
    if matches!(site_type, SiteType::Wp | SiteType::Php) {
        php::create_pool(domain, php_version).await?;
        println!("  {} Created PHP-FPM pool", "✓".green());
    }

    // Generate Nginx config
    let webroot = format!("/var/www/{}/prod/public", domain);
    nginx::create_site_config(domain, site_type, php_version, cache, &webroot, upstream).await?;
    println!("  {} Created Nginx configuration", "✓".green());

    // Install WordPress if WP type
    if matches!(site_type, SiteType::Wp) {
        if let Some(ref db) = db_info {
            install_wordpress(domain, &webroot, db, cli.verbose).await?;
        }
    }

    // Create default index.php for PHP sites (not WordPress)
    if matches!(site_type, SiteType::Php) {
        create_default_php_index(domain).await?;
    }

    // Create Node.js app structure if Node type
    if matches!(site_type, SiteType::Node) {
        create_node_app(domain).await?;
        println!("  {} Created Node.js app structure", "✓".green());
    }

    // Set file permissions
    set_permissions(domain).await?;
    println!("  {} Set file permissions", "✓".green());

    // Enable site
    enable_site(domain).await?;
    println!("  {} Enabled site", "✓".green());

    // Reload services
    reload_services(site_type, php_version).await?;
    println!("  {} Reloaded services", "✓".green());

    // Issue SSL certificate if requested
    if ssl {
        if wildcard {
            let provider = dns.ok_or_else(|| {
                anyhow::anyhow!("DNS provider required for wildcard SSL (use --dns)")
            })?;
            issue_wildcard_ssl(domain, provider, cli.verbose).await?;
        } else {
            issue_ssl(domain, cli.verbose).await?;
        }
        println!("  {} Issued SSL certificate", "✓".green());
    }

    // Store site info in database
    database::sites::create(domain, site_type, php_version, cache).await?;

    // Print summary
    print_summary(domain, site_type, &db_info, ssl);

    Ok(())
}

fn validate_domain(domain: &str) -> Result<()> {
    let domain_regex = regex::Regex::new(
        r"^([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$",
    )?;

    if !domain_regex.is_match(domain) {
        anyhow::bail!("Invalid domain format: {}", domain);
    }

    Ok(())
}

async fn validate_php_version(php_version: &str) -> Result<()> {
    // Check if PHP-FPM service exists for this version
    let service = format!("php{}-fpm", php_version);
    let result = shell::run_command("systemctl", &["is-enabled", &service]).await;

    if result.is_err() {
        anyhow::bail!(
            "PHP {} FPM is not installed. Install it with: rw stack install php --php-version {}",
            php_version,
            php_version
        );
    }

    Ok(())
}

async fn create_directories(domain: &str) -> Result<()> {
    let dirs = [
        format!("/var/www/{}/prod/public", domain),
        format!("/var/www/{}/logs", domain),
    ];

    for dir in &dirs {
        shell::run_command("mkdir", &["-p", dir]).await?;
    }

    Ok(())
}

async fn create_default_php_index(domain: &str) -> Result<()> {
    let index_path = format!("/var/www/{}/prod/public/index.php", domain);
    let content = r#"<?php
// Default PHP site - Replace this file with your application
phpinfo();
"#;
    tokio::fs::write(&index_path, content).await?;
    Ok(())
}

struct DbInfo {
    name: String,
    user: String,
    password: String,
}

async fn create_database(domain: &str) -> Result<DbInfo> {
    let db_name = domain.replace('.', "_").replace('-', "_");
    let db_user = db_name.clone();
    let db_password = password::generate(32);

    // Create database and user
    let sql = format!(
        r#"
        CREATE DATABASE IF NOT EXISTS `{}`;
        CREATE USER IF NOT EXISTS '{}'@'localhost' IDENTIFIED BY '{}';
        GRANT ALL PRIVILEGES ON `{}`.* TO '{}'@'localhost';
        FLUSH PRIVILEGES;
        "#,
        db_name, db_user, db_password, db_name, db_user
    );

    shell::run_command("mysql", &["-e", &sql]).await?;

    // Store in RustWops database
    database::databases::create(domain, &db_name, &db_user, &db_password).await?;

    Ok(DbInfo {
        name: db_name,
        user: db_user,
        password: db_password,
    })
}

async fn install_wordpress(domain: &str, webroot: &str, db: &DbInfo, verbose: bool) -> Result<()> {
    use std::io::{self, Write};

    // Download WordPress
    print!("  {} Downloading WordPress...", "→".bright_cyan());
    io::stdout().flush().ok();
    shell::run_command_with_output(
        "wp",
        &["core", "download", &format!("--path={}", webroot), "--allow-root"],
        verbose,
    )
    .await?;
    println!(" {}", "done".green());

    // Create wp-config.php
    print!("  {} Creating wp-config.php...", "→".bright_cyan());
    io::stdout().flush().ok();
    shell::run_command_with_output(
        "wp",
        &[
            "config",
            "create",
            &format!("--path={}", webroot),
            &format!("--dbname={}", db.name),
            &format!("--dbuser={}", db.user),
            &format!("--dbpass={}", db.password),
            "--dbhost=localhost",
            "--allow-root",
        ],
        verbose,
    )
    .await?;
    println!(" {}", "done".green());

    // Install WordPress
    print!("  {} Installing WordPress core...", "→".bright_cyan());
    io::stdout().flush().ok();
    let admin_password = password::generate(16);
    shell::run_command_with_output(
        "wp",
        &[
            "core",
            "install",
            &format!("--path={}", webroot),
            &format!("--url=http://{}", domain),
            &format!("--title={}", domain),
            "--admin_user=admin",
            &format!("--admin_password={}", admin_password),
            &format!("--admin_email=admin@{}", domain),
            "--skip-email",
            "--allow-root",
        ],
        verbose,
    )
    .await?;
    println!(" {}", "done".green());

    // Store admin password (we'll show it to user)
    println!(
        "\n  {} WordPress admin password: {}",
        "→".bright_cyan(),
        admin_password.bright_yellow()
    );

    Ok(())
}

async fn create_node_app(domain: &str) -> Result<()> {
    let app_dir = format!("/var/www/{}/prod", domain);

    // Create package.json
    let package_json = format!(
        r#"{{
  "name": "{}",
  "version": "1.0.0",
  "main": "app.js",
  "scripts": {{
    "start": "node app.js"
  }}
}}"#,
        domain.replace('.', "-")
    );

    shell::write_file(&format!("{}/package.json", app_dir), &package_json).await?;

    // Create basic app.js
    let app_js = r#"const http = require('http');
const port = process.env.PORT || 3000;

const server = http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/plain' });
  res.end('Hello from Node.js!\n');
});

server.listen(port, () => {
  console.log(`Server running on port ${port}`);
});
"#;

    shell::write_file(&format!("{}/app.js", app_dir), app_js).await?;

    // Create PM2 ecosystem file
    let ecosystem = format!(
        r#"module.exports = {{
  apps: [{{
    name: '{}',
    script: 'app.js',
    cwd: '{}',
    instances: 'max',
    exec_mode: 'cluster',
    env: {{
      NODE_ENV: 'production',
      PORT: 3000
    }}
  }}]
}};
"#,
        domain, app_dir
    );

    shell::write_file(&format!("{}/ecosystem.config.js", app_dir), &ecosystem).await?;

    Ok(())
}

async fn set_permissions(domain: &str) -> Result<()> {
    let webroot = format!("/var/www/{}", domain);

    // Try to set ownership to www-data, fall back to root if www-data doesn't exist
    if shell::run_command("chown", &["-R", "www-data:www-data", &webroot]).await.is_err() {
        shell::run_command("chown", &["-R", "root:root", &webroot]).await?;
    }

    shell::run_command("find", &[&webroot, "-type", "d", "-exec", "chmod", "755", "{}", "+"]).await?;
    shell::run_command("find", &[&webroot, "-type", "f", "-exec", "chmod", "644", "{}", "+"]).await?;

    Ok(())
}

async fn enable_site(domain: &str) -> Result<()> {
    let available = format!("/etc/nginx/sites-available/{}", domain);
    let enabled = format!("/etc/nginx/sites-enabled/{}", domain);

    shell::run_command("ln", &["-sf", &available, &enabled]).await?;

    Ok(())
}

async fn reload_services(site_type: SiteType, php_version: &str) -> Result<()> {
    // Test Nginx config (if nginx is installed)
    if shell::command_exists("nginx").await {
        if let Err(e) = shell::run_command("nginx", &["-t"]).await {
            eprintln!("  {} Nginx config test failed: {}", "⚠".yellow(), e);
        } else {
            // Reload Nginx
            let _ = shell::run_command("systemctl", &["reload", "nginx"]).await;
        }
    }

    // Reload PHP-FPM if applicable
    if matches!(site_type, SiteType::Wp | SiteType::Php) {
        let fpm = format!("php{}-fpm", php_version);
        let _ = shell::run_command("systemctl", &["reload", &fpm]).await;
    }

    Ok(())
}

async fn issue_ssl(domain: &str, verbose: bool) -> Result<()> {
    crate::commands::ssl::issue::execute_http(
        domain,
        crate::commands::ssl::KeyType::default(),
        false, // staging
        verbose,
    ).await
}

async fn issue_wildcard_ssl(domain: &str, provider: DnsProvider, verbose: bool) -> Result<()> {
    // Convert site::DnsProvider to ssl::DnsProvider
    let ssl_provider = match provider {
        DnsProvider::Cloudflare => crate::commands::ssl::DnsProvider::Cloudflare,
        DnsProvider::Digitalocean => crate::commands::ssl::DnsProvider::Digitalocean,
        DnsProvider::Route53 => crate::commands::ssl::DnsProvider::Route53,
    };
    crate::commands::ssl::issue::execute_dns(
        domain,
        ssl_provider,
        crate::commands::ssl::KeyType::default(),
        false, // staging
        verbose,
    ).await
}

fn print_summary(domain: &str, site_type: SiteType, db: &Option<DbInfo>, ssl: bool) {
    println!("\n{}", "━".repeat(50).dimmed());
    println!(
        "\n{} Site created successfully!\n",
        "✓".green().bold()
    );

    let protocol = if ssl { "https" } else { "http" };
    println!("  {} URL: {}://{}", "→".bright_cyan(), protocol, domain);
    println!("  {} Type: {:?}", "→".bright_cyan(), site_type);
    println!(
        "  {} Webroot: /var/www/{}/prod/public",
        "→".bright_cyan(),
        domain
    );

    if let Some(db) = db {
        println!("\n  {} Database:", "→".bright_cyan());
        println!("    Name: {}", db.name);
        println!("    User: {}", db.user);
        println!("    Password: {}", db.password.bright_yellow());
    }

    if matches!(site_type, SiteType::Wp) {
        println!("\n  {} WordPress:", "→".bright_cyan());
        println!("    Admin URL: {}://{}/wp-admin", protocol, domain);
        println!("    Username: admin");
    }

    println!();
}
