use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

use crate::utils::shell;
use crate::Cli;

#[derive(Clone, Subcommand)]
pub enum SecurityCommand {
    /// Run MySQLTuner database analysis
    Mysqltuner,

    /// ClamAV antivirus operations
    Scan {
        /// Directory to scan (default: /var/www)
        #[arg(short, long)]
        path: Option<String>,

        /// Move infected files to quarantine
        #[arg(long)]
        quarantine: bool,
    },

    /// Update ClamAV virus definitions
    UpdateDefinitions,

    /// Fail2Ban management
    Fail2ban {
        #[command(subcommand)]
        action: Fail2banAction,
    },

    /// Show security tools status
    Status,
}

#[derive(Clone, Subcommand)]
pub enum Fail2banAction {
    /// Show Fail2Ban status
    Status,

    /// Show banned IPs
    Banned,

    /// Unban an IP address
    Unban {
        /// IP address to unban
        ip: String,

        /// Jail name (optional, unbans from all jails if not specified)
        #[arg(short, long)]
        jail: Option<String>,
    },

    /// Ban an IP address
    Ban {
        /// IP address to ban
        ip: String,

        /// Jail name
        #[arg(short, long, default_value = "sshd")]
        jail: String,
    },

    /// Show recent Fail2Ban logs
    Logs {
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: u32,
    },
}

pub async fn execute(command: SecurityCommand, cli: &Cli) -> Result<()> {
    use crate::utils::system::require_root;

    match command {
        SecurityCommand::Mysqltuner => {
            require_root("run MySQLTuner")?;
            run_mysqltuner(cli.verbose).await
        }
        SecurityCommand::Scan { path, quarantine } => {
            require_root("run security scan")?;
            run_clamav_scan(path.as_deref(), quarantine, cli.verbose).await
        }
        SecurityCommand::UpdateDefinitions => {
            require_root("update ClamAV definitions")?;
            update_clamav_definitions(cli.verbose).await
        }
        SecurityCommand::Fail2ban { action } => execute_fail2ban(action, cli.verbose).await,
        // Read-only command - no root required
        SecurityCommand::Status => show_security_status(cli.verbose).await,
    }
}

// =============================================================================
// MySQLTuner
// =============================================================================

async fn run_mysqltuner(verbose: bool) -> Result<()> {
    println!(
        "{} Running MySQLTuner database analysis...\n",
        "→".bright_cyan().bold()
    );

    // Check if mysqltuner is installed
    if !std::path::Path::new("/usr/local/bin/mysqltuner").exists() {
        println!("{} MySQLTuner is not installed.", "✗".red().bold());
        println!("  Install it with: rw stack install mysqltuner\n");
        return Ok(());
    }

    // Run mysqltuner
    let output = shell::run_shell_script("/usr/local/bin/mysqltuner", verbose).await?;
    println!("{}", output);

    Ok(())
}

// =============================================================================
// ClamAV
// =============================================================================

async fn run_clamav_scan(path: Option<&str>, quarantine: bool, verbose: bool) -> Result<()> {
    let scan_path = path.unwrap_or("/var/www");

    println!(
        "{} Running ClamAV scan on {}...\n",
        "→".bright_cyan().bold(),
        scan_path.bright_white()
    );

    // Check if clamav is installed
    if shell::run_command("which", &["clamscan"]).await.is_err() {
        println!("{} ClamAV is not installed.", "✗".red().bold());
        println!("  Install it with: rw stack install clamav\n");
        return Ok(());
    }

    let mut args = vec!["--infected".to_string(), "--recursive".to_string()];

    if quarantine {
        // Ensure quarantine directory exists
        shell::run_command("mkdir", &["-p", "/var/lib/rustwops/quarantine"]).await?;
        args.push("--move=/var/lib/rustwops/quarantine".to_string());
        println!(
            "  {} Infected files will be moved to quarantine\n",
            "→".dimmed()
        );
    }

    args.push(scan_path.to_string());

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = shell::run_command_with_output("clamscan", &args_refs, verbose).await?;

    // Parse and display results
    println!("\n{}", output);

    // Check for infections in the output
    if output.contains("Infected files: 0") {
        println!("\n{} No threats found!\n", "✓".green().bold());
    } else if output.contains("Infected files:") {
        println!(
            "\n{} Threats detected! Review the output above.\n",
            "⚠".yellow().bold()
        );
        if quarantine {
            println!("  Infected files have been moved to: /var/lib/rustwops/quarantine\n");
        }
    }

    Ok(())
}

async fn update_clamav_definitions(verbose: bool) -> Result<()> {
    println!(
        "{} Updating ClamAV virus definitions...\n",
        "→".bright_cyan().bold()
    );

    // Check if freshclam is installed
    if shell::run_command("which", &["freshclam"]).await.is_err() {
        println!("{} ClamAV is not installed.", "✗".red().bold());
        println!("  Install it with: rw stack install clamav\n");
        return Ok(());
    }

    // Stop freshclam service temporarily
    println!("  {} Stopping freshclam service...", "→".dimmed());
    shell::run_command("systemctl", &["stop", "clamav-freshclam"])
        .await
        .ok();

    // Run freshclam
    println!("  {} Downloading latest definitions...", "→".dimmed());
    let result = shell::run_command_with_output("freshclam", &[], verbose).await;

    // Restart freshclam service
    println!("  {} Restarting freshclam service...", "→".dimmed());
    shell::run_command("systemctl", &["start", "clamav-freshclam"])
        .await
        .ok();

    match result {
        Ok(output) => {
            println!("\n{}", output);
            println!(
                "\n{} Virus definitions updated successfully!\n",
                "✓".green().bold()
            );
        }
        Err(e) => {
            println!(
                "\n{} Failed to update definitions: {}\n",
                "✗".red().bold(),
                e
            );
        }
    }

    Ok(())
}

// =============================================================================
// Fail2Ban
// =============================================================================

async fn execute_fail2ban(action: Fail2banAction, _verbose: bool) -> Result<()> {
    use crate::utils::system::require_root;

    // Check if fail2ban is installed
    if shell::run_command("which", &["fail2ban-client"])
        .await
        .is_err()
    {
        println!("{} Fail2Ban is not installed.", "✗".red().bold());
        println!("  Install it with: rw stack install fail2ban\n");
        return Ok(());
    }

    // Check root requirement for modifying operations
    match &action {
        Fail2banAction::Ban { .. } => require_root("ban IP address")?,
        Fail2banAction::Unban { .. } => require_root("unban IP address")?,
        // Status, Banned, Logs are read-only
        _ => {}
    }

    match action {
        Fail2banAction::Status => {
            println!("{} Fail2Ban Status\n", "→".bright_cyan().bold());

            let output = shell::run_command("fail2ban-client", &["status"]).await?;
            println!("{}\n", output);

            // Get status for each jail
            let jails_line = output.lines().find(|l| l.contains("Jail list:"));
            if let Some(line) = jails_line {
                let jails: Vec<&str> = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();

                for jail in jails {
                    println!("{} Jail: {}", "─".dimmed(), jail.bright_white());
                    if let Ok(status) =
                        shell::run_command("fail2ban-client", &["status", jail]).await
                    {
                        // Extract just the key info
                        for line in status.lines() {
                            if line.contains("Currently banned:") || line.contains("Total banned:")
                            {
                                println!("  {}", line.trim());
                            }
                        }
                    }
                }
                println!();
            }
        }

        Fail2banAction::Banned => {
            println!("{} Currently Banned IPs\n", "→".bright_cyan().bold());

            let output = shell::run_command("fail2ban-client", &["status"]).await?;
            let jails_line = output.lines().find(|l| l.contains("Jail list:"));

            if let Some(line) = jails_line {
                let jails: Vec<&str> = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("")
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();

                let mut any_banned = false;

                for jail in jails {
                    if let Ok(status) =
                        shell::run_command("fail2ban-client", &["status", jail]).await
                    {
                        for line in status.lines() {
                            if line.contains("Banned IP list:") {
                                let ips = line.split(':').nth(1).unwrap_or("").trim();
                                if !ips.is_empty() {
                                    any_banned = true;
                                    println!("{} {}:", "→".bright_cyan(), jail.bright_white());
                                    for ip in ips.split_whitespace() {
                                        println!("    {}", ip.red());
                                    }
                                }
                            }
                        }
                    }
                }

                if !any_banned {
                    println!("{} No IPs are currently banned.\n", "✓".green());
                } else {
                    println!();
                }
            }
        }

        Fail2banAction::Unban { ip, jail } => {
            println!(
                "{} Unbanning IP: {}\n",
                "→".bright_cyan().bold(),
                ip.bright_white()
            );

            if let Some(jail_name) = jail {
                // Unban from specific jail
                shell::run_command("fail2ban-client", &["set", &jail_name, "unbanip", &ip]).await?;
                println!(
                    "{} Unbanned {} from jail {}\n",
                    "✓".green().bold(),
                    ip,
                    jail_name
                );
            } else {
                // Unban from all jails
                let output = shell::run_command("fail2ban-client", &["status"]).await?;
                let jails_line = output.lines().find(|l| l.contains("Jail list:"));

                if let Some(line) = jails_line {
                    let jails: Vec<&str> = line
                        .split(':')
                        .nth(1)
                        .unwrap_or("")
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();

                    for jail_name in jails {
                        if shell::run_command(
                            "fail2ban-client",
                            &["set", jail_name, "unbanip", &ip],
                        )
                        .await
                        .is_ok()
                        {
                            println!("  {} Unbanned from {}", "✓".green(), jail_name);
                        }
                    }
                }
                println!();
            }
        }

        Fail2banAction::Ban { ip, jail } => {
            println!(
                "{} Banning IP: {} in jail: {}\n",
                "→".bright_cyan().bold(),
                ip.bright_white(),
                jail.bright_white()
            );

            shell::run_command("fail2ban-client", &["set", &jail, "banip", &ip]).await?;
            println!("{} Banned {} in jail {}\n", "✓".green().bold(), ip, jail);
        }

        Fail2banAction::Logs { lines } => {
            println!(
                "{} Recent Fail2Ban logs (last {} lines)\n",
                "→".bright_cyan().bold(),
                lines
            );

            let output =
                shell::run_command("tail", &["-n", &lines.to_string(), "/var/log/fail2ban.log"])
                    .await?;

            println!("{}\n", output);
        }
    }

    Ok(())
}

// =============================================================================
// Security Status
// =============================================================================

async fn show_security_status(_verbose: bool) -> Result<()> {
    println!("{} Security Tools Status\n", "→".bright_cyan().bold());

    // Fail2Ban
    print!("  {} Fail2Ban: ", "→".dimmed());
    if shell::run_command("systemctl", &["is-active", "fail2ban"])
        .await
        .is_ok()
    {
        let output = shell::run_command("fail2ban-client", &["status"])
            .await
            .unwrap_or_default();
        let jail_count = output
            .lines()
            .find(|l| l.contains("Number of jail:"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim())
            .unwrap_or("?");
        println!("{} ({} jails active)", "Running".green(), jail_count);
    } else if std::path::Path::new("/usr/bin/fail2ban-client").exists() {
        println!("{}", "Stopped".yellow());
    } else {
        println!("{}", "Not installed".dimmed());
    }

    // ClamAV
    print!("  {} ClamAV:   ", "→".dimmed());
    if shell::run_command("systemctl", &["is-active", "clamav-freshclam"])
        .await
        .is_ok()
    {
        // Get version
        let version = shell::run_command("clamscan", &["--version"])
            .await
            .map(|v| v.lines().next().unwrap_or("").to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        println!("{} ({})", "Running".green(), version.dimmed());
    } else if std::path::Path::new("/usr/bin/clamscan").exists() {
        println!("{}", "Stopped".yellow());
    } else {
        println!("{}", "Not installed".dimmed());
    }

    // MySQLTuner
    print!("  {} MySQLTuner: ", "→".dimmed());
    if std::path::Path::new("/usr/local/bin/mysqltuner").exists() {
        println!("{}", "Installed".green());
    } else {
        println!("{}", "Not installed".dimmed());
    }

    // Quarantine info
    let quarantine_path = std::path::Path::new("/var/lib/rustwops/quarantine");
    if quarantine_path.exists() {
        let count = std::fs::read_dir(quarantine_path)
            .map(|entries| entries.count())
            .unwrap_or(0);
        if count > 0 {
            println!("\n  {} {} files in quarantine", "⚠".yellow(), count);
        }
    }

    println!();
    Ok(())
}
