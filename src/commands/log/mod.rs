use anyhow::{anyhow, Result};
use clap::Subcommand;
use colored::Colorize;

use crate::database;
use crate::utils::shell;
use crate::Cli;

#[derive(Clone, Subcommand)]
pub enum LogCommand {
    /// View site logs (nginx access/error, php-fpm)
    Site {
        /// Domain name (omit for all sites)
        domain: Option<String>,

        /// Show only error logs
        #[arg(long, short)]
        errors: bool,

        /// Show only access logs
        #[arg(long, short)]
        access: bool,

        /// Show PHP-FPM logs
        #[arg(long)]
        php: bool,

        /// Follow log output in real-time
        #[arg(long, short)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,

        /// Filter by HTTP status code (e.g., 404, 500)
        #[arg(long)]
        status: Option<String>,

        /// Filter by IP address
        #[arg(long)]
        ip: Option<String>,
    },

    /// View Fail2Ban logs
    Fail2ban {
        /// Follow log output in real-time
        #[arg(long, short)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,

        /// Show only ban actions
        #[arg(long)]
        bans: bool,
    },

    /// View Nginx logs (global)
    Nginx {
        /// Show only error logs
        #[arg(long, short)]
        errors: bool,

        /// Follow log output in real-time
        #[arg(long, short)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,
    },

    /// View MySQL/MariaDB logs
    Mysql {
        /// Follow log output in real-time
        #[arg(long, short)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,
    },

    /// View PHP-FPM logs
    Php {
        /// PHP version (e.g., 8.3)
        version: Option<String>,

        /// Follow log output in real-time
        #[arg(long, short)]
        follow: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,
    },
}

pub async fn execute(command: LogCommand, _cli: &Cli) -> Result<()> {
    match command {
        LogCommand::Site {
            domain,
            errors,
            access,
            php,
            follow,
            n,
            status,
            ip,
        } => view_site_logs(domain, errors, access, php, follow, n, status, ip).await,
        LogCommand::Fail2ban { follow, n, bans } => view_fail2ban_logs(follow, n, bans).await,
        LogCommand::Nginx { errors, follow, n } => view_nginx_logs(errors, follow, n).await,
        LogCommand::Mysql { follow, n } => view_mysql_logs(follow, n).await,
        LogCommand::Php { version, follow, n } => view_php_logs(version, follow, n).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn view_site_logs(
    domain: Option<String>,
    errors_only: bool,
    access_only: bool,
    php: bool,
    follow: bool,
    lines: usize,
    status_filter: Option<String>,
    ip_filter: Option<String>,
) -> Result<()> {
    // Determine which log types to show
    let show_errors = errors_only || (!access_only && !php);
    let show_access = access_only || (!errors_only && !php);
    let show_php = php;

    if let Some(domain) = domain {
        // Single site logs
        println!(
            "{} Logs for: {}\n",
            "→".bright_cyan().bold(),
            domain.bright_white()
        );

        if show_access {
            let access_log = format!("/var/log/nginx/{}.access.log", domain);
            if std::path::Path::new(&access_log).exists() {
                println!("{}", "═══ Access Log ═══".bright_green().bold());
                view_log_file(
                    &access_log,
                    follow,
                    lines,
                    status_filter.as_deref(),
                    ip_filter.as_deref(),
                )
                .await?;
                println!();
            } else {
                println!(
                    "{} Access log not found: {}",
                    "!".yellow(),
                    access_log.dimmed()
                );
            }
        }

        if show_errors {
            let error_log = format!("/var/log/nginx/{}.error.log", domain);
            if std::path::Path::new(&error_log).exists() {
                println!("{}", "═══ Error Log ═══".bright_red().bold());
                view_log_file(&error_log, follow, lines, None, None).await?;
                println!();
            } else {
                println!(
                    "{} Error log not found: {}",
                    "!".yellow(),
                    error_log.dimmed()
                );
            }
        }

        if show_php {
            // Try to find the PHP version for this site
            if let Ok(Some(site)) = database::sites::get_by_domain(&domain).await {
                if let Some(ref php_ver) = site.php_version {
                    let php_log = format!("/var/log/php{}-fpm.log", php_ver);
                    if std::path::Path::new(&php_log).exists() {
                        println!(
                            "{}",
                            format!("═══ PHP-FPM {} Log ═══", php_ver)
                                .bright_magenta()
                                .bold()
                        );
                        // Filter PHP logs by pool name (domain with underscores)
                        let pool_name = domain.replace(['.', '-'], "_");
                        view_php_log_filtered(&php_log, &pool_name, follow, lines).await?;
                    }
                }
            }
        }
    } else {
        // All sites logs
        println!("{} All Sites Logs\n", "→".bright_cyan().bold());

        // Get all sites from database
        let sites = database::sites::list().await.unwrap_or_default();

        if sites.is_empty() {
            println!("{} No sites found", "!".yellow());
            return Ok(());
        }

        for site in sites {
            println!(
                "\n{} {}",
                "━━━".bright_cyan(),
                site.domain.bright_white().bold()
            );

            if show_errors {
                let error_log = format!("/var/log/nginx/{}.error.log", site.domain);
                if std::path::Path::new(&error_log).exists() {
                    // Show last few error lines
                    let output = shell::run_command("tail", &["-n", "10", &error_log]).await;
                    if let Ok(output) = output {
                        if !output.trim().is_empty() {
                            println!("  {}", "Errors:".red());
                            for line in output.lines().take(5) {
                                println!("    {}", line.dimmed());
                            }
                        }
                    }
                }
            }

            if show_access {
                let access_log = format!("/var/log/nginx/{}.access.log", site.domain);
                if std::path::Path::new(&access_log).exists() {
                    // Show request count
                    let count = shell::run_command("wc", &["-l", &access_log])
                        .await
                        .unwrap_or_default();
                    let count = count.split_whitespace().next().unwrap_or("0");
                    println!("  {} Total requests: {}", "Access:".green(), count);
                }
            }
        }
    }

    Ok(())
}

async fn view_log_file(
    path: &str,
    follow: bool,
    lines: usize,
    status_filter: Option<&str>,
    ip_filter: Option<&str>,
) -> Result<()> {
    let lines_str = lines.to_string();

    // Build the command based on filters
    // Nginx combined log format: IP - - [date] "METHOD /path HTTP/1.1" STATUS SIZE "referer" "ua"
    // Status code pattern: after closing quote and space, e.g., `" 502 `
    if follow {
        if status_filter.is_some() || ip_filter.is_some() {
            // Use tail -f piped to grep for filtering
            let mut grep_pattern = String::new();
            if let Some(status) = status_filter {
                grep_pattern = format!("\\\" {} ", status);
            }
            if let Some(ip) = ip_filter {
                if !grep_pattern.is_empty() {
                    grep_pattern = format!("{}|{}", grep_pattern, ip);
                } else {
                    grep_pattern = ip.to_string();
                }
            }

            let cmd = format!("tail -f {} | grep -E '{}'", path, grep_pattern);
            println!("{}", "(Press Ctrl+C to stop)".dimmed());
            shell::run_command_interactive("bash", &["-c", &cmd]).await?;
        } else {
            println!("{}", "(Press Ctrl+C to stop)".dimmed());
            shell::run_command_interactive("tail", &["-f", "-n", &lines_str, path]).await?;
        }
    } else {
        let output = if status_filter.is_some() || ip_filter.is_some() {
            let mut grep_pattern = String::new();
            if let Some(status) = status_filter {
                grep_pattern = format!("\\\" {} ", status);
            }
            if let Some(ip) = ip_filter {
                if !grep_pattern.is_empty() {
                    grep_pattern = format!("{}.*{}|{}.*{}", grep_pattern, ip, ip, grep_pattern);
                } else {
                    grep_pattern = ip.to_string();
                }
            }

            let cmd = format!(
                "grep -E '{}' {} | tail -n {}",
                grep_pattern, path, lines_str
            );
            shell::run_command("bash", &["-c", &cmd]).await?
        } else {
            shell::run_command("tail", &["-n", &lines_str, path]).await?
        };

        if output.trim().is_empty() {
            println!("{}", "(no matching entries)".dimmed());
        } else {
            // Colorize log output
            for line in output.lines() {
                println!("{}", colorize_log_line(line));
            }
        }
    }

    Ok(())
}

async fn view_php_log_filtered(
    path: &str,
    pool_name: &str,
    follow: bool,
    lines: usize,
) -> Result<()> {
    let lines_str = lines.to_string();

    if follow {
        let cmd = format!("tail -f {} | grep -i '{}'", path, pool_name);
        println!("{}", "(Press Ctrl+C to stop)".dimmed());
        shell::run_command_interactive("bash", &["-c", &cmd]).await?;
    } else {
        let cmd = format!("grep -i '{}' {} | tail -n {}", pool_name, path, lines_str);
        let output = shell::run_command("bash", &["-c", &cmd])
            .await
            .unwrap_or_default();

        if output.trim().is_empty() {
            println!("{}", "(no entries for this site)".dimmed());
        } else {
            for line in output.lines() {
                println!("{}", colorize_log_line(line));
            }
        }
    }

    Ok(())
}

async fn view_fail2ban_logs(follow: bool, lines: usize, bans_only: bool) -> Result<()> {
    println!("{} Fail2Ban Logs\n", "→".bright_cyan().bold());

    let log_path = "/var/log/fail2ban.log";

    if !std::path::Path::new(log_path).exists() {
        return Err(anyhow!("Fail2Ban log not found. Is Fail2Ban installed?"));
    }

    let lines_str = lines.to_string();

    if follow {
        if bans_only {
            let cmd = format!("tail -f {} | grep -E '(Ban|Unban)'", log_path);
            println!("{}", "(Press Ctrl+C to stop)".dimmed());
            shell::run_command_interactive("bash", &["-c", &cmd]).await?;
        } else {
            println!("{}", "(Press Ctrl+C to stop)".dimmed());
            shell::run_command_interactive("tail", &["-f", "-n", &lines_str, log_path]).await?;
        }
    } else {
        let output = if bans_only {
            let cmd = format!("grep -E '(Ban|Unban)' {} | tail -n {}", log_path, lines_str);
            shell::run_command("bash", &["-c", &cmd]).await?
        } else {
            shell::run_command("tail", &["-n", &lines_str, log_path]).await?
        };

        if output.trim().is_empty() {
            println!("{}", "(no entries)".dimmed());
        } else {
            for line in output.lines() {
                println!("{}", colorize_fail2ban_line(line));
            }
        }
    }

    Ok(())
}

async fn view_nginx_logs(errors_only: bool, follow: bool, lines: usize) -> Result<()> {
    let log_path = if errors_only {
        println!("{} Nginx Error Log\n", "→".bright_cyan().bold());
        "/var/log/nginx/error.log"
    } else {
        println!("{} Nginx Access Log\n", "→".bright_cyan().bold());
        "/var/log/nginx/access.log"
    };

    if !std::path::Path::new(log_path).exists() {
        return Err(anyhow!("Nginx log not found: {}", log_path));
    }

    view_log_file(log_path, follow, lines, None, None).await
}

async fn view_mysql_logs(follow: bool, lines: usize) -> Result<()> {
    println!("{} MySQL/MariaDB Log\n", "→".bright_cyan().bold());

    // Try file-based logs first
    let log_paths = [
        "/var/log/mysql/error.log",
        "/var/log/mariadb/mariadb.log",
        "/var/log/mysql.log",
    ];

    if let Some(log_path) = log_paths.iter().find(|p| std::path::Path::new(p).exists()) {
        return view_log_file(log_path, follow, lines, None, None).await;
    }

    // Fall back to journalctl (MariaDB on Ubuntu 24.04 logs to systemd journal)
    let lines_str = lines.to_string();

    if follow {
        println!("{}", "(Press Ctrl+C to stop)".dimmed());
        shell::run_command_interactive("journalctl", &["-u", "mariadb", "-f", "-n", &lines_str])
            .await?;
    } else {
        let output = shell::run_command(
            "journalctl",
            &["-u", "mariadb", "--no-pager", "-n", &lines_str],
        )
        .await?;

        if output.trim().is_empty() {
            println!("{}", "(no entries)".dimmed());
        } else {
            for line in output.lines() {
                println!("{}", colorize_log_line(line));
            }
        }
    }

    Ok(())
}

async fn view_php_logs(version: Option<String>, follow: bool, lines: usize) -> Result<()> {
    let version = version.unwrap_or_else(|| "8.3".to_string());

    println!(
        "{} PHP-FPM {} Log\n",
        "→".bright_cyan().bold(),
        version.bright_white()
    );

    let log_path = format!("/var/log/php{}-fpm.log", version);

    if !std::path::Path::new(&log_path).exists() {
        return Err(anyhow!("PHP-FPM log not found: {}", log_path));
    }

    view_log_file(&log_path, follow, lines, None, None).await
}

fn colorize_log_line(line: &str) -> String {
    // Colorize based on log level/content
    if line.contains("error") || line.contains("ERROR") || line.contains("[error]") {
        line.red().to_string()
    } else if line.contains("warn") || line.contains("WARN") || line.contains("[warn]") {
        line.yellow().to_string()
    } else if line.contains(" 5") && (line.contains("\" 5") || line.contains("HTTP/1.1\" 5")) {
        // 5xx errors in access log
        line.red().to_string()
    } else if line.contains(" 4") && (line.contains("\" 4") || line.contains("HTTP/1.1\" 4")) {
        // 4xx errors in access log
        line.yellow().to_string()
    } else if line.contains("\" 2") || line.contains("HTTP/1.1\" 2") {
        // 2xx success
        line.to_string()
    } else {
        line.to_string()
    }
}

fn colorize_fail2ban_line(line: &str) -> String {
    if line.contains("Ban") && !line.contains("Unban") {
        line.red().to_string()
    } else if line.contains("Unban") {
        line.green().to_string()
    } else if line.contains("Found") {
        line.yellow().to_string()
    } else {
        line.to_string()
    }
}
