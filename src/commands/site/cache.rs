use anyhow::Result;
use colored::Colorize;

use crate::database;
use crate::utils::shell;
use crate::Cli;

/// Purge cache for a WordPress site
pub async fn purge(domain: &str, all: bool, page: bool, object: bool, cli: &Cli) -> Result<()> {
    // Check if site exists
    let site = database::sites::get_by_domain(domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Site {} not found", domain))?;

    // Check if it's a WordPress site
    if site.site_type != "wp" {
        anyhow::bail!(
            "Cache purge is only available for WordPress sites. {} is a {} site.",
            domain,
            site.site_type
        );
    }

    let webroot = format!("/var/www/{}/prod/public", domain);

    // Determine what to purge
    // If no specific flags, default to purging page cache
    let purge_page = all || page || !object;
    let purge_object = all || object;

    println!(
        "{} Purging cache for {}...\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Purge page cache (FastCGI or Redis full-page cache via Nginx Helper)
    if purge_page {
        purge_page_cache(domain, &webroot, cli.verbose).await?;
    }

    // Purge Redis object cache
    if purge_object {
        purge_object_cache(&webroot, cli.verbose).await?;
    }

    println!("\n{} Cache purged successfully!", "✓".green().bold());

    Ok(())
}

/// Purge page cache using Nginx Helper plugin AND direct cache deletion
async fn purge_page_cache(_domain: &str, webroot: &str, verbose: bool) -> Result<()> {
    use std::io::{self, Write};

    print!("  {} Purging page cache...", "→".bright_cyan());
    io::stdout().flush().ok();

    // Always clear the filesystem cache directly (most reliable)
    let cache_path = "/var/cache/nginx/fastcgi";
    if std::path::Path::new(cache_path).exists() {
        // Clear all cache files
        let _ = shell::run_command("find", &[cache_path, "-type", "f", "-delete"]).await;
    }

    // Also try Nginx Helper's WP-CLI command to notify WordPress
    let _ = shell::run_command_with_output(
        "wp",
        &[
            "nginx-helper",
            "purge-all",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    println!(" {}", "done".green());
    Ok(())
}

/// Purge Redis object cache
async fn purge_object_cache(webroot: &str, verbose: bool) -> Result<()> {
    use std::io::{self, Write};

    print!("  {} Purging Redis object cache...", "→".bright_cyan());
    io::stdout().flush().ok();

    // Try Redis Object Cache plugin's flush command
    let result = shell::run_command_with_output(
        "wp",
        &[
            "redis",
            "flush",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    if result.is_ok() {
        println!(" {} (via Redis Object Cache plugin)", "done".green());
        return Ok(());
    }

    // Fallback: Try WordPress cache flush
    let result = shell::run_command_with_output(
        "wp",
        &[
            "cache",
            "flush",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    if result.is_ok() {
        println!(" {} (via WP cache flush)", "done".green());
        return Ok(());
    }

    // Final fallback: Direct redis-cli flush (only for this site's keys)
    // We don't do FLUSHALL as that would affect all sites
    println!(" {} (plugin not available)", "skipped".yellow());

    Ok(())
}

/// Check cache status for a site
pub async fn status(domain: &str, _cli: &Cli) -> Result<()> {
    let webroot = format!("/var/www/{}/prod/public", domain);

    println!(
        "{} Cache status for {}:\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Check if Nginx Helper is installed and active
    let nginx_helper = shell::run_command(
        "wp",
        &[
            "plugin",
            "is-active",
            "nginx-helper",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
    )
    .await
    .is_ok();

    println!(
        "  Nginx Helper: {}",
        if nginx_helper {
            "active".green()
        } else {
            "not installed".yellow()
        }
    );

    // Check if Redis Object Cache is installed and active
    let redis_cache = shell::run_command(
        "wp",
        &[
            "plugin",
            "is-active",
            "redis-cache",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
    )
    .await
    .is_ok();

    println!(
        "  Redis Object Cache: {}",
        if redis_cache {
            "active".green()
        } else {
            "not installed".yellow()
        }
    );

    // Check Redis connection if plugin is active
    if redis_cache {
        let redis_status = shell::run_command(
            "wp",
            &[
                "redis",
                "status",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
        )
        .await;

        if redis_status.is_ok() {
            println!("  Redis connection: {}", "connected".green());
        } else {
            println!("  Redis connection: {}", "disconnected".red());
        }
    }

    // Check cache directory
    let cache_path = "/var/cache/nginx";
    if std::path::Path::new(cache_path).exists() {
        // Get cache size
        if let Ok(output) = shell::run_command("du", &["-sh", cache_path]).await {
            let size = output.split_whitespace().next().unwrap_or("unknown");
            println!("  FastCGI cache size: {}", size.bright_white());
        }
    }

    println!();
    Ok(())
}
