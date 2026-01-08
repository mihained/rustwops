use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

use crate::utils::shell;
use crate::Cli;

#[derive(Clone, Subcommand)]
pub enum ServiceCommand {
    /// Start a service
    Start {
        /// Service name
        service: String,
    },

    /// Stop a service
    Stop {
        /// Service name
        service: String,
    },

    /// Restart a service
    Restart {
        /// Service name
        service: String,
    },

    /// Reload a service
    Reload {
        /// Service name
        service: String,
    },

    /// Show service status
    Status {
        /// Service name (all if not specified)
        service: Option<String>,
    },

    /// View service logs
    Log {
        /// Service name
        service: String,

        /// Follow log output
        #[arg(long)]
        tail: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,
    },
}

pub async fn execute(command: ServiceCommand, _cli: &Cli) -> Result<()> {
    use crate::utils::system::require_root;

    match command {
        ServiceCommand::Start { service } => {
            require_root("start service")?;
            start_service(&service).await
        }
        ServiceCommand::Stop { service } => {
            require_root("stop service")?;
            stop_service(&service).await
        }
        ServiceCommand::Restart { service } => {
            require_root("restart service")?;
            restart_service(&service).await
        }
        ServiceCommand::Reload { service } => {
            require_root("reload service")?;
            reload_service(&service).await
        }
        // Read-only commands - no root required
        ServiceCommand::Status { service } => show_status(service.as_deref()).await,
        ServiceCommand::Log { service, tail, n } => show_logs(&service, tail, n).await,
    }
}

async fn start_service(service: &str) -> Result<()> {
    let service = normalize_service_name(service);
    println!("{} Starting {}...", "→".bright_cyan(), service);

    shell::run_command("systemctl", &["start", &service]).await?;

    println!("{} {} started", "✓".green(), service);
    Ok(())
}

async fn stop_service(service: &str) -> Result<()> {
    let service = normalize_service_name(service);
    println!("{} Stopping {}...", "→".bright_cyan(), service);

    shell::run_command("systemctl", &["stop", &service]).await?;

    println!("{} {} stopped", "✓".green(), service);
    Ok(())
}

async fn restart_service(service: &str) -> Result<()> {
    let service = normalize_service_name(service);
    println!("{} Restarting {}...", "→".bright_cyan(), service);

    shell::run_command("systemctl", &["restart", &service]).await?;

    println!("{} {} restarted", "✓".green(), service);
    Ok(())
}

async fn reload_service(service: &str) -> Result<()> {
    let service = normalize_service_name(service);
    println!("{} Reloading {}...", "→".bright_cyan(), service);

    shell::run_command("systemctl", &["reload", &service]).await?;

    println!("{} {} reloaded", "✓".green(), service);
    Ok(())
}

async fn show_status(service: Option<&str>) -> Result<()> {
    if let Some(service) = service {
        let service = normalize_service_name(service);
        let output = shell::run_command("systemctl", &["status", &service]).await?;
        println!("{}", output);
    } else {
        // Show status of all managed services
        let services = [
            "nginx",
            "mariadb",
            "mysql",
            "redis-server",
            "php7.4-fpm",
            "php8.0-fpm",
            "php8.1-fpm",
            "php8.2-fpm",
            "php8.3-fpm",
            "php8.4-fpm",
        ];

        println!("{} Service Status:\n", "→".bright_cyan().bold());

        for service in services {
            let status = shell::run_command("systemctl", &["is-active", service])
                .await
                .unwrap_or_else(|_| "not-found".to_string());

            let status_display = match status.trim() {
                "active" => "● running".green().to_string(),
                "inactive" => "○ stopped".yellow().to_string(),
                _ => "✗ not installed".dimmed().to_string(),
            };

            if status.trim() != "not-found" || service == "nginx" || service == "redis-server" {
                println!("  {:20} {}", service, status_display);
            }
        }

        println!();
    }

    Ok(())
}

async fn show_logs(service: &str, tail: bool, n: usize) -> Result<()> {
    let service = normalize_service_name(service);
    let n_str = n.to_string();

    let mut args = vec!["-u", &service, "-n", &n_str];
    if tail {
        args.push("-f");
    }

    let output = shell::run_command("journalctl", &args).await?;
    println!("{}", output);

    Ok(())
}

fn normalize_service_name(service: &str) -> String {
    // Allow shortcuts like "php8.3" -> "php8.3-fpm"
    if service.starts_with("php") && !service.ends_with("-fpm") {
        format!("{}-fpm", service)
    } else if service == "mysql" || service == "mariadb" {
        // Check which is installed
        "mariadb".to_string()
    } else if service == "redis" {
        "redis-server".to_string()
    } else {
        service.to_string()
    }
}
