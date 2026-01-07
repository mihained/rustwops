use anyhow::Result;
use colored::Colorize;

use crate::database;
use crate::utils::shell;
use crate::Cli;

pub async fn execute(domain: &str, _cli: &Cli) -> Result<()> {
    // Get site from database
    let site = database::sites::get(domain).await?;

    println!(
        "\n{} Site Information: {}\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Basic info
    println!("  {} General:", "●".bright_cyan());
    println!("    Domain:   {}", site.domain);
    println!("    Type:     {}", site.site_type);
    println!(
        "    Status:   {}",
        if site.enabled {
            "enabled".green()
        } else {
            "disabled".yellow()
        }
    );
    println!("    Webroot:  {}", site.webroot);
    println!("    Created:  {}", site.created_at);

    // PHP info
    if let Some(ref php_version) = site.php_version {
        println!("\n  {} PHP:", "●".bright_cyan());
        println!("    Version:  {}", php_version);
        println!(
            "    Pool:     /etc/php/{}/fpm/pool.d/{}.conf",
            php_version, domain
        );
        println!(
            "    Socket:   /run/php/php{}-fpm-{}.sock",
            php_version, domain
        );
    }

    // Cache info
    if let Some(ref cache_type) = site.cache_type {
        println!("\n  {} Cache:", "●".bright_cyan());
        println!("    Type:     {}", cache_type);
    }

    // SSL info
    if site.has_ssl {
        println!("\n  {} SSL:", "●".bright_cyan());
        if let Ok(cert_info) = get_ssl_info(domain).await {
            println!("    Certificate: {}", cert_info.cert_path);
            println!("    Expires:     {}", cert_info.expires);
            println!(
                "    Wildcard:    {}",
                if cert_info.is_wildcard { "yes" } else { "no" }
            );
        }
    }

    // Database info
    if let Ok(Some(db)) = database::databases::get_by_domain(domain).await {
        println!("\n  {} Database:", "●".bright_cyan());
        println!("    Name:     {}", db.db_name);
        println!("    User:     {}", db.db_user);
    }

    // Staging info
    if let Ok(Some(staging)) = database::staging::get_by_production(domain).await {
        println!("\n  {} Staging:", "●".bright_cyan());
        println!("    Domain:   {}", staging.staging_domain);
        if let Some(last_sync) = staging.last_sync_at {
            println!("    Last Sync: {}", last_sync);
        }
    }

    // Disk usage
    println!("\n  {} Disk Usage:", "●".bright_cyan());
    if let Ok(usage) = get_disk_usage(domain).await {
        println!("    Files:    {}", usage);
    }

    // WordPress specific info
    if site.site_type == "wp" {
        if let Ok(wp_info) = get_wordpress_info(domain).await {
            println!("\n  {} WordPress:", "●".bright_cyan());
            println!("    Version:  {}", wp_info.version);
            println!("    Plugins:  {}", wp_info.plugin_count);
            println!("    Themes:   {}", wp_info.theme_count);
        }
    }

    println!();

    Ok(())
}

struct SslInfo {
    cert_path: String,
    expires: String,
    is_wildcard: bool,
}

async fn get_ssl_info(domain: &str) -> Result<SslInfo> {
    let cert_path = format!("/etc/ssl/rustwops/{}/fullchain.pem", domain);

    // Get expiry date
    let expires = shell::run_command(
        "openssl",
        &["x509", "-enddate", "-noout", "-in", &cert_path],
    )
    .await
    .map(|o| o.replace("notAfter=", "").trim().to_string())
    .unwrap_or_else(|_| "unknown".to_string());

    // Check if wildcard
    let subject = shell::run_command(
        "openssl",
        &["x509", "-subject", "-noout", "-in", &cert_path],
    )
    .await
    .unwrap_or_default();
    let is_wildcard = subject.contains("*.");

    Ok(SslInfo {
        cert_path,
        expires,
        is_wildcard,
    })
}

async fn get_disk_usage(domain: &str) -> Result<String> {
    let webroot = format!("/var/www/{}", domain);
    let output = shell::run_command("du", &["-sh", &webroot]).await?;
    Ok(output
        .split_whitespace()
        .next()
        .unwrap_or("unknown")
        .to_string())
}

struct WordPressInfo {
    version: String,
    plugin_count: usize,
    theme_count: usize,
}

async fn get_wordpress_info(domain: &str) -> Result<WordPressInfo> {
    let webroot = format!("/var/www/{}/prod/public", domain);

    // Get WP version
    let version = shell::run_command(
        "wp",
        &[
            "core",
            "version",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
    )
    .await
    .map(|o| o.trim().to_string())
    .unwrap_or_else(|_| "unknown".to_string());

    // Count plugins
    let plugins = shell::run_command(
        "wp",
        &[
            "plugin",
            "list",
            "--format=count",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
    )
    .await
    .map(|o| o.trim().parse().unwrap_or(0))
    .unwrap_or(0);

    // Count themes
    let themes = shell::run_command(
        "wp",
        &[
            "theme",
            "list",
            "--format=count",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
    )
    .await
    .map(|o| o.trim().parse().unwrap_or(0))
    .unwrap_or(0);

    Ok(WordPressInfo {
        version,
        plugin_count: plugins,
        theme_count: themes,
    })
}
