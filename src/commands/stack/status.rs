use anyhow::Result;
use colored::Colorize;
use tabled::{Table, Tabled};

use crate::utils::shell;
use crate::Cli;

#[derive(Tabled)]
struct ServiceStatus {
    #[tabled(rename = "Service")]
    service: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Version")]
    version: String,
}

pub async fn execute(cli: &Cli) -> Result<()> {
    println!("{} Stack Status\n", "→".bright_cyan().bold());

    let mut services = Vec::new();

    // Check Nginx
    services.push(check_service("Nginx", "nginx", get_nginx_version).await);

    // Check PHP versions
    for version in &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4"] {
        let service_name = format!("php{}-fpm", version);
        if is_service_installed(&service_name).await {
            services.push(check_service(&format!("PHP {}", version), &service_name, || {
                Box::pin(async move { get_php_version(version).await })
            }).await);
        }
    }

    // Check MariaDB/MySQL
    if is_service_installed("mariadb").await {
        services.push(check_service("MariaDB", "mariadb", get_mariadb_version).await);
    } else if is_service_installed("mysql").await {
        services.push(check_service("MySQL", "mysql", get_mysql_version).await);
    }

    // Check Redis
    services.push(check_service("Redis", "redis-server", get_redis_version).await);

    // Check PM2
    services.push(check_pm2().await);

    let table = Table::new(&services).to_string();
    println!("{}", table);

    if cli.verbose {
        println!("\n{} Detailed Information:", "→".bright_cyan());
        print_detailed_info().await?;
    }

    Ok(())
}

async fn check_service<F, Fut>(name: &str, service: &str, get_version: F) -> ServiceStatus
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    let status = get_service_status(service).await;
    let version = if status.contains("running") {
        get_version().await.unwrap_or_else(|_| "unknown".to_string())
    } else {
        "-".to_string()
    };

    ServiceStatus {
        service: name.to_string(),
        status,
        version,
    }
}

async fn get_service_status(service: &str) -> String {
    match shell::run_command("systemctl", &["is-active", service]).await {
        Ok(output) if output.trim() == "active" => "● running".green().to_string(),
        Ok(_) => "○ stopped".yellow().to_string(),
        Err(_) => "✗ not installed".dimmed().to_string(),
    }
}

async fn is_service_installed(service: &str) -> bool {
    shell::run_command("systemctl", &["list-unit-files", &format!("{}.service", service)])
        .await
        .map(|o| o.contains(service))
        .unwrap_or(false)
}

async fn get_nginx_version() -> Result<String> {
    let output = shell::run_command("nginx", &["-v"]).await?;
    Ok(output
        .lines()
        .next()
        .unwrap_or("")
        .replace("nginx version: nginx/", "")
        .trim()
        .to_string())
}

async fn get_php_version(version: &str) -> Result<String> {
    let binary = format!("php{}", version);
    let output = shell::run_command(&binary, &["-v"]).await?;
    Ok(output
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("unknown")
        .to_string())
}

async fn get_mariadb_version() -> Result<String> {
    let output = shell::run_command("mariadb", &["--version"]).await?;
    Ok(output
        .split_whitespace()
        .find(|s| s.starts_with("10.") || s.starts_with("11."))
        .unwrap_or("unknown")
        .to_string())
}

async fn get_mysql_version() -> Result<String> {
    let output = shell::run_command("mysql", &["--version"]).await?;
    Ok(output
        .split_whitespace()
        .find(|s| s.starts_with("8.") || s.starts_with("5."))
        .unwrap_or("unknown")
        .to_string())
}

async fn get_redis_version() -> Result<String> {
    let output = shell::run_command("redis-server", &["--version"]).await?;
    Ok(output
        .split_whitespace()
        .find(|s| s.starts_with("v="))
        .map(|s| s.replace("v=", ""))
        .unwrap_or_else(|| "unknown".to_string()))
}

async fn check_pm2() -> ServiceStatus {
    let pm2_check = shell::run_shell_script(
        r#"
        export HOME=/root
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
        pm2 --version 2>/dev/null
        "#,
        false,
    )
    .await;

    match pm2_check {
        Ok(version) => ServiceStatus {
            service: "PM2".to_string(),
            status: "● running".green().to_string(),
            version: version.trim().to_string(),
        },
        Err(_) => ServiceStatus {
            service: "PM2".to_string(),
            status: "✗ not installed".dimmed().to_string(),
            version: "-".to_string(),
        },
    }
}

async fn print_detailed_info() -> Result<()> {
    // Print disk usage
    println!("\n  {} Disk Usage:", "•".dimmed());
    if let Ok(output) = shell::run_command("df", &["-h", "/var/www"]).await {
        for line in output.lines().skip(1) {
            println!("    {}", line);
        }
    }

    // Print memory usage
    println!("\n  {} Memory Usage:", "•".dimmed());
    if let Ok(output) = shell::run_command("free", &["-h"]).await {
        for line in output.lines() {
            println!("    {}", line);
        }
    }

    Ok(())
}
