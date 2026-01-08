use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

use crate::commands;
use crate::utils::system::is_root;
use crate::Cli;

/// Run a CLI command with privilege escalation if needed
/// This allows interactive mode to work without root,
/// prompting for sudo only when a privileged action is selected
async fn run_privileged_command(args: &[&str]) -> Result<()> {
    use std::process::Stdio;
    use tokio::process::Command;

    let current_exe = std::env::current_exe()?;
    let exe_path = current_exe.to_str().unwrap_or("rw");

    if is_root() {
        // Already root, run directly
        let status = Command::new(exe_path)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("Command failed");
        }
    } else {
        // Need sudo - show what we're doing
        println!(
            "\n{} This action requires elevated privileges.",
            "→".bright_cyan()
        );
        println!(
            "  Running: {} {} {}\n",
            "sudo".yellow(),
            exe_path,
            args.join(" ")
        );

        let status = Command::new("sudo")
            .arg(exe_path)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("Command failed");
        }
    }

    Ok(())
}

pub async fn run() -> Result<()> {
    print_banner();

    // Initialize database if needed
    crate::database::ensure_initialized().await?;

    loop {
        let selection = main_menu()?;

        match selection {
            MainMenu::Stack => stack_menu().await?,
            MainMenu::Site => site_menu().await?,
            MainMenu::Backup => backup_menu().await?,
            MainMenu::Security => security_menu().await?,
            MainMenu::Logs => logs_menu().await?,
            MainMenu::Service => service_menu().await?,
            MainMenu::Info => info_menu().await?,
            MainMenu::Exit => {
                println!("\nGoodbye.\n");
                break;
            }
        }
    }

    Ok(())
}

fn print_banner() {
    println!(
        "{}",
        r#"
  ____           _   __        __
 |  _ \ _   _ ___| |_ \ \      / /__  _ __  ___
 | |_) | | | / __| __| \ \ /\ / / _ \| '_ \/ __|
 |  _ <| |_| \__ \ |_   \ V  V / (_) | |_) \__ \
 |_| \_\\__,_|___/\__|   \_/\_/ \___/| .__/|___/
                                     |_|
"#
        .bright_cyan()
    );
    println!(
        "  {} {}\n",
        "RustWops".bright_white().bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
}

#[derive(Debug, Clone, Copy)]
enum MainMenu {
    Stack,
    Site,
    Backup,
    Security,
    Logs,
    Service,
    Info,
    Exit,
}

fn main_menu() -> Result<MainMenu> {
    let items = vec![
        "Stack      Manage server components (Nginx, PHP, MySQL, Redis)",
        "Sites      Create and manage websites",
        "Backup     Create and restore site backups",
        "Security   Fail2Ban, ClamAV, MySQLTuner tools",
        "Logs       View site, nginx, php, mysql, fail2ban logs",
        "Services   Start, stop, restart services",
        "Info       Show system information",
        "Exit",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Main Menu")
        .items(&items)
        .default(1) // Default to Sites
        .interact()?;

    Ok(match selection {
        0 => MainMenu::Stack,
        1 => MainMenu::Site,
        2 => MainMenu::Backup,
        3 => MainMenu::Security,
        4 => MainMenu::Logs,
        5 => MainMenu::Service,
        6 => MainMenu::Info,
        _ => MainMenu::Exit,
    })
}

// ============================================================================
// Stack Menu
// ============================================================================

async fn stack_menu() -> Result<()> {
    let items = vec![
        "Install stack components",
        "Remove stack components",
        "Update stack components",
        "Show stack status",
        "Manage PHP versions",
        "← Back to main menu",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Stack Management")
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => stack_install().await?,
        1 => stack_remove().await?,
        2 => stack_update().await?,
        3 => stack_status().await?,
        4 => php_versions_menu().await?,
        _ => {}
    }

    Ok(())
}

async fn stack_install() -> Result<()> {
    let components = vec![
        "Nginx",
        "PHP",
        "MySQL/MariaDB",
        "Redis",
        "Node.js",
        "─────────────────", // Separator
        "Fail2Ban (intrusion prevention)",
        "ClamAV (antivirus)",
        "MySQLTuner (database optimizer)",
    ];

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select components to install (Space to select, Enter to confirm)")
        .items(&components)
        .interact()?;

    // Filter out separator selection
    let selections: Vec<usize> = selections.into_iter().filter(|&i| i != 5).collect();

    if selections.is_empty() {
        println!("{} No components selected.\n", "→".yellow());
        return Ok(());
    }

    // PHP version
    let php_version = if selections.contains(&1) {
        let versions = vec!["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];
        let idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select PHP version")
            .items(&versions)
            .default(1) // 8.3
            .interact()?;
        versions[idx].to_string()
    } else {
        "8.3".to_string()
    };

    // Database type
    let db_type = if selections.contains(&2) {
        let dbs = vec!["MariaDB (recommended)", "MySQL"];
        let idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select database type")
            .items(&dbs)
            .default(0)
            .interact()?;
        if idx == 0 {
            commands::stack::DbType::Mariadb
        } else {
            commands::stack::DbType::Mysql
        }
    } else {
        commands::stack::DbType::Mariadb
    };

    // Node.js version
    let node_version = if selections.contains(&4) {
        let versions = vec!["22 (Current)", "20 (LTS)", "18 (LTS)"];
        let idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select Node.js version")
            .items(&versions)
            .default(1) // 20 LTS
            .interact()?;
        match idx {
            0 => "22",
            1 => "20",
            _ => "18",
        }
        .to_string()
    } else {
        "20".to_string()
    };

    // Confirm
    println!("\n{} Installation Summary:", "→".bright_cyan());
    for &idx in &selections {
        if idx != 5 {
            // Skip separator
            println!("  • {}", components[idx]);
        }
    }
    if selections.contains(&1) {
        println!("  • PHP version: {}", php_version);
    }
    if selections.contains(&2) {
        println!("  • Database: {:?}", db_type);
    }
    if selections.contains(&4) {
        println!("  • Node.js: {}", node_version);
    }
    println!();

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed with installation?")
        .default(true)
        .interact()?;

    if !confirm {
        println!("{} Installation cancelled.\n", "→".yellow());
        return Ok(());
    }

    // Build CLI arguments for privileged command
    let mut args = vec!["stack", "install"];

    // Component names for CLI
    let mut comp_args: Vec<&str> = Vec::new();
    for &idx in &selections {
        match idx {
            0 => comp_args.push("nginx"),
            1 => comp_args.push("php"),
            2 => comp_args.push("mysql"),
            3 => comp_args.push("redis"),
            4 => comp_args.push("nodejs"),
            6 => comp_args.push("fail2ban"),
            7 => comp_args.push("clamav"),
            8 => comp_args.push("mysqltuner"),
            _ => {}
        }
    }

    // Add components to args
    for comp in &comp_args {
        args.push(comp);
    }

    // Add options
    let php_arg = format!("--php-version={}", php_version);
    args.push(&php_arg);

    let db_arg = format!(
        "--db-type={}",
        match db_type {
            commands::stack::DbType::Mariadb => "mariadb",
            commands::stack::DbType::Mysql => "mysql",
        }
    );
    args.push(&db_arg);

    let node_arg = format!("--node-version={}", node_version);
    args.push(&node_arg);

    args.push("-y"); // Skip confirmation since we already confirmed

    run_privileged_command(&args).await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn stack_remove() -> Result<()> {
    let components = vec!["Nginx", "PHP", "MySQL/MariaDB", "Redis", "Node.js"];

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select components to REMOVE (Space to select, Enter to confirm)")
        .items(&components)
        .interact()?;

    if selections.is_empty() {
        println!("{} No components selected.\n", "→".yellow());
        return Ok(());
    }

    let purge = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Purge configuration files too?")
        .default(false)
        .interact()?;

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Are you SURE you want to remove these components?")
        .default(false)
        .interact()?;

    if !confirm {
        println!("{} Removal cancelled.\n", "→".yellow());
        return Ok(());
    }

    // Build CLI arguments for privileged command
    let mut args = vec!["stack", "remove"];

    for &idx in &selections {
        match idx {
            0 => args.push("nginx"),
            1 => args.push("php"),
            2 => args.push("mysql"),
            3 => args.push("redis"),
            4 => args.push("nodejs"),
            _ => {}
        }
    }

    if purge {
        args.push("--purge");
    }
    args.push("-y");

    run_privileged_command(&args).await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn stack_update() -> Result<()> {
    let items = vec![
        "Update all components",
        "Select specific components",
        "← Back",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Update Stack")
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // Update all components
            run_privileged_command(&["stack", "update", "-y"]).await?;
        }
        1 => {
            let components = vec!["Nginx", "PHP", "MySQL/MariaDB", "Redis", "Node.js"];
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Select components to update")
                .items(&components)
                .interact()?;

            let mut args = vec!["stack", "update"];
            for &idx in &selections {
                match idx {
                    0 => args.push("nginx"),
                    1 => args.push("php"),
                    2 => args.push("mysql"),
                    3 => args.push("redis"),
                    4 => args.push("nodejs"),
                    _ => {}
                }
            }
            args.push("-y");

            run_privileged_command(&args).await?;
        }
        _ => return Ok(()),
    }

    press_enter_to_continue()?;
    Ok(())
}

async fn stack_status() -> Result<()> {
    let cli = create_cli(true, false);
    commands::stack::status::execute(&cli).await?;
    press_enter_to_continue()?;
    Ok(())
}

async fn php_versions_menu() -> Result<()> {
    let items = vec![
        "List installed PHP versions",
        "Install additional PHP version",
        "← Back",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("PHP Version Management")
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // List PHP versions (read-only, no sudo needed)
            let cli = create_cli(false, false);
            commands::stack::install::list_php_versions(&cli).await?;
        }
        1 => {
            let versions = vec!["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];
            let idx = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select PHP version to install")
                .items(&versions)
                .default(0)
                .interact()?;

            let php_arg = format!("--php-version={}", versions[idx]);
            run_privileged_command(&["stack", "php-install", &php_arg, "-y"]).await?;
        }
        _ => return Ok(()),
    }

    press_enter_to_continue()?;
    Ok(())
}

// ============================================================================
// Security Menu
// ============================================================================

async fn security_menu() -> Result<()> {
    loop {
        let items = vec![
            "Security status",
            "Run MySQLTuner (database analysis)",
            "Run ClamAV scan",
            "Update virus definitions",
            "Fail2Ban management",
            "← Back to main menu",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Security Tools")
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                // Status is read-only, no sudo needed
                commands::security::execute(
                    commands::security::SecurityCommand::Status,
                    &create_cli(false, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            1 => {
                // MySQLTuner requires root
                run_privileged_command(&["security", "mysqltuner"]).await?;
                press_enter_to_continue()?;
            }
            2 => {
                clamav_scan_menu().await?;
            }
            3 => {
                // Update definitions requires root
                run_privileged_command(&["security", "update-definitions"]).await?;
                press_enter_to_continue()?;
            }
            4 => {
                fail2ban_menu().await?;
            }
            _ => return Ok(()),
        }
    }
}

async fn clamav_scan_menu() -> Result<()> {
    let path: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Directory to scan")
        .default("/var/www".to_string())
        .interact_text()?;

    let quarantine = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Move infected files to quarantine?")
        .default(true)
        .interact()?;

    // ClamAV scan requires root
    let path_arg = format!("--path={}", path);
    let mut args = vec!["security", "scan", &path_arg];
    if quarantine {
        args.push("--quarantine");
    }

    run_privileged_command(&args).await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn fail2ban_menu() -> Result<()> {
    loop {
        let items = vec![
            "Show status",
            "Show banned IPs",
            "Unban an IP",
            "Ban an IP",
            "Show recent logs",
            "← Back",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Fail2Ban Management")
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                // Status is read-only, no sudo needed
                commands::security::execute(
                    commands::security::SecurityCommand::Fail2ban {
                        action: commands::security::Fail2banAction::Status,
                    },
                    &create_cli(false, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            1 => {
                // Banned list is read-only, no sudo needed
                commands::security::execute(
                    commands::security::SecurityCommand::Fail2ban {
                        action: commands::security::Fail2banAction::Banned,
                    },
                    &create_cli(false, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            2 => {
                let ip: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("IP address to unban")
                    .interact_text()?;

                // Unban requires root
                run_privileged_command(&["security", "fail2ban", "unban", &ip]).await?;
                press_enter_to_continue()?;
            }
            3 => {
                let ip: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("IP address to ban")
                    .interact_text()?;

                let jails = vec![
                    "sshd",
                    "nginx-http-auth",
                    "nginx-botsearch",
                    "nginx-forbidden",
                    "wordpress",
                    "recidive",
                ];
                let jail_idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select jail")
                    .items(&jails)
                    .default(0)
                    .interact()?;

                // Ban requires root
                let jail_arg = format!("--jail={}", jails[jail_idx]);
                run_privileged_command(&["security", "fail2ban", "ban", &ip, &jail_arg]).await?;
                press_enter_to_continue()?;
            }
            4 => {
                // Logs are read-only, no sudo needed
                commands::security::execute(
                    commands::security::SecurityCommand::Fail2ban {
                        action: commands::security::Fail2banAction::Logs { lines: 50 },
                    },
                    &create_cli(false, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            _ => return Ok(()),
        }
    }
}

// ============================================================================
// Site Menu
// ============================================================================

async fn site_menu() -> Result<()> {
    loop {
        // Get all sites from database (refresh each loop)
        let all_sites = crate::database::sites::list().await?;
        let staging_entries = crate::database::staging::list().await?;

        // Filter out staging sites - only show production sites
        let staging_domains: Vec<&str> = staging_entries
            .iter()
            .map(|s| s.staging_domain.as_str())
            .collect();
        let sites: Vec<_> = all_sites
            .into_iter()
            .filter(|s| !staging_domains.contains(&s.domain.as_str()))
            .collect();

        if sites.is_empty() {
            let items = vec!["Create new site", "Back"];

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Sites (none found)")
                .items(&items)
                .default(0)
                .interact()?;

            if selection == 0 {
                site_create().await?;
            } else {
                return Ok(());
            }
            continue;
        }

        // Build professional site list
        let mut items: Vec<String> = sites
            .iter()
            .map(|s| {
                let has_staging = staging_entries
                    .iter()
                    .any(|st| st.production_site_id == s.id);
                let staging_indicator = if has_staging { " [+staging]" } else { "" };
                format!(
                    "{:<30} {:>8}{}",
                    s.domain,
                    s.site_type.to_uppercase(),
                    staging_indicator
                )
            })
            .collect();

        items.insert(0, "[+] Create new site".to_string());
        items.push("[<] Back".to_string());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select site")
            .items(&items)
            .default(0)
            .interact()?;

        if selection == 0 {
            site_create().await?;
        } else if selection == items.len() - 1 {
            return Ok(());
        } else {
            let site = sites[selection - 1].clone();
            site_actions_menu(&site).await?;
        }
    }
}

async fn site_actions_menu(site: &crate::database::sites::Site) -> Result<()> {
    let is_wordpress = site.site_type == "wp";

    // Check if this site has a staging environment
    let staging_entries = crate::database::staging::list().await?;
    let has_staging = staging_entries
        .iter()
        .any(|s| s.production_site_id == site.id);

    loop {
        let mut items = vec!["Site info".to_string(), "SSL certificate".to_string()];

        // Enable/Disable option
        if site.enabled {
            items.push("Disable site".to_string());
        } else {
            items.push("Enable site".to_string());
        }

        // Update site option (PHP version, cache type for WP)
        if site.site_type == "wp" || site.site_type == "php" {
            items.push("Update site".to_string());
        }

        // Staging option
        if has_staging {
            items.push("Staging environment".to_string());
        } else {
            items.push("Create staging".to_string());
        }

        // WordPress-specific actions
        if is_wordpress {
            items.push("Purge cache".to_string());
            items.push("Reset admin password".to_string());
            items.push("WP-CLI commands".to_string());
        }

        // Node.js-specific actions
        if site.site_type == "node" {
            items.push("PM2 management".to_string());
        }

        items.push("Delete site".to_string());
        items.push("Back".to_string());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "{} ({})",
                site.domain,
                site.site_type.to_uppercase()
            ))
            .items(&items)
            .default(0)
            .interact()?;

        // Calculate action index
        let mut idx = 0;

        if selection == idx {
            // Site info
            let cli = create_cli(false, false);
            commands::site::info::execute(&site.domain, &cli).await?;
            press_enter_to_continue()?;
            continue;
        }
        idx += 1;

        if selection == idx {
            // SSL
            site_ssl_menu(&site.domain).await?;
            continue;
        }
        idx += 1;

        if selection == idx {
            // Enable/Disable
            site_enable_disable(&site.domain, site.enabled).await?;
            // Refresh site status
            return Ok(());
        }
        idx += 1;

        // Update site option (only for wp/php sites)
        if site.site_type == "wp" || site.site_type == "php" {
            if selection == idx {
                site_update_menu(site).await?;
                return Ok(());
            }
            idx += 1;
        }

        if selection == idx {
            // Staging
            if has_staging {
                site_staging_menu(site).await?;
            } else {
                create_staging_for_site(site).await?;
            }
            // Refresh staging status
            return Ok(());
        }
        idx += 1;

        if is_wordpress {
            if selection == idx {
                // Purge cache
                cache_purge_menu(&site.domain).await?;
                continue;
            }
            idx += 1;

            if selection == idx {
                // Reset password
                wp_reset_password(&site.domain).await?;
                continue;
            }
            idx += 1;

            if selection == idx {
                // WP-CLI
                wp_cli_shell(&site.domain).await?;
                continue;
            }
            idx += 1;
        }

        // Node.js PM2 management
        if site.site_type == "node" {
            if selection == idx {
                pm2_menu(&site.domain).await?;
                continue;
            }
            idx += 1;
        }

        if selection == idx {
            // Delete
            site_delete_confirm(&site.domain).await?;
            return Ok(());
        }

        // Back
        return Ok(());
    }
}

async fn create_staging_for_site(site: &crate::database::sites::Site) -> Result<()> {
    let prefix: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Staging subdomain prefix")
        .default("staging".to_string())
        .interact_text()?;

    let staging_domain = format!("{}.{}", prefix, site.domain);

    println!("\n{} Creating staging environment...", "→".bright_cyan());
    println!("  Production: {}", site.domain);
    println!("  Staging:    {}", staging_domain);

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed?")
        .default(true)
        .interact()?;

    if !confirm {
        return Ok(());
    }

    // Staging create requires root
    let prefix_arg = format!("--prefix={}", prefix);
    run_privileged_command(&["staging", "create", &site.domain, &prefix_arg, "-y"]).await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn site_ssl_menu(domain: &str) -> Result<()> {
    loop {
        let items = vec!["Issue/renew certificate", "Show SSL status", "Back"];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("SSL for {}", domain))
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                let wildcard = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Issue wildcard certificate?")
                    .default(false)
                    .interact()?;

                if wildcard {
                    let providers = vec!["Cloudflare", "DigitalOcean", "AWS Route53"];
                    let idx = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select DNS provider")
                        .items(&providers)
                        .default(0)
                        .interact()?;

                    let dns_provider = match idx {
                        0 => "cloudflare",
                        1 => "digitalocean",
                        _ => "route53",
                    };

                    // SSL issue with DNS requires root
                    run_privileged_command(&[
                        "ssl",
                        "issue",
                        domain,
                        "--wildcard",
                        "--dns",
                        dns_provider,
                        "-v",
                    ])
                    .await?;
                } else {
                    // SSL issue with HTTP requires root
                    run_privileged_command(&["ssl", "issue", domain, "-v"]).await?;
                }
                press_enter_to_continue()?;
            }
            1 => {
                // SSL status is read-only
                let cli = create_cli(false, false);
                commands::ssl::execute(
                    commands::ssl::SslCommand::Status {
                        domain: Some(domain.to_string()),
                    },
                    &cli,
                )
                .await?;
                press_enter_to_continue()?;
            }
            _ => return Ok(()),
        }
    }
}

async fn site_staging_menu(site: &crate::database::sites::Site) -> Result<()> {
    let is_wordpress = site.site_type == "wp";

    // Get staging info
    let staging_entries = crate::database::staging::list().await?;
    let staging_entry = staging_entries
        .iter()
        .find(|s| s.production_site_id == site.id);

    let staging_domain = match staging_entry {
        Some(s) => s.staging_domain.clone(),
        None => return Ok(()), // No staging, shouldn't happen if called correctly
    };

    loop {
        let mut items = vec![
            "Staging site info".to_string(),
            "Sync: Production -> Staging".to_string(),
            "Promote: Staging -> Production".to_string(),
        ];

        // WordPress-specific actions for staging site
        if is_wordpress {
            items.push("Reset staging admin password".to_string());
            items.push("Staging WP-CLI commands".to_string());
        }

        items.push("Delete staging environment".to_string());
        items.push("Back".to_string());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Staging: {}", staging_domain))
            .items(&items)
            .default(0)
            .interact()?;

        let mut idx = 0;

        if selection == idx {
            // Staging info
            let cli = create_cli(false, false);
            commands::staging::execute(
                commands::staging::StagingCommand::Info {
                    domain: site.domain.clone(),
                },
                &cli,
            )
            .await?;
            press_enter_to_continue()?;
            continue;
        }
        idx += 1;

        if selection == idx {
            // Sync prod -> staging (requires root)
            run_privileged_command(&[
                "staging",
                "sync",
                &site.domain,
                "--direction=prod-to-stage",
                "-y",
            ])
            .await?;
            press_enter_to_continue()?;
            continue;
        }
        idx += 1;

        if selection == idx {
            // Promote staging -> prod
            println!(
                "\n{} This will overwrite production with staging data!",
                "WARNING:".red().bold()
            );
            let confirm = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Are you absolutely sure?")
                .default(false)
                .interact()?;

            if confirm {
                // Sync stage -> prod (requires root)
                run_privileged_command(&[
                    "staging",
                    "sync",
                    &site.domain,
                    "--direction=stage-to-prod",
                    "-y",
                ])
                .await?;
            }
            press_enter_to_continue()?;
            continue;
        }
        idx += 1;

        if is_wordpress {
            if selection == idx {
                // Reset staging password
                wp_reset_password(&staging_domain).await?;
                continue;
            }
            idx += 1;

            if selection == idx {
                // Staging WP-CLI
                wp_cli_shell(&staging_domain).await?;
                continue;
            }
            idx += 1;
        }

        if selection == idx {
            // Delete staging
            let confirm = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Delete staging environment?")
                .default(false)
                .interact()?;

            if confirm {
                // Staging delete requires root
                run_privileged_command(&["staging", "delete", &site.domain, "-y"]).await?;
                press_enter_to_continue()?;
                return Ok(());
            }
            continue;
        }

        // Back
        return Ok(());
    }
}

async fn wp_reset_password(domain: &str) -> Result<()> {
    println!(
        "\n{} Reset WordPress Admin Password\n",
        "→".bright_cyan().bold()
    );

    // Get the actual webroot from the database
    let site = crate::database::sites::get_by_domain(domain).await?;
    let webroot = match site {
        Some(s) => s.webroot,
        None => {
            println!("{} Site not found in database.\n", "✗".red().bold());
            return Ok(());
        }
    };

    let username: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("WordPress admin username")
        .default("admin".to_string())
        .interact_text()?;

    let password: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("New password (leave empty to generate)")
        .allow_empty(true)
        .interact_text()?;

    let password = if password.is_empty() {
        // Generate a random password using shell-safe characters only
        use rand::Rng;
        let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::thread_rng();
        let generated: String = (0..24)
            .map(|_| {
                let idx = rng.gen_range(0..charset.len());
                charset[idx] as char
            })
            .collect();
        println!(
            "\n{} Generated password: {}\n",
            "→".bright_cyan(),
            generated.bright_white().bold()
        );
        generated
    } else {
        password
    };

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Reset password for user '{}'?", username))
        .default(true)
        .interact()?;

    if !confirm {
        println!("{} Cancelled.\n", "→".yellow());
        return Ok(());
    }

    // Run wp-cli to reset password
    // Escape special characters in password for shell safety
    let escaped_password = password.replace('\\', "\\\\").replace('\'', "'\\''");
    let cmd = format!(
        "cd {} && sudo -u www-data wp user update {} --user_pass='{}'",
        webroot, username, escaped_password
    );

    match crate::utils::shell::run_shell_script(&cmd, true).await {
        Ok(_) => {
            println!("\n{} Password reset successfully!", "✓".green().bold());
            println!("  Username: {}", username.bright_white());
            println!("  Password: {}", password.bright_white());
            println!();
        }
        Err(e) => {
            println!("\n{} Failed to reset password: {}\n", "✗".red().bold(), e);
        }
    }

    press_enter_to_continue()?;
    Ok(())
}

async fn wp_cli_shell(domain: &str) -> Result<()> {
    // Get the actual webroot from the database
    let site = crate::database::sites::get_by_domain(domain).await?;
    let webroot = match site {
        Some(s) => s.webroot,
        None => {
            println!("{} Site not found in database.\n", "✗".red().bold());
            return Ok(());
        }
    };

    loop {
        println!("\n{} WP-CLI Quick Commands\n", "→".bright_cyan().bold());

        let items = vec![
            "List all users",
            "Show site options",
            "Check for updates",
            "Flush cache",
            "Run custom command",
            "← Back",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select WP-CLI command")
            .items(&items)
            .default(0)
            .interact()?;

        let wp_cmd = match selection {
            0 => Some("wp user list"),
            1 => Some("wp option list --autoload=yes"),
            2 => Some("wp core check-update && wp plugin list --update=available && wp theme list --update=available"),
            3 => Some("wp cache flush && wp rewrite flush"),
            4 => {
                let cmd: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter WP-CLI command (without 'wp')")
                    .interact_text()?;
                let full_cmd = format!("cd {} && sudo -u www-data wp {}", webroot, cmd);
                match crate::utils::shell::run_shell_script(&full_cmd, true).await {
                    Ok(output) => println!("\n{}\n", output),
                    Err(e) => println!("\n{} Error: {}\n", "✗".red(), e),
                }
                press_enter_to_continue()?;
                continue;
            }
            _ => return Ok(()),
        };

        if let Some(cmd) = wp_cmd {
            let full_cmd = format!("cd {} && sudo -u www-data {}", webroot, cmd);
            match crate::utils::shell::run_shell_script(&full_cmd, true).await {
                Ok(output) => println!("\n{}\n", output),
                Err(e) => println!("\n{} Error: {}\n", "✗".red(), e),
            }
            press_enter_to_continue()?;
        }
    }
}

async fn cache_purge_menu(domain: &str) -> Result<()> {
    let items = vec![
        "Purge all caches (page + object)",
        "Purge page cache only (FastCGI/Redis)",
        "Purge object cache only (Redis)",
        "← Back",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Cache purge for {}", domain))
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // Purge all caches requires root
            run_privileged_command(&["site", "cache-purge", domain, "--all"]).await?;
            press_enter_to_continue()?;
        }
        1 => {
            // Purge page cache requires root
            run_privileged_command(&["site", "cache-purge", domain, "--page"]).await?;
            press_enter_to_continue()?;
        }
        2 => {
            // Purge object cache requires root
            run_privileged_command(&["site", "cache-purge", domain, "--object"]).await?;
            press_enter_to_continue()?;
        }
        _ => {}
    }

    Ok(())
}

async fn pm2_menu(domain: &str) -> Result<()> {
    loop {
        let items = vec![
            "Show status",
            "Start app",
            "Stop app",
            "Restart app",
            "View logs",
            "← Back",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("PM2 for {}", domain))
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                // Status requires root
                run_privileged_command(&["site", "pm2", domain, "status"]).await?;
                press_enter_to_continue()?;
            }
            1 => {
                // Start requires root
                run_privileged_command(&["site", "pm2", domain, "start"]).await?;
                press_enter_to_continue()?;
            }
            2 => {
                // Stop requires root
                run_privileged_command(&["site", "pm2", domain, "stop"]).await?;
                press_enter_to_continue()?;
            }
            3 => {
                // Restart requires root
                run_privileged_command(&["site", "pm2", domain, "restart"]).await?;
                press_enter_to_continue()?;
            }
            4 => {
                // Logs requires root (runs interactively)
                println!(
                    "\n{} Press Ctrl+C to stop viewing logs\n",
                    "→".bright_cyan()
                );
                run_privileged_command(&["site", "pm2", domain, "logs"]).await?;
            }
            _ => return Ok(()),
        }
    }
}

async fn site_delete_confirm(domain: &str) -> Result<()> {
    let delete_options = vec![
        "Delete everything (files + database)",
        "Delete files only",
        "Delete database only",
        "← Cancel",
    ];

    let idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Delete {} - What to delete?", domain))
        .items(&delete_options)
        .default(0)
        .interact()?;

    if idx == 3 {
        return Ok(());
    }

    let (all, files, db) = match idx {
        0 => (true, false, false),
        1 => (false, true, false),
        _ => (false, false, true),
    };

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Are you SURE you want to delete {}?", domain))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{} Deletion cancelled.\n", "→".yellow());
        return Ok(());
    }

    // Build CLI arguments for privileged command
    let mut args = vec!["site", "delete", domain];
    if all {
        args.push("--all");
    }
    if files {
        args.push("--files");
    }
    if db {
        args.push("--db");
    }
    args.push("-y");

    run_privileged_command(&args).await?;
    press_enter_to_continue()?;
    Ok(())
}

async fn site_create() -> Result<()> {
    // Domain
    let domain: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter domain name")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.contains('.') && input.len() > 3 {
                Ok(())
            } else {
                Err("Please enter a valid domain (e.g., example.com)")
            }
        })
        .interact_text()?;

    // Site type
    let site_types = vec![
        "WordPress",
        "PHP",
        "Static (HTML/CSS/JS)",
        "Node.js (with PM2)",
        "Reverse Proxy",
    ];
    let type_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select site type")
        .items(&site_types)
        .default(0)
        .interact()?;

    let site_type = match type_idx {
        0 => commands::site::SiteType::Wp,
        1 => commands::site::SiteType::Php,
        2 => commands::site::SiteType::Static,
        3 => commands::site::SiteType::Node,
        _ => commands::site::SiteType::Proxy,
    };

    // PHP version (for WP/PHP sites)
    let php_version = if type_idx <= 1 {
        // Get installed PHP versions
        let installed = crate::config::php::get_installed_versions().await;

        if installed.is_empty() {
            println!("\n{} No PHP versions installed!", "Error:".red().bold());
            println!("  Install PHP first with: rw stack install php\n");
            return Ok(());
        }

        // Build version list showing which are installed
        let all_versions = ["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];
        let version_items: Vec<String> = all_versions
            .iter()
            .map(|v| {
                if installed.contains(&v.to_string()) {
                    format!("{} (installed)", v)
                } else {
                    format!("{} (not installed)", v)
                }
            })
            .collect();

        let idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select PHP version")
            .items(&version_items)
            .default(0)
            .interact()?;

        let selected_version = all_versions[idx].to_string();

        // Check if selected version is installed
        if !installed.contains(&selected_version) {
            println!(
                "\n{} PHP {} is not installed!",
                "Error:".red().bold(),
                selected_version
            );
            println!(
                "  Install it with: rw stack install php --php-version {}\n",
                selected_version
            );
            return Ok(());
        }

        selected_version
    } else {
        "8.4".to_string()
    };

    // Cache type (for WP)
    let cache = if type_idx == 0 {
        let cache_types = vec!["No caching", "FastCGI Cache (Nginx)", "Redis Object Cache"];
        let idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select cache type")
            .items(&cache_types)
            .default(0)
            .interact()?;
        match idx {
            1 => Some(commands::site::CacheType::Fastcgi),
            2 => Some(commands::site::CacheType::Redis),
            _ => None,
        }
    } else {
        None
    };

    // Upstream port (for proxy/node)
    let upstream = if type_idx >= 3 {
        let port: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter upstream port")
            .default("3000".to_string())
            .interact_text()?;
        Some(port.parse::<u16>().unwrap_or(3000))
    } else {
        None
    };

    // SSL
    let ssl = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable SSL certificate?")
        .default(true)
        .interact()?;

    let (wildcard, dns_provider) = if ssl {
        let wildcard = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Issue wildcard certificate? (*.domain.com)")
            .default(false)
            .interact()?;

        let dns = if wildcard {
            let providers = vec!["Cloudflare", "DigitalOcean", "AWS Route53"];
            let idx = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select DNS provider for wildcard")
                .items(&providers)
                .default(0)
                .interact()?;
            Some(match idx {
                0 => commands::site::DnsProvider::Cloudflare,
                1 => commands::site::DnsProvider::Digitalocean,
                _ => commands::site::DnsProvider::Route53,
            })
        } else {
            None
        };

        (wildcard, dns)
    } else {
        (false, None)
    };

    // Summary
    println!("\n{} Site Creation Summary:", "→".bright_cyan());
    println!("  Domain:    {}", domain.bright_white());
    println!("  Type:      {:?}", site_type);
    if type_idx <= 1 {
        println!("  PHP:       {}", php_version);
    }
    if let Some(ref c) = cache {
        println!("  Cache:     {:?}", c);
    }
    if let Some(p) = upstream {
        println!("  Upstream:  port {}", p);
    }
    println!("  SSL:       {}", if ssl { "Yes" } else { "No" });
    if wildcard {
        println!("  Wildcard:  Yes");
    }
    println!();

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Create this site?")
        .default(true)
        .interact()?;

    if !confirm {
        println!("{} Site creation cancelled.\n", "→".yellow());
        return Ok(());
    }

    // Build CLI arguments for privileged command
    let mut args = vec!["site", "create", &domain];

    // Site type
    let type_arg = format!(
        "--type={}",
        match site_type {
            commands::site::SiteType::Wp => "wp",
            commands::site::SiteType::Php => "php",
            commands::site::SiteType::Static => "static",
            commands::site::SiteType::Node => "node",
            commands::site::SiteType::Proxy => "proxy",
        }
    );
    args.push(&type_arg);

    // PHP version
    let php_arg = format!("--php={}", php_version);
    if type_idx <= 1 {
        args.push(&php_arg);
    }

    // MySQL for wp/php
    if type_idx <= 1 {
        args.push("--mysql");
    }

    // Cache
    let cache_arg;
    if let Some(ref c) = cache {
        cache_arg = format!(
            "--cache={}",
            match c {
                commands::site::CacheType::Fastcgi => "fastcgi",
                commands::site::CacheType::Redis => "redis",
                commands::site::CacheType::None => "none",
            }
        );
        args.push(&cache_arg);
    }

    // SSL
    if ssl {
        args.push("--ssl");
    }
    if wildcard {
        args.push("--wildcard");
    }

    // DNS provider
    let dns_arg;
    if let Some(ref dns) = dns_provider {
        dns_arg = format!(
            "--dns={}",
            match dns {
                commands::site::DnsProvider::Cloudflare => "cloudflare",
                commands::site::DnsProvider::Digitalocean => "digitalocean",
                commands::site::DnsProvider::Route53 => "route53",
            }
        );
        args.push(&dns_arg);
    }

    // Upstream port
    let upstream_arg;
    if let Some(p) = upstream {
        upstream_arg = format!("--upstream={}", p);
        args.push(&upstream_arg);
    }

    args.push("-y");

    run_privileged_command(&args).await?;

    press_enter_to_continue()?;
    Ok(())
}

// ============================================================================
// Logs Menu
// ============================================================================

async fn logs_menu() -> Result<()> {
    loop {
        let items = vec![
            "View site logs",
            "View all sites (summary)",
            "Nginx logs",
            "PHP-FPM logs",
            "MySQL/MariaDB logs",
            "Fail2Ban logs",
            "← Back",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Log Viewer")
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => view_site_logs_interactive().await?,
            1 => view_all_sites_logs().await?,
            2 => view_nginx_logs_interactive().await?,
            3 => view_php_logs_interactive().await?,
            4 => view_mysql_logs_interactive().await?,
            5 => view_fail2ban_logs_interactive().await?,
            _ => return Ok(()),
        }
    }
}

async fn view_site_logs_interactive() -> Result<()> {
    // Get list of sites
    let sites = crate::database::sites::list().await?;

    if sites.is_empty() {
        println!("\n{} No sites found", "!".yellow());
        press_enter_to_continue()?;
        return Ok(());
    }

    // Build site list for selection
    let site_items: Vec<String> = sites.iter().map(|s| s.domain.clone()).collect();

    let site_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select site")
        .items(&site_items)
        .default(0)
        .interact()?;

    let domain = &sites[site_idx].domain;

    // Log type selection
    let log_types = vec![
        "Error logs only",
        "Access logs only",
        "PHP-FPM logs",
        "All logs",
    ];

    let log_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Log type")
        .items(&log_types)
        .default(0)
        .interact()?;

    let (errors, access, php) = match log_type {
        0 => (true, false, false),
        1 => (false, true, false),
        2 => (false, false, true),
        _ => (false, false, false),
    };

    // Lines to show
    let lines: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of lines")
        .default(50)
        .interact_text()?;

    // Follow option
    let follow = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Follow log in real-time? (Ctrl+C to stop)")
        .default(false)
        .interact()?;

    commands::log::execute(
        commands::log::LogCommand::Site {
            domain: Some(domain.clone()),
            errors,
            access,
            php,
            follow,
            n: lines,
            status: None,
            ip: None,
        },
        &create_cli(false, false),
    )
    .await?;

    if !follow {
        press_enter_to_continue()?;
    }
    Ok(())
}

async fn view_all_sites_logs() -> Result<()> {
    let errors_only = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Show only errors?")
        .default(true)
        .interact()?;

    commands::log::execute(
        commands::log::LogCommand::Site {
            domain: None,
            errors: errors_only,
            access: !errors_only,
            php: false,
            follow: false,
            n: 50,
            status: None,
            ip: None,
        },
        &create_cli(false, false),
    )
    .await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn view_nginx_logs_interactive() -> Result<()> {
    let log_types = vec!["Error log", "Access log"];

    let log_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Log type")
        .items(&log_types)
        .default(0)
        .interact()?;

    let errors = log_type == 0;

    let lines: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of lines")
        .default(50)
        .interact_text()?;

    let follow = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Follow log in real-time?")
        .default(false)
        .interact()?;

    commands::log::execute(
        commands::log::LogCommand::Nginx {
            errors,
            follow,
            n: lines,
        },
        &create_cli(false, false),
    )
    .await?;

    if !follow {
        press_enter_to_continue()?;
    }
    Ok(())
}

async fn view_php_logs_interactive() -> Result<()> {
    // Get installed PHP versions
    let installed = crate::config::php::get_installed_versions().await;

    if installed.is_empty() {
        println!("\n{} No PHP versions installed", "!".yellow());
        press_enter_to_continue()?;
        return Ok(());
    }

    let version_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("PHP version")
        .items(&installed)
        .default(0)
        .interact()?;

    let version = &installed[version_idx];

    let lines: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of lines")
        .default(50)
        .interact_text()?;

    let follow = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Follow log in real-time?")
        .default(false)
        .interact()?;

    commands::log::execute(
        commands::log::LogCommand::Php {
            version: Some(version.clone()),
            follow,
            n: lines,
        },
        &create_cli(false, false),
    )
    .await?;

    if !follow {
        press_enter_to_continue()?;
    }
    Ok(())
}

async fn view_mysql_logs_interactive() -> Result<()> {
    let lines: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of lines")
        .default(50)
        .interact_text()?;

    let follow = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Follow log in real-time?")
        .default(false)
        .interact()?;

    commands::log::execute(
        commands::log::LogCommand::Mysql { follow, n: lines },
        &create_cli(false, false),
    )
    .await?;

    if !follow {
        press_enter_to_continue()?;
    }
    Ok(())
}

async fn view_fail2ban_logs_interactive() -> Result<()> {
    let bans_only = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Show only ban/unban actions?")
        .default(true)
        .interact()?;

    let lines: usize = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of lines")
        .default(50)
        .interact_text()?;

    let follow = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Follow log in real-time?")
        .default(false)
        .interact()?;

    commands::log::execute(
        commands::log::LogCommand::Fail2ban {
            follow,
            n: lines,
            bans: bans_only,
        },
        &create_cli(false, false),
    )
    .await?;

    if !follow {
        press_enter_to_continue()?;
    }
    Ok(())
}

// ============================================================================
// Backup Menu
// ============================================================================

async fn backup_menu() -> Result<()> {
    loop {
        let items = vec![
            "Create backup",
            "Restore from backup",
            "List backups",
            "Delete backup",
            "Configure backups",
            "Show configuration",
            "← Back",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Backup Management")
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => create_backup_interactive().await?,
            1 => restore_backup_interactive().await?,
            2 => list_backups_interactive().await?,
            3 => delete_backup_interactive().await?,
            4 => configure_backup_interactive().await?,
            5 => show_backup_config_interactive().await?,
            _ => return Ok(()),
        }
    }
}

async fn create_backup_interactive() -> Result<()> {
    // Get list of sites
    let sites = crate::database::sites::list().await?;

    if sites.is_empty() {
        println!("\n{} No sites found", "!".yellow());
        press_enter_to_continue()?;
        return Ok(());
    }

    // Build site list for selection
    let site_items: Vec<String> = sites.iter().map(|s| s.domain.clone()).collect();

    let site_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select site to backup")
        .items(&site_items)
        .default(0)
        .interact()?;

    let domain = &sites[site_idx].domain;

    // Backup type
    let backup_types = vec![
        "Full backup (files + database)",
        "Database only",
        "Files only",
    ];

    let backup_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Backup type")
        .items(&backup_types)
        .default(0)
        .interact()?;

    let (db_only, files_only) = match backup_type {
        1 => (true, false),
        2 => (false, true),
        _ => (false, false),
    };

    // Optional backup name
    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Backup name (optional, press Enter to skip)")
        .allow_empty(true)
        .interact_text()?;

    let name = if name.is_empty() { None } else { Some(name) };

    // Build CLI arguments for privileged command
    let mut args = vec!["backup", "create", domain];
    let name_arg;
    if let Some(ref n) = name {
        name_arg = format!("--name={}", n);
        args.push(&name_arg);
    }
    if db_only {
        args.push("--db-only");
    }
    if files_only {
        args.push("--files-only");
    }

    run_privileged_command(&args).await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn restore_backup_interactive() -> Result<()> {
    // Get list of backups
    let backups = crate::database::backups::list(None).await?;

    if backups.is_empty() {
        println!("\n{} No backups found", "!".yellow());
        press_enter_to_continue()?;
        return Ok(());
    }

    // Build backup list for selection
    let backup_items: Vec<String> = backups
        .iter()
        .map(|b| {
            format!(
                "{} - {} ({})",
                b.id,
                b.domain,
                b.backup_name.as_deref().unwrap_or("unnamed")
            )
        })
        .collect();

    let backup_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select backup to restore")
        .items(&backup_items)
        .default(0)
        .interact()?;

    let backup_id = backups[backup_idx].id.to_string();

    // Restore to different domain?
    let restore_to_different = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Restore to a different domain?")
        .default(false)
        .interact()?;

    let target = if restore_to_different {
        let sites = crate::database::sites::list().await?;
        if sites.is_empty() {
            println!("\n{} No sites available as restore target", "!".yellow());
            press_enter_to_continue()?;
            return Ok(());
        }

        let site_items: Vec<String> = sites.iter().map(|s| s.domain.clone()).collect();

        let site_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select target site")
            .items(&site_items)
            .default(0)
            .interact()?;

        Some(sites[site_idx].domain.clone())
    } else {
        None
    };

    // Restore type
    let restore_types = vec![
        "Full restore (files + database)",
        "Database only",
        "Files only",
    ];

    let restore_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Restore type")
        .items(&restore_types)
        .default(0)
        .interact()?;

    let (db_only, files_only) = match restore_type {
        1 => (true, false),
        2 => (false, true),
        _ => (false, false),
    };

    // Confirm restore
    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("This will overwrite existing data. Continue?")
        .default(false)
        .interact()?;

    if !confirm {
        println!("{} Restore cancelled", "!".yellow());
        press_enter_to_continue()?;
        return Ok(());
    }

    // Build CLI arguments for privileged command
    let mut args = vec!["backup", "restore", &backup_id];
    let target_arg;
    if let Some(ref t) = target {
        target_arg = format!("--target={}", t);
        args.push(&target_arg);
    }
    if db_only {
        args.push("--db-only");
    }
    if files_only {
        args.push("--files-only");
    }
    args.push("-y");

    run_privileged_command(&args).await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn list_backups_interactive() -> Result<()> {
    let detailed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Show detailed information?")
        .default(false)
        .interact()?;

    commands::backup::execute(
        commands::backup::BackupCommand::List {
            domain: None,
            detailed,
        },
        &create_cli(false, false),
    )
    .await?;

    press_enter_to_continue()?;
    Ok(())
}

async fn delete_backup_interactive() -> Result<()> {
    let delete_types = vec!["Delete specific backup", "Delete old backups"];

    let delete_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Delete option")
        .items(&delete_types)
        .default(0)
        .interact()?;

    if delete_type == 0 {
        // Delete specific backup
        let backups = crate::database::backups::list(None).await?;

        if backups.is_empty() {
            println!("\n{} No backups found", "!".yellow());
            press_enter_to_continue()?;
            return Ok(());
        }

        let backup_items: Vec<String> = backups
            .iter()
            .map(|b| {
                format!(
                    "{} - {} ({})",
                    b.id,
                    b.domain,
                    b.backup_name.as_deref().unwrap_or("unnamed")
                )
            })
            .collect();

        let backup_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select backup to delete")
            .items(&backup_items)
            .default(0)
            .interact()?;

        let backup_id = backups[backup_idx].id.to_string();

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Delete this backup?")
            .default(false)
            .interact()?;

        if !confirm {
            println!("{} Cancelled", "!".yellow());
            press_enter_to_continue()?;
            return Ok(());
        }

        // Delete specific backup (requires root)
        run_privileged_command(&["backup", "delete", "--backup-id", &backup_id, "-y"]).await?;
    } else {
        // Delete old backups
        let days: u32 = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Delete backups older than (days)")
            .default(30)
            .interact_text()?;

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Delete all backups older than {} days?", days))
            .default(false)
            .interact()?;

        if !confirm {
            println!("{} Cancelled", "!".yellow());
            press_enter_to_continue()?;
            return Ok(());
        }

        // Delete old backups (requires root)
        let days_arg = format!("--older-than={}", days);
        run_privileged_command(&["backup", "delete", &days_arg, "-y"]).await?;
    }

    press_enter_to_continue()?;
    Ok(())
}

async fn configure_backup_interactive() -> Result<()> {
    println!("\n{} Backup Configuration\n", "→".bright_cyan().bold());

    // Backup directory
    let dir: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Backup directory (leave empty to keep current)")
        .allow_empty(true)
        .default("/var/backups/rustwops".to_string())
        .interact_text()?;

    // Retention days
    let retention: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Retention days (leave empty to keep current)")
        .allow_empty(true)
        .default("30".to_string())
        .interact_text()?;

    // Schedule
    let schedule_options = vec![
        "No change",
        "Daily at midnight (0 0 * * *)",
        "Daily at 3am (0 3 * * *)",
        "Weekly on Sunday at midnight (0 0 * * 0)",
        "Custom cron expression",
    ];

    let schedule_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Backup schedule")
        .items(&schedule_options)
        .default(0)
        .interact()?;

    let schedule = match schedule_idx {
        1 => Some("0 0 * * *".to_string()),
        2 => Some("0 3 * * *".to_string()),
        3 => Some("0 0 * * 0".to_string()),
        4 => {
            let cron: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter cron expression (e.g., '0 2 * * *')")
                .interact_text()?;
            Some(cron)
        }
        _ => None,
    };

    // Build CLI arguments for privileged command
    let mut args = vec!["backup", "config"];

    let dir_arg;
    if !dir.is_empty() {
        dir_arg = format!("--dir={}", dir);
        args.push(&dir_arg);
    }

    let retention_arg;
    if !retention.is_empty() {
        if let Ok(days) = retention.parse::<u32>() {
            retention_arg = format!("--retention={}", days);
            args.push(&retention_arg);
        }
    }

    let schedule_arg;
    if let Some(ref s) = schedule {
        schedule_arg = format!("--schedule={}", s);
        args.push(&schedule_arg);
    }

    if args.len() > 2 {
        run_privileged_command(&args).await?;
    } else {
        println!("{} No changes made", "→".yellow());
    }

    press_enter_to_continue()?;
    Ok(())
}

async fn show_backup_config_interactive() -> Result<()> {
    // Show backup config (read-only, no sudo needed)
    let cli = create_cli(false, false);
    commands::backup::execute(commands::backup::BackupCommand::ConfigShow, &cli).await?;

    press_enter_to_continue()?;
    Ok(())
}

// ============================================================================
// Service Menu
// ============================================================================

async fn service_menu() -> Result<()> {
    let items = vec![
        "Show status",
        "Start service",
        "Stop service",
        "Restart service",
        "Reload service",
        "Back",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Service Management")
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            // Status is read-only, no sudo needed
            let cli = create_cli(false, false);
            commands::service::execute(
                commands::service::ServiceCommand::Status { service: None },
                &cli,
            )
            .await?;
            press_enter_to_continue()?;
        }
        1..=4 => {
            // Build dynamic service list based on what's installed
            let mut services = vec![
                "nginx".to_string(),
                "mariadb".to_string(),
                "redis-server".to_string(),
            ];

            // Check for installed PHP versions
            let php_versions = ["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];
            for version in php_versions {
                let service = format!("php{}-fpm", version);
                // Check if the service exists by trying to get its status
                if crate::utils::shell::run_command(
                    "systemctl",
                    &["list-unit-files", &format!("{}.service", service)],
                )
                .await
                .map(|output| output.contains(&service))
                .unwrap_or(false)
                {
                    services.push(service);
                }
            }

            let svc_idx = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select service")
                .items(&services)
                .default(0)
                .interact()?;

            let service = &services[svc_idx];

            // Service start/stop/restart/reload requires root
            let action = match selection {
                1 => "start",
                2 => "stop",
                3 => "restart",
                _ => "reload",
            };

            run_privileged_command(&["service", action, service]).await?;
            press_enter_to_continue()?;
        }
        _ => {}
    }

    Ok(())
}

// ============================================================================
// Site Enable/Disable
// ============================================================================

async fn site_enable_disable(domain: &str, currently_enabled: bool) -> Result<()> {
    if currently_enabled {
        // Confirm disable
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Disable site {}?", domain))
            .default(false)
            .interact()?;

        if confirm {
            run_privileged_command(&["site", "disable", domain]).await?;
        }
    } else {
        // Enable without confirmation
        run_privileged_command(&["site", "enable", domain]).await?;
    }

    press_enter_to_continue()?;
    Ok(())
}

// Site Update Menu
// ============================================================================

async fn site_update_menu(site: &crate::database::sites::Site) -> Result<()> {
    let is_wordpress = site.site_type == "wp";

    let mut items = vec!["Change PHP version".to_string()];

    if is_wordpress {
        items.push("Change cache type".to_string());
    }

    items.push("Back".to_string());

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Update {}", site.domain))
        .items(&items)
        .default(0)
        .interact()?;

    if selection == 0 {
        // Change PHP version
        site_update_php(&site.domain, site.php_version.as_deref()).await?;
        return Ok(());
    }

    if is_wordpress && selection == 1 {
        // Change cache type
        site_update_cache(&site.domain, site.cache_type.as_deref()).await?;
        return Ok(());
    }

    // Back
    Ok(())
}

async fn site_update_php(domain: &str, current_version: Option<&str>) -> Result<()> {
    use crate::config::php;

    // Get installed PHP versions
    let versions = php::get_installed_versions().await;

    if versions.is_empty() {
        println!(
            "\n{} No PHP versions installed. Run 'rw stack install php' first.",
            "Error:".red().bold()
        );
        press_enter_to_continue()?;
        return Ok(());
    }

    // Build selection items with current version marked
    let items: Vec<String> = versions
        .iter()
        .map(|v| {
            if Some(v.as_str()) == current_version {
                format!("PHP {} (current)", v)
            } else {
                format!("PHP {}", v)
            }
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select PHP version")
        .items(&items)
        .default(0)
        .interact()?;

    let new_version = &versions[selection];

    if Some(new_version.as_str()) == current_version {
        println!("\n{} Already using PHP {}", "→".bright_cyan(), new_version);
        press_enter_to_continue()?;
        return Ok(());
    }

    let php_arg = format!("--php={}", new_version);
    run_privileged_command(&["site", "update", domain, &php_arg]).await?;
    press_enter_to_continue()?;
    Ok(())
}

async fn site_update_cache(domain: &str, current_cache: Option<&str>) -> Result<()> {
    use crate::commands::site::CacheType;

    let cache_options = [
        ("None", CacheType::None, "none"),
        ("FastCGI (page cache)", CacheType::Fastcgi, "fastcgi"),
        ("Redis (object cache)", CacheType::Redis, "redis"),
    ];

    // Build selection items with current cache marked
    let items: Vec<String> = cache_options
        .iter()
        .map(|(label, _, key)| {
            if Some(*key) == current_cache {
                format!("{} (current)", label)
            } else {
                label.to_string()
            }
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select cache type")
        .items(&items)
        .default(0)
        .interact()?;

    let (_, _new_cache, new_cache_str) = &cache_options[selection];

    if Some(*new_cache_str) == current_cache {
        println!(
            "\n{} Already using {} cache",
            "→".bright_cyan(),
            new_cache_str
        );
        press_enter_to_continue()?;
        return Ok(());
    }

    // Warn about cache changes
    println!(
        "\n{} Changing cache type will update nginx config and WordPress plugins.",
        "Note:".yellow().bold()
    );

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Continue?")
        .default(true)
        .interact()?;

    if !confirm {
        return Ok(());
    }

    let cache_arg = format!("--cache={}", new_cache_str);
    run_privileged_command(&["site", "update", domain, &cache_arg]).await?;
    press_enter_to_continue()?;
    Ok(())
}

// Info Menu
// ============================================================================

async fn info_menu() -> Result<()> {
    let cli = create_cli(true, false);
    commands::info::execute(&cli).await?;
    press_enter_to_continue()?;
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

fn create_cli(verbose: bool, yes: bool) -> Cli {
    Cli {
        command: crate::Commands::Info, // Dummy, not used
        verbose,
        yes,
        format: crate::OutputFormat::Text,
    }
}

fn press_enter_to_continue() -> Result<()> {
    println!();
    Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("Press Enter to continue")
        .allow_empty(true)
        .interact_text()?;
    // Clear screen effect
    println!("\n\n");
    Ok(())
}
