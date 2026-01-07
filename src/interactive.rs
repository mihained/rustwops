use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};

use crate::commands;
use crate::Cli;

pub async fn run() -> Result<()> {
    print_banner();

    // Initialize database if needed
    crate::database::ensure_initialized().await?;

    loop {
        let selection = main_menu()?;

        match selection {
            MainMenu::Stack => stack_menu().await?,
            MainMenu::Site => site_menu().await?,
            MainMenu::Security => security_menu().await?,
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
    Security,
    Service,
    Info,
    Exit,
}

fn main_menu() -> Result<MainMenu> {
    let items = vec![
        "Stack      Manage server components (Nginx, PHP, MySQL, Redis)",
        "Sites      Create and manage websites",
        "Security   Fail2Ban, ClamAV, MySQLTuner tools",
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
        2 => MainMenu::Security,
        3 => MainMenu::Service,
        4 => MainMenu::Info,
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

    // Build component list
    let mut comp_list = Vec::new();
    for &idx in &selections {
        match idx {
            0 => comp_list.push(commands::stack::Component::Nginx),
            1 => comp_list.push(commands::stack::Component::Php),
            2 => comp_list.push(commands::stack::Component::Mysql),
            3 => comp_list.push(commands::stack::Component::Redis),
            4 => comp_list.push(commands::stack::Component::Nodejs),
            6 => comp_list.push(commands::stack::Component::Fail2ban),
            7 => comp_list.push(commands::stack::Component::Clamav),
            8 => comp_list.push(commands::stack::Component::Mysqltuner),
            _ => {}
        }
    }

    // Create a mock CLI for verbose=false, yes=true
    let cli = create_cli(false, true);

    commands::stack::install::execute(
        false, // not --all
        comp_list,
        &php_version,
        db_type,
        &node_version,
        false, // not custom nginx
        &cli,
    )
    .await?;

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

    let mut comp_list = Vec::new();
    for &idx in &selections {
        match idx {
            0 => comp_list.push(commands::stack::Component::Nginx),
            1 => comp_list.push(commands::stack::Component::Php),
            2 => comp_list.push(commands::stack::Component::Mysql),
            3 => comp_list.push(commands::stack::Component::Redis),
            4 => comp_list.push(commands::stack::Component::Nodejs),
            _ => {}
        }
    }

    let cli = create_cli(false, true);
    commands::stack::remove::execute(comp_list, purge, &cli).await?;

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
            let cli = create_cli(false, true);
            commands::stack::update::execute(vec![], &cli).await?;
        }
        1 => {
            let components = vec!["Nginx", "PHP", "MySQL/MariaDB", "Redis", "Node.js"];
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Select components to update")
                .items(&components)
                .interact()?;

            let mut comp_list = Vec::new();
            for &idx in &selections {
                match idx {
                    0 => comp_list.push(commands::stack::Component::Nginx),
                    1 => comp_list.push(commands::stack::Component::Php),
                    2 => comp_list.push(commands::stack::Component::Mysql),
                    3 => comp_list.push(commands::stack::Component::Redis),
                    4 => comp_list.push(commands::stack::Component::Nodejs),
                    _ => {}
                }
            }

            let cli = create_cli(false, true);
            commands::stack::update::execute(comp_list, &cli).await?;
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

            let cli = create_cli(false, true);
            commands::stack::install::install_php_version(versions[idx], &cli).await?;
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
                commands::security::execute(
                    commands::security::SecurityCommand::Status,
                    &create_cli(false, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            1 => {
                commands::security::execute(
                    commands::security::SecurityCommand::Mysqltuner,
                    &create_cli(true, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            2 => {
                clamav_scan_menu().await?;
            }
            3 => {
                commands::security::execute(
                    commands::security::SecurityCommand::UpdateDefinitions,
                    &create_cli(true, false),
                )
                .await?;
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

    commands::security::execute(
        commands::security::SecurityCommand::Scan {
            path: Some(path),
            quarantine,
        },
        &create_cli(true, false),
    )
    .await?;

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

                commands::security::execute(
                    commands::security::SecurityCommand::Fail2ban {
                        action: commands::security::Fail2banAction::Unban { ip, jail: None },
                    },
                    &create_cli(false, false),
                )
                .await?;
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

                commands::security::execute(
                    commands::security::SecurityCommand::Fail2ban {
                        action: commands::security::Fail2banAction::Ban {
                            ip,
                            jail: jails[jail_idx].to_string(),
                        },
                    },
                    &create_cli(false, false),
                )
                .await?;
                press_enter_to_continue()?;
            }
            4 => {
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

        // Staging option
        if has_staging {
            items.push("Staging environment".to_string());
        } else {
            items.push("Create staging".to_string());
        }

        // WordPress-specific actions
        if is_wordpress {
            items.push("Reset admin password".to_string());
            items.push("WP-CLI commands".to_string());
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

    let cli = create_cli(false, true);
    commands::staging::execute(
        commands::staging::StagingCommand::Create {
            domain: site.domain.clone(),
            prefix,
        },
        &cli,
    )
    .await?;

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

                    let provider = match idx {
                        0 => commands::ssl::DnsProvider::Cloudflare,
                        1 => commands::ssl::DnsProvider::Digitalocean,
                        _ => commands::ssl::DnsProvider::Route53,
                    };

                    commands::ssl::issue::execute_dns(
                        domain,
                        provider,
                        commands::ssl::KeyType::default(),
                        false, // staging
                        true,  // verbose
                    )
                    .await?;
                } else {
                    commands::ssl::issue::execute_http(
                        domain,
                        commands::ssl::KeyType::default(),
                        false, // staging
                        true,  // verbose
                    )
                    .await?;
                }
                press_enter_to_continue()?;
            }
            1 => {
                // TODO: Show SSL status when implemented
                println!("\n{} SSL status check not yet implemented.\n", "→".yellow());
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
            // Sync prod -> staging
            let cli = create_cli(false, true);
            commands::staging::execute(
                commands::staging::StagingCommand::Sync {
                    domain: site.domain.clone(),
                    direction: commands::staging::SyncDirection::ProdToStage,
                    files_only: false,
                    db_only: false,
                    exclude_tables: None,
                    dry_run: false,
                },
                &cli,
            )
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
                let cli = create_cli(false, true);
                commands::staging::execute(
                    commands::staging::StagingCommand::Sync {
                        domain: site.domain.clone(),
                        direction: commands::staging::SyncDirection::StageToProd,
                        files_only: false,
                        db_only: false,
                        exclude_tables: None,
                        dry_run: false,
                    },
                    &cli,
                )
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
                let cli = create_cli(false, true);
                commands::staging::execute(
                    commands::staging::StagingCommand::Delete {
                        domain: site.domain.clone(),
                    },
                    &cli,
                )
                .await?;
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

    let cli = create_cli(false, true);
    commands::site::delete::execute(domain, all, files, db, &cli).await?;
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

    let cli = create_cli(false, true);
    commands::site::create::execute(
        &domain,
        site_type,
        &php_version,
        type_idx <= 1, // mysql for wp/php
        cache,
        ssl,
        wildcard,
        dns_provider,
        upstream,
        &cli,
    )
    .await?;

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

            let service = services[svc_idx].to_string();
            let cli = create_cli(false, true);

            let cmd = match selection {
                1 => commands::service::ServiceCommand::Start { service },
                2 => commands::service::ServiceCommand::Stop { service },
                3 => commands::service::ServiceCommand::Restart { service },
                _ => commands::service::ServiceCommand::Reload { service },
            };

            commands::service::execute(cmd, &cli).await?;
            press_enter_to_continue()?;
        }
        _ => {}
    }

    Ok(())
}

// ============================================================================
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
