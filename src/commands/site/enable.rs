use anyhow::Result;
use colored::Colorize;

use crate::database;
use crate::utils::shell;
use crate::Cli;

const SITES_AVAILABLE: &str = "/etc/nginx/sites-available";
const SITES_ENABLED: &str = "/etc/nginx/sites-enabled";

/// Enable a site by creating nginx symlink
pub async fn enable(domain: &str, cli: &Cli) -> Result<()> {
    // Check if site exists
    let site = database::sites::get_by_domain(domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Site {} not found", domain))?;

    if site.enabled {
        println!(
            "{} Site {} is already enabled",
            "→".bright_cyan(),
            domain.bright_white()
        );
        return Ok(());
    }

    println!(
        "{} Enabling site {}...\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Create symlink
    let available = format!("{}/{}", SITES_AVAILABLE, domain);
    let enabled = format!("{}/{}", SITES_ENABLED, domain);

    // Check if config exists
    if !std::path::Path::new(&available).exists() {
        anyhow::bail!(
            "Nginx configuration not found at {}. Site may be corrupted.",
            available
        );
    }

    // Remove existing symlink if any
    if std::path::Path::new(&enabled).exists() {
        tokio::fs::remove_file(&enabled).await?;
    }

    // Create symlink
    tokio::fs::symlink(&available, &enabled).await?;
    println!("  {} Created nginx symlink", "✓".green());

    // Enable PHP-FPM pool if applicable
    if let Some(ref php_version) = site.php_version {
        if !php_version.is_empty() {
            let pool_path = format!("/etc/php/{}/fpm/pool.d/{}.conf", php_version, domain);
            if std::path::Path::new(&pool_path).exists() {
                // Reload PHP-FPM
                let service = format!("php{}-fpm", php_version);
                shell::run_command("systemctl", &["reload", &service]).await?;
                println!("  {} Reloaded PHP-FPM {}", "✓".green(), php_version);
            }
        }
    }

    // Test nginx configuration
    shell::run_command("nginx", &["-t"]).await?;
    println!("  {} Nginx configuration valid", "✓".green());

    // Reload nginx
    shell::run_command("systemctl", &["reload", "nginx"]).await?;
    println!("  {} Reloaded nginx", "✓".green());

    // Update database
    database::sites::update_enabled(domain, true).await?;
    println!("  {} Updated database", "✓".green());

    println!(
        "\n{} Site {} enabled successfully!",
        "✓".green().bold(),
        domain.bright_white()
    );

    if cli.verbose {
        println!("\n  URL: http://{}", domain);
    }

    Ok(())
}

/// Disable a site by removing nginx symlink
pub async fn disable(domain: &str, cli: &Cli) -> Result<()> {
    // Check if site exists
    let site = database::sites::get_by_domain(domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Site {} not found", domain))?;

    if !site.enabled {
        println!(
            "{} Site {} is already disabled",
            "→".bright_cyan(),
            domain.bright_white()
        );
        return Ok(());
    }

    println!(
        "{} Disabling site {}...\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Remove symlink
    let enabled = format!("{}/{}", SITES_ENABLED, domain);

    if std::path::Path::new(&enabled).exists() {
        tokio::fs::remove_file(&enabled).await?;
        println!("  {} Removed nginx symlink", "✓".green());
    }

    // Test nginx configuration
    shell::run_command("nginx", &["-t"]).await?;
    println!("  {} Nginx configuration valid", "✓".green());

    // Reload nginx
    shell::run_command("systemctl", &["reload", "nginx"]).await?;
    println!("  {} Reloaded nginx", "✓".green());

    // Update database
    database::sites::update_enabled(domain, false).await?;
    println!("  {} Updated database", "✓".green());

    println!(
        "\n{} Site {} disabled successfully!",
        "✓".green().bold(),
        domain.bright_white()
    );

    if cli.verbose {
        println!("\n  Note: Site files and database are preserved. Use 'rw site enable {}' to re-enable.", domain);
    }

    Ok(())
}
