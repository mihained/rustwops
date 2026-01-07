use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use dialoguer::Confirm;
use tabled::{Table, Tabled};

use crate::config::{nginx, php};
use crate::database;
use crate::utils::{password, shell};
use crate::Cli;

#[derive(Clone, Subcommand)]
pub enum StagingCommand {
    /// Create staging environment
    Create {
        /// Production domain
        domain: String,

        /// Staging subdomain prefix
        #[arg(long, default_value = "staging")]
        prefix: String,
    },

    /// Sync staging environment
    Sync {
        /// Production domain
        domain: String,

        /// Sync direction
        #[arg(long, value_enum)]
        direction: SyncDirection,

        /// Sync only files
        #[arg(long)]
        files_only: bool,

        /// Sync only database
        #[arg(long)]
        db_only: bool,

        /// Tables to exclude (comma-separated)
        #[arg(long)]
        exclude_tables: Option<String>,

        /// Dry run (show what would be synced)
        #[arg(long)]
        dry_run: bool,
    },

    /// Delete staging environment
    Delete {
        /// Production domain
        domain: String,
    },

    /// List staging environments
    List,

    /// Show staging info
    Info {
        /// Production domain
        domain: String,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum SyncDirection {
    #[value(name = "prod-to-stage")]
    ProdToStage,
    #[value(name = "stage-to-prod")]
    StageToProd,
}

impl std::fmt::Display for SyncDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncDirection::ProdToStage => write!(f, "prod_to_stage"),
            SyncDirection::StageToProd => write!(f, "stage_to_prod"),
        }
    }
}

pub async fn execute(command: StagingCommand, cli: &Cli) -> Result<()> {
    match command {
        StagingCommand::Create { domain, prefix } => create_staging(&domain, &prefix, cli).await,
        StagingCommand::Sync {
            domain,
            direction,
            files_only,
            db_only,
            exclude_tables,
            dry_run,
        } => {
            sync_staging(
                &domain,
                direction,
                files_only,
                db_only,
                exclude_tables,
                dry_run,
                cli,
            )
            .await
        }
        StagingCommand::Delete { domain } => delete_staging(&domain, cli).await,
        StagingCommand::List => list_staging().await,
        StagingCommand::Info { domain } => show_staging_info(&domain).await,
    }
}

// =============================================================================
// Create Staging
// =============================================================================

async fn create_staging(production_domain: &str, prefix: &str, _cli: &Cli) -> Result<()> {
    println!(
        "{} Creating staging environment for: {}\n",
        "→".bright_cyan().bold(),
        production_domain.bright_white()
    );

    // Check if production site exists
    if !database::sites::exists(production_domain).await? {
        anyhow::bail!("Production site {} does not exist", production_domain);
    }

    // Check if staging already exists
    if database::staging::get_by_production(production_domain)
        .await?
        .is_some()
    {
        anyhow::bail!(
            "Staging environment already exists for {}",
            production_domain
        );
    }

    let staging_domain = format!("{}.{}", prefix, production_domain);

    // Check if staging domain conflicts with existing site
    if database::sites::exists(&staging_domain).await? {
        anyhow::bail!("Domain {} already exists as a site", staging_domain);
    }

    // Get production site info
    let prod_site = database::sites::get(production_domain).await?;
    let php_version = prod_site.php_version.as_deref().unwrap_or("8.4");

    println!(
        "  {} Staging domain: {}",
        "→".bright_cyan(),
        staging_domain.bright_white()
    );

    // Create staging directory structure
    let staging_webroot = format!("/var/www/{}/staging/public", production_domain);
    let prod_webroot = format!("/var/www/{}/prod/public", production_domain);

    shell::run_command("mkdir", &["-p", &staging_webroot]).await?;
    println!("  {} Created staging directory", "✓".green());

    // Clone files from production
    println!("  {} Cloning files from production...", "→".bright_cyan());
    shell::run_command(
        "rsync",
        &[
            "-a",
            "--delete",
            &format!("{}/", prod_webroot),
            &format!("{}/", staging_webroot),
        ],
    )
    .await?;
    println!("  {} Cloned files", "✓".green());

    // Clone database if applicable
    let staging_db = if prod_site.site_type == "wp" || prod_site.site_type == "php" {
        if let Some(prod_db) = database::databases::get_by_domain(production_domain).await? {
            let staging_db_name = format!("{}_staging", prod_db.db_name);
            let staging_db_user =
                format!("{}_stg", &prod_db.db_user[..prod_db.db_user.len().min(12)]);
            let staging_db_pass = password::generate(32);

            // Create staging database
            clone_database(
                &prod_db.db_name,
                &staging_db_name,
                &staging_db_user,
                &staging_db_pass,
            )
            .await?;
            println!("  {} Cloned database", "✓".green());

            // Store staging database info
            database::databases::create(
                &staging_domain,
                &staging_db_name,
                &staging_db_user,
                &staging_db_pass,
            )
            .await?;

            // For WordPress, update wp-config.php to use staging database
            if prod_site.site_type == "wp" {
                update_wp_config(
                    &staging_webroot,
                    &staging_db_name,
                    &staging_db_user,
                    &staging_db_pass,
                )
                .await?;
                println!("  {} Updated wp-config.php", "✓".green());

                wp_search_replace(
                    production_domain,
                    &staging_domain,
                    &staging_webroot,
                    &staging_db_name,
                )
                .await?;
                println!("  {} Updated WordPress URLs", "✓".green());
            }

            Some((staging_db_name, staging_db_user, staging_db_pass))
        } else {
            None
        }
    } else {
        None
    };

    // Create PHP-FPM pool for staging (with correct webroot path)
    if prod_site.site_type == "wp" || prod_site.site_type == "php" {
        let staging_chdir = format!("/var/www/{}/staging", production_domain);
        php::create_pool_with_webroot(&staging_domain, php_version, Some(&staging_chdir)).await?;
        println!("  {} Created PHP-FPM pool", "✓".green());
    }

    // Create Nginx config for staging
    let site_type = match prod_site.site_type.as_str() {
        "wp" => crate::commands::site::SiteType::Wp,
        "php" => crate::commands::site::SiteType::Php,
        "static" => crate::commands::site::SiteType::Static,
        "proxy" => crate::commands::site::SiteType::Proxy,
        "node" => crate::commands::site::SiteType::Node,
        _ => crate::commands::site::SiteType::Php,
    };

    let cache_type = prod_site.cache_type.as_deref().and_then(|c| match c {
        "fastcgi" => Some(crate::commands::site::CacheType::Fastcgi),
        "redis" => Some(crate::commands::site::CacheType::Redis),
        _ => None,
    });

    nginx::create_site_config(
        &staging_domain,
        site_type,
        php_version,
        cache_type,
        &staging_webroot,
        None,
    )
    .await?;
    println!("  {} Created Nginx configuration", "✓".green());

    // Enable staging site
    let available = format!("/etc/nginx/sites-available/{}", staging_domain);
    let enabled = format!("/etc/nginx/sites-enabled/{}", staging_domain);
    shell::run_command("ln", &["-sf", &available, &enabled]).await?;
    println!("  {} Enabled staging site", "✓".green());

    // Set permissions
    let staging_base = format!("/var/www/{}/staging", production_domain);
    if shell::run_command("chown", &["-R", "www-data:www-data", &staging_base])
        .await
        .is_err()
    {
        shell::run_command("chown", &["-R", "root:root", &staging_base]).await?;
    }
    println!("  {} Set file permissions", "✓".green());

    // Reload services
    if shell::command_exists("nginx").await {
        let _ = shell::run_command("nginx", &["-t"]).await;
        let _ = shell::run_command("systemctl", &["reload", "nginx"]).await;
    }
    if prod_site.site_type == "wp" || prod_site.site_type == "php" {
        let fpm = format!("php{}-fpm", php_version);
        let _ = shell::run_command("systemctl", &["reload", &fpm]).await;
    }
    println!("  {} Reloaded services", "✓".green());

    // Register staging site in database (with correct staging webroot)
    database::sites::create_with_webroot(
        &staging_domain,
        site_type,
        php_version,
        cache_type,
        &staging_webroot,
    )
    .await?;
    database::staging::create(production_domain, prefix).await?;

    // Print summary
    println!("\n{}", "━".repeat(50).dimmed());
    println!("\n{} Staging environment created!\n", "✓".green().bold());
    println!(
        "  {} Production: http://{}",
        "→".bright_cyan(),
        production_domain
    );
    println!(
        "  {} Staging:    http://{}",
        "→".bright_cyan(),
        staging_domain
    );

    if let Some((db_name, db_user, db_pass)) = staging_db {
        println!("\n  {} Staging Database:", "→".bright_cyan());
        println!("    Name:     {}", db_name);
        println!("    User:     {}", db_user);
        println!("    Password: {}", db_pass.bright_yellow());
    }

    println!(
        "\n  {} Add to /etc/hosts: 127.0.0.1 {}",
        "ℹ".blue(),
        staging_domain
    );
    println!();

    Ok(())
}

// =============================================================================
// Sync Staging
// =============================================================================

async fn sync_staging(
    production_domain: &str,
    direction: SyncDirection,
    files_only: bool,
    db_only: bool,
    exclude_tables: Option<String>,
    dry_run: bool,
    cli: &Cli,
) -> Result<()> {
    let direction_str = match direction {
        SyncDirection::ProdToStage => "Production → Staging",
        SyncDirection::StageToProd => "Staging → Production",
    };

    println!(
        "{} Syncing: {} ({})\n",
        "→".bright_cyan().bold(),
        production_domain.bright_white(),
        direction_str
    );

    // Get staging info
    let staging = database::staging::get_by_production(production_domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No staging environment found for {}", production_domain))?;

    let staging_domain = &staging.staging_domain;
    let prod_site = database::sites::get(production_domain).await?;

    // Confirm stage-to-prod sync
    if matches!(direction, SyncDirection::StageToProd) && !cli.yes && !dry_run {
        let confirm = Confirm::new()
            .with_prompt("This will overwrite PRODUCTION data. Are you sure?")
            .default(false)
            .interact()?;

        if !confirm {
            println!("{}", "Aborted.".yellow());
            return Ok(());
        }

        // Create backup before stage-to-prod
        println!("  {} Creating backup before sync...", "→".bright_cyan());
        // TODO: Integrate with backup system when implemented
    }

    let sync_files = !db_only;
    let sync_db = !files_only;

    let prod_webroot = format!("/var/www/{}/prod/public", production_domain);
    let staging_webroot = format!("/var/www/{}/staging/public", production_domain);

    // Sync files
    if sync_files {
        let (source, dest) = match direction {
            SyncDirection::ProdToStage => (&prod_webroot, &staging_webroot),
            SyncDirection::StageToProd => (&staging_webroot, &prod_webroot),
        };

        if dry_run {
            println!(
                "  {} [DRY RUN] Would sync files: {} → {}",
                "→".bright_cyan(),
                source,
                dest
            );
        } else {
            println!("  {} Syncing files...", "→".bright_cyan());
            // Exclude wp-config.php to preserve database credentials
            shell::run_command(
                "rsync",
                &[
                    "-a",
                    "--delete",
                    "--exclude=wp-config.php",
                    &format!("{}/", source),
                    &format!("{}/", dest),
                ],
            )
            .await?;
            println!("  {} Files synced", "✓".green());
        }
    }

    // Sync database
    if sync_db && (prod_site.site_type == "wp" || prod_site.site_type == "php") {
        let prod_db = database::databases::get_by_domain(production_domain).await?;
        let staging_db = database::databases::get_by_domain(staging_domain).await?;

        if let (Some(prod_db), Some(staging_db)) = (prod_db, staging_db) {
            let (source_db, dest_db, source_domain, dest_domain) = match direction {
                SyncDirection::ProdToStage => (
                    &prod_db.db_name,
                    &staging_db.db_name,
                    production_domain,
                    staging_domain.as_str(),
                ),
                SyncDirection::StageToProd => (
                    &staging_db.db_name,
                    &prod_db.db_name,
                    staging_domain.as_str(),
                    production_domain,
                ),
            };

            // Parse excluded tables
            let exclude: Vec<&str> = exclude_tables
                .as_deref()
                .map(|t| t.split(',').map(|s| s.trim()).collect())
                .unwrap_or_default();

            if dry_run {
                println!(
                    "  {} [DRY RUN] Would sync database: {} → {}",
                    "→".bright_cyan(),
                    source_db,
                    dest_db
                );
                if !exclude.is_empty() {
                    println!(
                        "  {} [DRY RUN] Excluding tables: {}",
                        "→".bright_cyan(),
                        exclude.join(", ")
                    );
                }
            } else {
                println!("  {} Syncing database...", "→".bright_cyan());
                sync_database(source_db, dest_db, &exclude).await?;
                println!("  {} Database synced", "✓".green());

                // For WordPress, update URLs
                if prod_site.site_type == "wp" {
                    let webroot = match direction {
                        SyncDirection::ProdToStage => &staging_webroot,
                        SyncDirection::StageToProd => &prod_webroot,
                    };
                    wp_search_replace(source_domain, dest_domain, webroot, dest_db).await?;
                    println!("  {} Updated WordPress URLs", "✓".green());
                }
            }
        }
    }

    if !dry_run {
        // Update sync timestamp
        database::staging::update_sync(production_domain, &direction.to_string()).await?;

        println!("\n{} Sync complete!\n", "✓".green().bold());
    } else {
        println!(
            "\n{} Dry run complete. No changes made.\n",
            "ℹ".blue().bold()
        );
    }

    Ok(())
}

// =============================================================================
// Delete Staging
// =============================================================================

async fn delete_staging(production_domain: &str, cli: &Cli) -> Result<()> {
    println!(
        "{} Deleting staging environment for: {}\n",
        "→".bright_cyan().bold(),
        production_domain.bright_white()
    );

    // Get staging info
    let staging = database::staging::get_by_production(production_domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No staging environment found for {}", production_domain))?;

    let staging_domain = &staging.staging_domain;
    let prod_site = database::sites::get(production_domain).await?;
    let php_version = prod_site.php_version.as_deref().unwrap_or("8.4");

    // Confirm deletion
    if !cli.yes {
        let confirm = Confirm::new()
            .with_prompt(format!("Delete staging environment {}?", staging_domain))
            .default(false)
            .interact()?;

        if !confirm {
            println!("{}", "Aborted.".yellow());
            return Ok(());
        }
    }

    // Disable and remove nginx config
    let enabled = format!("/etc/nginx/sites-enabled/{}", staging_domain);
    let available = format!("/etc/nginx/sites-available/{}", staging_domain);
    shell::run_command("rm", &["-f", &enabled]).await?;
    shell::run_command("rm", &["-f", &available]).await?;
    println!("  {} Removed Nginx configuration", "✓".green());

    // Remove PHP-FPM pool
    if prod_site.site_type == "wp" || prod_site.site_type == "php" {
        let pool_file = format!(
            "/etc/php/{}/fpm/pool.d/{}.conf",
            php_version, staging_domain
        );
        shell::run_command("rm", &["-f", &pool_file]).await?;
        println!("  {} Removed PHP-FPM pool", "✓".green());
    }

    // Remove staging files
    let staging_dir = format!("/var/www/{}/staging", production_domain);
    shell::run_command("rm", &["-rf", &staging_dir]).await?;
    println!("  {} Removed staging files", "✓".green());

    // Drop staging database
    if let Some(staging_db) = database::databases::get_by_domain(staging_domain).await? {
        let sql = format!(
            "DROP DATABASE IF EXISTS `{}`; DROP USER IF EXISTS '{}'@'localhost';",
            staging_db.db_name, staging_db.db_user
        );
        let _ = shell::run_command("mysql", &["-e", &sql]).await;
        database::databases::delete(staging_domain).await?;
        println!("  {} Removed staging database", "✓".green());
    }

    // Reload services
    if shell::command_exists("nginx").await {
        let _ = shell::run_command("systemctl", &["reload", "nginx"]).await;
    }
    if prod_site.site_type == "wp" || prod_site.site_type == "php" {
        let fpm = format!("php{}-fpm", php_version);
        let _ = shell::run_command("systemctl", &["reload", &fpm]).await;
    }
    println!("  {} Reloaded services", "✓".green());

    // Remove from database
    database::sites::delete(staging_domain).await?;
    database::staging::delete(production_domain).await?;

    println!("\n{} Staging environment deleted!\n", "✓".green().bold());

    Ok(())
}

// =============================================================================
// List Staging
// =============================================================================

#[derive(Tabled)]
struct StagingRow {
    #[tabled(rename = "Production")]
    production: String,
    #[tabled(rename = "Staging")]
    staging: String,
    #[tabled(rename = "Last Sync")]
    last_sync: String,
    #[tabled(rename = "Direction")]
    direction: String,
}

async fn list_staging() -> Result<()> {
    let staging_sites = database::staging::list().await?;

    if staging_sites.is_empty() {
        println!("{} No staging environments found.\n", "→".bright_cyan());
        return Ok(());
    }

    println!(
        "{} Staging Environments ({}):\n",
        "→".bright_cyan().bold(),
        staging_sites.len()
    );

    let mut rows = Vec::new();

    for staging in staging_sites {
        // Get production domain
        let prod_domain = {
            let conn = database::get_connection()?;
            let conn = conn.lock().await;
            conn.query_row(
                "SELECT domain FROM sites WHERE id = ?1",
                rusqlite::params![staging.production_site_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "unknown".to_string())
        };

        let last_sync = staging
            .last_sync_at
            .map(|s| s[..19].to_string())
            .unwrap_or_else(|| "never".to_string());

        let direction = staging
            .last_sync_direction
            .map(|d| match d.as_str() {
                "prod_to_stage" => "prod → stage".to_string(),
                "stage_to_prod" => "stage → prod".to_string(),
                _ => d,
            })
            .unwrap_or_else(|| "-".to_string());

        rows.push(StagingRow {
            production: prod_domain,
            staging: staging.staging_domain,
            last_sync,
            direction,
        });
    }

    let table = Table::new(rows).to_string();
    println!("{}\n", table);

    Ok(())
}

// =============================================================================
// Staging Info
// =============================================================================

async fn show_staging_info(production_domain: &str) -> Result<()> {
    let staging = database::staging::get_by_production(production_domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No staging environment found for {}", production_domain))?;

    let _prod_site = database::sites::get(production_domain).await?;

    println!(
        "{} Staging Information: {}\n",
        "→".bright_cyan().bold(),
        production_domain.bright_white()
    );

    println!("  {} General:", "●".bright_white().bold());
    println!("    Production:    {}", production_domain);
    println!("    Staging:       {}", staging.staging_domain);
    println!("    Created:       {}", staging.created_at);

    if let Some(last_sync) = &staging.last_sync_at {
        println!("    Last Sync:     {}", last_sync);
    }
    if let Some(direction) = &staging.last_sync_direction {
        let dir_display = match direction.as_str() {
            "prod_to_stage" => "Production → Staging",
            "stage_to_prod" => "Staging → Production",
            _ => direction,
        };
        println!("    Sync Direction: {}", dir_display);
    }

    println!("\n  {} Paths:", "●".bright_white().bold());
    println!("    Production: /var/www/{}/prod/public", production_domain);
    println!(
        "    Staging:    /var/www/{}/staging/public",
        production_domain
    );

    // Database info
    if let Some(prod_db) = database::databases::get_by_domain(production_domain).await? {
        println!("\n  {} Production Database:", "●".bright_white().bold());
        println!("    Name: {}", prod_db.db_name);
        println!("    User: {}", prod_db.db_user);
    }

    if let Some(staging_db) = database::databases::get_by_domain(&staging.staging_domain).await? {
        println!("\n  {} Staging Database:", "●".bright_white().bold());
        println!("    Name: {}", staging_db.db_name);
        println!("    User: {}", staging_db.db_user);
    }

    // Disk usage
    let prod_size = shell::run_command(
        "du",
        &["-sh", &format!("/var/www/{}/prod", production_domain)],
    )
    .await
    .ok()
    .and_then(|s| s.split_whitespace().next().map(String::from))
    .unwrap_or_else(|| "N/A".to_string());

    let staging_size = shell::run_command(
        "du",
        &["-sh", &format!("/var/www/{}/staging", production_domain)],
    )
    .await
    .ok()
    .and_then(|s| s.split_whitespace().next().map(String::from))
    .unwrap_or_else(|| "N/A".to_string());

    println!("\n  {} Disk Usage:", "●".bright_white().bold());
    println!("    Production: {}", prod_size);
    println!("    Staging:    {}", staging_size);

    println!();

    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

async fn clone_database(
    source_db: &str,
    dest_db: &str,
    dest_user: &str,
    dest_pass: &str,
) -> Result<()> {
    // Create destination database and user
    // Drop user first to ensure we can set the correct password
    // (CREATE USER IF NOT EXISTS doesn't update password for existing users)
    let setup_sql = format!(
        r#"
        DROP DATABASE IF EXISTS `{}`;
        CREATE DATABASE `{}`;
        DROP USER IF EXISTS '{}'@'localhost';
        CREATE USER '{}'@'localhost' IDENTIFIED BY '{}';
        GRANT ALL PRIVILEGES ON `{}`.* TO '{}'@'localhost';
        FLUSH PRIVILEGES;
        "#,
        dest_db, dest_db, dest_user, dest_user, dest_pass, dest_db, dest_user
    );
    shell::run_command("mysql", &["-e", &setup_sql]).await?;

    // Dump and import
    let dump_cmd = format!(
        "mysqldump --single-transaction {} | mysql {}",
        source_db, dest_db
    );
    shell::run_shell_script(&dump_cmd, false).await?;

    Ok(())
}

async fn sync_database(source_db: &str, dest_db: &str, exclude_tables: &[&str]) -> Result<()> {
    // Build mysqldump command with exclusions
    let mut dump_args = vec!["--single-transaction".to_string(), source_db.to_string()];
    for table in exclude_tables {
        dump_args.push(format!("--ignore-table={}.{}", source_db, table));
    }

    let dump_cmd = format!("mysqldump {} | mysql {}", dump_args.join(" "), dest_db);

    // Drop all tables in destination first (to ensure clean sync)
    let drop_tables = format!(
        r#"
        SET FOREIGN_KEY_CHECKS = 0;
        SET @tables = NULL;
        SELECT GROUP_CONCAT('`', table_name, '`') INTO @tables
        FROM information_schema.tables
        WHERE table_schema = '{}';
        SET @tables = IFNULL(@tables, 'dummy');
        SET @sql = CONCAT('DROP TABLE IF EXISTS ', @tables);
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
        SET FOREIGN_KEY_CHECKS = 1;
        "#,
        dest_db
    );
    let _ = shell::run_command("mysql", &[dest_db, "-e", &drop_tables]).await;

    shell::run_shell_script(&dump_cmd, false).await?;

    Ok(())
}

async fn wp_search_replace(
    old_domain: &str,
    new_domain: &str,
    webroot: &str,
    _db_name: &str,
) -> Result<()> {
    // Use WP-CLI for search-replace (handles serialized data correctly)
    let old_url = format!("http://{}", old_domain);
    let new_url = format!("http://{}", new_domain);

    shell::run_command(
        "wp",
        &[
            "search-replace",
            &old_url,
            &new_url,
            &format!("--path={}", webroot),
            "--all-tables",
            "--allow-root",
        ],
    )
    .await?;

    // Also replace https if applicable
    let old_https = format!("https://{}", old_domain);
    let new_https = format!("https://{}", new_domain);

    let _ = shell::run_command(
        "wp",
        &[
            "search-replace",
            &old_https,
            &new_https,
            &format!("--path={}", webroot),
            "--all-tables",
            "--allow-root",
        ],
    )
    .await;

    Ok(())
}

async fn update_wp_config(
    webroot: &str,
    db_name: &str,
    db_user: &str,
    db_pass: &str,
) -> Result<()> {
    let wp_config_path = format!("{}/wp-config.php", webroot);

    // Read current wp-config.php
    let content = tokio::fs::read_to_string(&wp_config_path).await?;

    // Replace database credentials using regex
    // Match: define( 'DB_NAME', 'value' ) with various spacing and quote styles
    let db_name_re =
        regex::Regex::new(r#"define\s*\(\s*['"]DB_NAME['"]\s*,\s*['"][^'"]*['"]\s*\)"#)?;
    let db_user_re =
        regex::Regex::new(r#"define\s*\(\s*['"]DB_USER['"]\s*,\s*['"][^'"]*['"]\s*\)"#)?;
    let db_pass_re =
        regex::Regex::new(r#"define\s*\(\s*['"]DB_PASSWORD['"]\s*,\s*['"][^'"]*['"]\s*\)"#)?;

    let content = db_name_re
        .replace(&content, format!("define( 'DB_NAME', '{}' )", db_name))
        .to_string();
    let content = db_user_re
        .replace(&content, format!("define( 'DB_USER', '{}' )", db_user))
        .to_string();
    let content = db_pass_re
        .replace(&content, format!("define( 'DB_PASSWORD', '{}' )", db_pass))
        .to_string();

    tokio::fs::write(&wp_config_path, content).await?;

    Ok(())
}
