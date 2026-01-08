use anyhow::{anyhow, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Stdio;

use crate::database;
use crate::Cli;

use super::Pm2Action;

pub async fn execute(domain: &str, action: Pm2Action, _cli: &Cli) -> Result<()> {
    // Get site info
    let site = database::sites::get_by_domain(domain)
        .await?
        .ok_or_else(|| anyhow!("Site not found: {}", domain))?;

    // Verify it's a Node.js site
    if site.site_type != "node" {
        return Err(anyhow!(
            "Site '{}' is not a Node.js site (type: {})",
            domain,
            site.site_type
        ));
    }

    // Check ecosystem file exists
    // The ecosystem file is in the parent of webroot (prod/ not prod/public/)
    let webroot_path = std::path::Path::new(&site.webroot);
    let app_dir = webroot_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| site.webroot.clone());
    let ecosystem_file = format!("{}/ecosystem.config.js", app_dir);

    if !Path::new(&ecosystem_file).exists() {
        return Err(anyhow!(
            "PM2 ecosystem file not found: {}\n\
             Make sure the site was created with 'rw site create --type node'",
            ecosystem_file
        ));
    }

    // Get the app name (same as domain - matches ecosystem.config.js)
    let app_name = domain;

    match action {
        Pm2Action::Start => start_app(&app_dir, &ecosystem_file, app_name).await,
        Pm2Action::Stop => stop_app(app_name).await,
        Pm2Action::Restart => restart_app(app_name).await,
        Pm2Action::Logs => show_logs(app_name).await,
        Pm2Action::Status => show_status(app_name).await,
    }
}

async fn start_app(app_dir: &str, ecosystem_file: &str, app_name: &str) -> Result<()> {
    println!(
        "{} Starting PM2 app: {}\n",
        "→".bright_cyan().bold(),
        app_name.bright_white()
    );

    // Check if already running
    let status = tokio::process::Command::new("pm2")
        .args(["show", app_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;

    if status.success() {
        println!(
            "  {} App '{}' is already running. Use 'restart' to reload.",
            "!".yellow(),
            app_name
        );
        return Ok(());
    }

    // Start with ecosystem file
    let output = tokio::process::Command::new("pm2")
        .args(["start", ecosystem_file])
        .current_dir(app_dir)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to start PM2 app: {}", stderr));
    }

    // Save PM2 process list
    tokio::process::Command::new("pm2")
        .args(["save"])
        .output()
        .await?;

    println!(
        "{} App '{}' started successfully!",
        "✓".green().bold(),
        app_name
    );
    println!(
        "\n  {} View logs: rw site pm2 {} logs",
        "→".cyan(),
        app_name
    );

    Ok(())
}

async fn stop_app(app_name: &str) -> Result<()> {
    println!(
        "{} Stopping PM2 app: {}\n",
        "→".bright_cyan().bold(),
        app_name.bright_white()
    );

    let output = tokio::process::Command::new("pm2")
        .args(["stop", app_name])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") || stderr.contains("doesn't exist") {
            println!("  {} App '{}' is not running", "!".yellow(), app_name);
            return Ok(());
        }
        return Err(anyhow!("Failed to stop PM2 app: {}", stderr));
    }

    // Save PM2 process list
    tokio::process::Command::new("pm2")
        .args(["save"])
        .output()
        .await?;

    println!("{} App '{}' stopped", "✓".green().bold(), app_name);

    Ok(())
}

async fn restart_app(app_name: &str) -> Result<()> {
    println!(
        "{} Restarting PM2 app: {}\n",
        "→".bright_cyan().bold(),
        app_name.bright_white()
    );

    let output = tokio::process::Command::new("pm2")
        .args(["restart", app_name])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") || stderr.contains("doesn't exist") {
            println!(
                "  {} App '{}' is not running. Starting it instead...",
                "!".yellow(),
                app_name
            );
            // Try to start it
            return Err(anyhow!(
                "App not running. Use 'rw site pm2 {} start' to start it.",
                app_name
            ));
        }
        return Err(anyhow!("Failed to restart PM2 app: {}", stderr));
    }

    // Save PM2 process list
    tokio::process::Command::new("pm2")
        .args(["save"])
        .output()
        .await?;

    println!("{} App '{}' restarted", "✓".green().bold(), app_name);

    Ok(())
}

async fn show_logs(app_name: &str) -> Result<()> {
    println!(
        "{} Showing logs for: {} (Ctrl+C to exit)\n",
        "→".bright_cyan().bold(),
        app_name.bright_white()
    );

    // Run pm2 logs interactively
    let status = tokio::process::Command::new("pm2")
        .args(["logs", app_name, "--lines", "50"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow!(
            "Failed to show logs. Is the app '{}' running?",
            app_name
        ));
    }

    Ok(())
}

async fn show_status(app_name: &str) -> Result<()> {
    println!(
        "{} PM2 Status for: {}\n",
        "→".bright_cyan().bold(),
        app_name.bright_white()
    );

    // Get detailed status
    let output = tokio::process::Command::new("pm2")
        .args(["show", app_name])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") || stderr.contains("doesn't exist") {
            println!("  {} App '{}' is not running", "!".yellow(), app_name);
            println!(
                "\n  {} Start it with: rw site pm2 {} start",
                "→".cyan(),
                app_name
            );
            return Ok(());
        }
        return Err(anyhow!("Failed to get PM2 status: {}", stderr));
    }

    // Print the output
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{}", stdout);

    Ok(())
}
