use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::Subcommand;
use colored::Colorize;
use std::path::Path;
use tabled::{Table, Tabled};

use crate::database::{self, backups::BackupMetadata};
use crate::utils::shell;
use crate::Cli;

const BACKUP_DIR: &str = "/var/lib/rustwops/backups";

#[derive(Clone, Subcommand)]
pub enum BackupCommand {
    /// Create a backup
    Create {
        /// Domain name (all sites if not specified)
        domain: Option<String>,

        /// Backup name/label
        #[arg(long)]
        name: Option<String>,

        /// Backup only database
        #[arg(long)]
        db_only: bool,

        /// Backup only files
        #[arg(long)]
        files_only: bool,
    },

    /// Restore from backup
    Restore {
        /// Backup ID or file path
        backup: String,

        /// Restore to different domain
        #[arg(long)]
        target: Option<String>,

        /// Restore only database
        #[arg(long)]
        db_only: bool,

        /// Restore only files
        #[arg(long)]
        files_only: bool,
    },

    /// List backups
    List {
        /// Filter by domain
        #[arg(long)]
        domain: Option<String>,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },

    /// Delete backup
    Delete {
        /// Backup ID
        backup_id: Option<String>,

        /// Delete backups older than N days
        #[arg(long)]
        older_than: Option<u32>,
    },

    /// Configure backup settings
    Config {
        /// Backup directory
        #[arg(long)]
        dir: Option<String>,

        /// Retention days
        #[arg(long)]
        retention: Option<u32>,

        /// S3 bucket name
        #[arg(long)]
        s3_bucket: Option<String>,

        /// S3 region
        #[arg(long)]
        s3_region: Option<String>,

        /// Backup schedule (cron format)
        #[arg(long)]
        schedule: Option<String>,
    },

    /// Show backup configuration
    ConfigShow,
}

pub async fn execute(command: BackupCommand, cli: &Cli) -> Result<()> {
    match command {
        BackupCommand::Create {
            domain,
            name,
            db_only,
            files_only,
        } => create_backup(domain, name, db_only, files_only, cli).await,
        BackupCommand::Restore {
            backup,
            target,
            db_only,
            files_only,
        } => restore_backup(backup, target, db_only, files_only, cli).await,
        BackupCommand::List { domain, detailed } => list_backups(domain, detailed).await,
        BackupCommand::Delete {
            backup_id,
            older_than,
        } => delete_backup(backup_id, older_than, cli).await,
        BackupCommand::Config { .. } => {
            anyhow::bail!("Backup config not yet implemented. Coming soon!")
        }
        BackupCommand::ConfigShow => {
            anyhow::bail!("Backup config-show not yet implemented. Coming soon!")
        }
    }
}

async fn create_backup(
    domain: Option<String>,
    name: Option<String>,
    db_only: bool,
    files_only: bool,
    _cli: &Cli,
) -> Result<()> {
    let domain = domain.ok_or_else(|| anyhow!("Domain name is required"))?;

    println!(
        "{} Creating backup for: {}\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Get site info
    let site = database::sites::get_by_domain(&domain)
        .await?
        .ok_or_else(|| anyhow!("Site not found: {}", domain))?;

    // Ensure backup directory exists
    tokio::fs::create_dir_all(BACKUP_DIR).await?;
    let site_backup_dir = format!("{}/{}", BACKUP_DIR, domain);
    tokio::fs::create_dir_all(&site_backup_dir).await?;

    // Generate backup filename
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let backup_name = name.unwrap_or_else(|| format!("backup_{}", timestamp));
    let backup_path = format!("{}/{}_{}.tar.gz", site_backup_dir, backup_name, timestamp);

    // Create temp directory for backup contents
    let temp_dir = format!("/tmp/rustwops_backup_{}_{}", domain, timestamp);
    tokio::fs::create_dir_all(&temp_dir).await?;

    let mut includes_db = false;
    let mut includes_files = false;
    let mut db_name = None;

    // Backup database if site has one and not files_only
    if !files_only {
        if let Ok(Some(db)) = database::databases::get_by_domain(&domain).await {
            println!("  {} Backing up database: {}", "→".cyan(), db.db_name);

            let dump_path = format!("{}/database.sql", temp_dir);

            // Get MySQL root password from credentials file
            let mysql_pass = get_mysql_root_password().await?;

            // Use mysqldump
            let dump_cmd = format!(
                "mysqldump -u root -p'{}' --single-transaction --quick '{}' > '{}'",
                mysql_pass, db.db_name, dump_path
            );
            shell::run_command("bash", &["-c", &dump_cmd]).await?;

            // Compress the SQL dump
            shell::run_command("gzip", &[&dump_path]).await?;

            includes_db = true;
            db_name = Some(db.db_name.clone());
            println!("  {} Database backup complete", "✓".green());
        }
    }

    // Backup files if not db_only
    if !db_only {
        let webroot = &site.webroot;
        if Path::new(webroot).exists() {
            println!("  {} Backing up files: {}", "→".cyan(), webroot);

            let files_archive = format!("{}/files.tar.gz", temp_dir);
            shell::run_command(
                "tar",
                &[
                    "-czf",
                    &files_archive,
                    "-C",
                    webroot,
                    "--exclude=*.log",
                    "--exclude=node_modules",
                    "--exclude=.git",
                    ".",
                ],
            )
            .await?;

            includes_files = true;
            println!("  {} Files backup complete", "✓".green());
        }
    }

    // Create metadata
    let metadata = BackupMetadata {
        domain: domain.clone(),
        site_type: site.site_type.clone(),
        php_version: site.php_version.clone(),
        db_name,
        backup_date: Utc::now().to_rfc3339(),
        files_included: includes_files,
        db_included: includes_db,
        wp_version: None, // TODO: detect WP version
        wp_plugins: None, // TODO: list WP plugins
    };

    let metadata_path = format!("{}/metadata.json", temp_dir);
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    tokio::fs::write(&metadata_path, &metadata_json).await?;

    // Create final archive
    println!("  {} Creating archive: {}", "→".cyan(), backup_path);
    shell::run_command("tar", &["-czf", &backup_path, "-C", &temp_dir, "."]).await?;

    // Get file size
    let file_size = tokio::fs::metadata(&backup_path).await?.len() as i64;

    // Record in database
    let backup_id = database::backups::create(
        Some(site.id),
        &domain,
        Some(&backup_name),
        &backup_path,
        Some(file_size),
        includes_db,
        includes_files,
        Some(&metadata_json),
    )
    .await?;

    // Cleanup temp directory
    tokio::fs::remove_dir_all(&temp_dir).await?;

    println!(
        "\n{}\n",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!("{} Backup created successfully!\n", "✓".green().bold());
    println!("  {} ID: {}", "→".cyan(), backup_id);
    println!("  {} Path: {}", "→".cyan(), backup_path);
    println!("  {} Size: {}", "→".cyan(), format_size(file_size as u64));

    Ok(())
}

async fn restore_backup(
    backup: String,
    target: Option<String>,
    db_only: bool,
    files_only: bool,
    cli: &Cli,
) -> Result<()> {
    // Determine if backup is an ID or path
    let backup_record = if let Ok(id) = backup.parse::<i64>() {
        database::backups::get_by_id(id)
            .await?
            .ok_or_else(|| anyhow!("Backup not found: {}", id))?
    } else if Path::new(&backup).exists() {
        // If it's a path, we need to extract metadata
        return Err(anyhow!(
            "Restore from file path not yet supported. Use backup ID instead."
        ));
    } else {
        return Err(anyhow!("Invalid backup reference: {}", backup));
    };

    let target_domain = target.as_ref().unwrap_or(&backup_record.domain);

    println!(
        "{} Restoring backup to: {}\n",
        "→".bright_cyan().bold(),
        target_domain.bright_white()
    );

    // Verify target site exists
    let site = database::sites::get_by_domain(target_domain)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "Target site not found: {}. Create the site first.",
                target_domain
            )
        })?;

    // Confirm restore
    if !cli.yes {
        println!(
            "  {} This will overwrite existing data!",
            "⚠".yellow().bold()
        );
        print!("  Continue? [y/N] ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("\n{} Restore cancelled", "!".yellow());
            return Ok(());
        }
    }

    // Extract backup to temp directory
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let temp_dir = format!("/tmp/rustwops_restore_{}_{}", target_domain, timestamp);
    tokio::fs::create_dir_all(&temp_dir).await?;

    println!("  {} Extracting backup...", "→".cyan());
    shell::run_command("tar", &["-xzf", &backup_record.file_path, "-C", &temp_dir]).await?;

    // Restore database if included and not files_only
    if backup_record.includes_db && !files_only {
        let db_dump = format!("{}/database.sql.gz", temp_dir);
        if Path::new(&db_dump).exists() {
            println!("  {} Restoring database...", "→".cyan());

            // Get target database info
            if let Ok(Some(db)) = database::databases::get_by_domain(target_domain).await {
                let mysql_pass = get_mysql_root_password().await?;

                // Decompress and restore
                let restore_cmd = format!(
                    "gunzip -c '{}' | mysql -u root -p'{}' '{}'",
                    db_dump, mysql_pass, db.db_name
                );
                shell::run_command("bash", &["-c", &restore_cmd]).await?;

                println!("  {} Database restored", "✓".green());
            } else {
                println!(
                    "  {} No database found for target site, skipping DB restore",
                    "!".yellow()
                );
            }
        }
    }

    // Restore files if included and not db_only
    if backup_record.includes_files && !db_only {
        let files_archive = format!("{}/files.tar.gz", temp_dir);
        if Path::new(&files_archive).exists() {
            println!("  {} Restoring files...", "→".cyan());

            // Extract to webroot
            let webroot = &site.webroot;
            shell::run_command("tar", &["-xzf", &files_archive, "-C", webroot]).await?;

            // Fix permissions
            shell::run_command("chown", &["-R", "www-data:www-data", webroot]).await?;

            println!("  {} Files restored", "✓".green());
        }
    }

    // Cleanup
    tokio::fs::remove_dir_all(&temp_dir).await?;

    println!(
        "\n{}\n",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!("{} Restore completed successfully!", "✓".green().bold());

    Ok(())
}

#[derive(Tabled)]
struct BackupRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Domain")]
    domain: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "DB")]
    db: String,
    #[tabled(rename = "Files")]
    files: String,
    #[tabled(rename = "Created")]
    created: String,
}

async fn list_backups(domain: Option<String>, detailed: bool) -> Result<()> {
    let backups = database::backups::list(domain.as_deref()).await?;

    if backups.is_empty() {
        println!("{} No backups found", "!".yellow());
        return Ok(());
    }

    let title = if let Some(ref d) = domain {
        format!("→ Backups for: {}", d)
    } else {
        format!("→ All Backups ({})", backups.len())
    };

    println!("{}\n", title.bright_cyan().bold());

    if detailed {
        for backup in &backups {
            println!(
                "{} {} (ID: {})",
                "━━━".bright_cyan(),
                backup.domain.bright_white().bold(),
                backup.id
            );
            println!(
                "  {} Name: {}",
                "→".cyan(),
                backup.backup_name.as_deref().unwrap_or("-")
            );
            println!("  {} Path: {}", "→".cyan(), backup.file_path);
            println!(
                "  {} Size: {}",
                "→".cyan(),
                backup
                    .file_size
                    .map(|s| format_size(s as u64))
                    .unwrap_or_else(|| "-".to_string())
            );
            println!(
                "  {} Includes DB: {}",
                "→".cyan(),
                if backup.includes_db { "Yes" } else { "No" }
            );
            println!(
                "  {} Includes Files: {}",
                "→".cyan(),
                if backup.includes_files { "Yes" } else { "No" }
            );
            println!("  {} Created: {}", "→".cyan(), &backup.created_at);
            println!();
        }
    } else {
        let rows: Vec<BackupRow> = backups
            .iter()
            .map(|b| BackupRow {
                id: b.id,
                domain: b.domain.clone(),
                name: b.backup_name.clone().unwrap_or_else(|| "-".to_string()),
                size: b
                    .file_size
                    .map(|s| format_size(s as u64))
                    .unwrap_or_else(|| "-".to_string()),
                db: if b.includes_db {
                    "✓".to_string()
                } else {
                    "✗".to_string()
                },
                files: if b.includes_files {
                    "✓".to_string()
                } else {
                    "✗".to_string()
                },
                created: b.created_at.chars().take(16).collect(),
            })
            .collect();

        let table = Table::new(rows).to_string();
        println!("{}", table);
    }

    Ok(())
}

async fn delete_backup(
    backup_id: Option<String>,
    older_than: Option<u32>,
    cli: &Cli,
) -> Result<()> {
    if let Some(days) = older_than {
        // Delete backups older than N days
        if !cli.yes {
            print!("  Delete all backups older than {} days? [y/N] ", days);
            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("{} Cancelled", "!".yellow());
                return Ok(());
            }
        }

        // Get backups to delete for file cleanup
        let old_backups = database::backups::get_older_than(days).await?;

        for backup in old_backups {
            // Delete file
            if Path::new(&backup.file_path).exists() {
                tokio::fs::remove_file(&backup.file_path).await?;
            }
        }

        let deleted = database::backups::delete_older_than(days).await?;
        println!(
            "{} Deleted {} backup(s) older than {} days",
            "✓".green(),
            deleted,
            days
        );
    } else if let Some(id_str) = backup_id {
        let id: i64 = id_str.parse().map_err(|_| anyhow!("Invalid backup ID"))?;

        let backup = database::backups::get_by_id(id)
            .await?
            .ok_or_else(|| anyhow!("Backup not found: {}", id))?;

        if !cli.yes {
            print!("  Delete backup {} ({})? [y/N] ", id, backup.domain);
            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("{} Cancelled", "!".yellow());
                return Ok(());
            }
        }

        // Delete file
        if Path::new(&backup.file_path).exists() {
            tokio::fs::remove_file(&backup.file_path).await?;
        }

        // Delete record
        database::backups::delete(id).await?;
        println!("{} Backup {} deleted", "✓".green(), id);
    } else {
        return Err(anyhow!(
            "Either backup_id or --older-than must be specified"
        ));
    }

    Ok(())
}

async fn get_mysql_root_password() -> Result<String> {
    let cred_path = "/etc/rustwops/credentials/mysql.cnf";
    if Path::new(cred_path).exists() {
        let content = tokio::fs::read_to_string(cred_path).await?;
        // Parse password from [client] section: password=xxxxx
        for line in content.lines() {
            if let Some(pass) = line.strip_prefix("password=") {
                return Ok(pass.to_string());
            }
        }
        Err(anyhow!("Password not found in MySQL credentials file"))
    } else {
        Err(anyhow!(
            "MySQL credentials not found. Run 'rw stack install mysql' first."
        ))
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
