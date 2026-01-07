use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use super::Component;
use crate::utils::shell;
use crate::Cli;

pub async fn execute(components: Vec<Component>, purge: bool, cli: &Cli) -> Result<()> {
    println!(
        "{} Removing stack components...\n",
        "→".bright_cyan().bold()
    );

    if !cli.yes {
        let confirm = Confirm::new()
            .with_prompt("Are you sure you want to remove these components?")
            .default(false)
            .interact()?;

        if !confirm {
            println!("{}", "Aborted.".yellow());
            return Ok(());
        }
    }

    for component in &components {
        match component {
            Component::Nginx => remove_nginx(purge, cli.verbose).await?,
            Component::Php => remove_php(purge, cli.verbose).await?,
            Component::Mysql => remove_database(purge, cli.verbose).await?,
            Component::Redis => remove_redis(purge, cli.verbose).await?,
            Component::Nodejs => remove_nodejs(purge, cli.verbose).await?,
            Component::Fail2ban => remove_security_tool("fail2ban", "fail2ban", purge, cli.verbose).await?,
            Component::Mysqltuner => remove_mysqltuner(cli.verbose).await?,
            Component::Clamav => remove_security_tool("clamav-daemon", "clamav clamav-daemon clamav-freshclam", purge, cli.verbose).await?,
        }
    }

    println!(
        "\n{} Components removed successfully!\n",
        "✓".green().bold()
    );

    Ok(())
}

async fn remove_nginx(purge: bool, verbose: bool) -> Result<()> {
    println!("  {} Removing Nginx...", "→".dimmed());

    shell::run_command("systemctl", &["stop", "nginx"]).await.ok();
    shell::run_command("systemctl", &["disable", "nginx"]).await.ok();

    let cmd = if purge { "purge" } else { "remove" };
    shell::run_command_with_output("apt-get", &[cmd, "-y", "-qq", "nginx"], verbose).await?;

    if purge {
        shell::run_command("rm", &["-rf", "/etc/nginx"]).await.ok();
    }

    println!("  {} Nginx removed", "✓".green());
    Ok(())
}

async fn remove_php(purge: bool, verbose: bool) -> Result<()> {
    println!("  {} Removing PHP...", "→".dimmed());

    // Stop all PHP-FPM services
    for version in &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4"] {
        let service = format!("php{}-fpm", version);
        shell::run_command("systemctl", &["stop", &service]).await.ok();
        shell::run_command("systemctl", &["disable", &service]).await.ok();
    }

    let cmd = if purge { "purge" } else { "remove" };
    shell::run_command_with_output("apt-get", &[cmd, "-y", "-qq", "php*"], verbose).await?;

    if purge {
        shell::run_command("rm", &["-rf", "/etc/php"]).await.ok();
    }

    println!("  {} PHP removed", "✓".green());
    Ok(())
}

async fn remove_database(purge: bool, verbose: bool) -> Result<()> {
    println!("  {} Removing database server...", "→".dimmed());

    // Try MariaDB first
    shell::run_command("systemctl", &["stop", "mariadb"]).await.ok();
    shell::run_command("systemctl", &["disable", "mariadb"]).await.ok();

    // Then MySQL
    shell::run_command("systemctl", &["stop", "mysql"]).await.ok();
    shell::run_command("systemctl", &["disable", "mysql"]).await.ok();

    let cmd = if purge { "purge" } else { "remove" };
    shell::run_command_with_output(
        "apt-get",
        &[cmd, "-y", "-qq", "mariadb-server", "mariadb-client", "mysql-server", "mysql-client"],
        verbose,
    )
    .await
    .ok();

    if purge {
        shell::run_command("rm", &["-rf", "/var/lib/mysql"]).await.ok();
        shell::run_command("rm", &["-rf", "/etc/mysql"]).await.ok();
    }

    println!("  {} Database server removed", "✓".green());
    Ok(())
}

async fn remove_redis(purge: bool, verbose: bool) -> Result<()> {
    println!("  {} Removing Redis...", "→".dimmed());

    shell::run_command("systemctl", &["stop", "redis-server"]).await.ok();
    shell::run_command("systemctl", &["disable", "redis-server"]).await.ok();

    let cmd = if purge { "purge" } else { "remove" };
    shell::run_command_with_output("apt-get", &[cmd, "-y", "-qq", "redis-server"], verbose).await?;

    if purge {
        shell::run_command("rm", &["-rf", "/var/lib/redis"]).await.ok();
        shell::run_command("rm", &["-rf", "/etc/redis"]).await.ok();
    }

    println!("  {} Redis removed", "✓".green());
    Ok(())
}

async fn remove_nodejs(purge: bool, verbose: bool) -> Result<()> {
    println!("  {} Removing Node.js and PM2...", "→".dimmed());

    // Remove PM2
    let pm2_remove = r#"
        export HOME=/root
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
        pm2 kill 2>/dev/null || true
        npm uninstall -g pm2 2>/dev/null || true
    "#;
    shell::run_shell_script(pm2_remove, verbose).await.ok();

    if purge {
        // Remove nvm entirely
        shell::run_command("rm", &["-rf", "/root/.nvm"]).await.ok();
        shell::run_command("rm", &["-rf", "/root/.pm2"]).await.ok();
    }

    println!("  {} Node.js and PM2 removed", "✓".green());
    Ok(())
}

async fn remove_security_tool(service: &str, packages: &str, purge: bool, verbose: bool) -> Result<()> {
    let name = packages.split_whitespace().next().unwrap_or(packages);
    println!("  {} Removing {}...", "→".dimmed(), name);

    // Stop and disable service
    shell::run_command("systemctl", &["stop", service]).await.ok();
    shell::run_command("systemctl", &["disable", service]).await.ok();

    let cmd = if purge { "purge" } else { "remove" };
    let mut args = vec![cmd, "-y", "-qq"];
    args.extend(packages.split_whitespace());

    shell::run_command_with_output("apt-get", &args, verbose).await.ok();

    println!("  {} {} removed", "✓".green(), name);
    Ok(())
}

async fn remove_mysqltuner(_verbose: bool) -> Result<()> {
    println!("  {} Removing MySQLTuner...", "→".dimmed());

    shell::run_command("rm", &["-f", "/usr/local/bin/mysqltuner"]).await.ok();

    println!("  {} MySQLTuner removed", "✓".green());
    Ok(())
}
