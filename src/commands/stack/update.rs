use anyhow::Result;
use colored::Colorize;

use super::Component;
use crate::utils::shell;
use crate::Cli;

pub async fn execute(components: Vec<Component>, cli: &Cli) -> Result<()> {
    println!(
        "{} Updating stack components...\n",
        "→".bright_cyan().bold()
    );

    // Update apt cache first
    println!("  {} Updating apt cache...", "→".dimmed());
    shell::run_command_with_output("apt-get", &["update", "-qq"], cli.verbose).await?;

    if components.is_empty() {
        // Update all installed components
        update_all(cli.verbose).await?;
    } else {
        for component in &components {
            match component {
                Component::Nginx => update_nginx(cli.verbose).await?,
                Component::Php => update_php(cli.verbose).await?,
                Component::Mysql => update_database(cli.verbose).await?,
                Component::Redis => update_redis(cli.verbose).await?,
                Component::Nodejs => update_nodejs(cli.verbose).await?,
                Component::Fail2ban => update_security_tool("fail2ban", cli.verbose).await?,
                Component::Mysqltuner => update_mysqltuner(cli.verbose).await?,
                Component::Clamav => update_security_tool("clamav clamav-daemon clamav-freshclam", cli.verbose).await?,
            }
        }
    }

    println!(
        "\n{} Stack update complete!\n",
        "✓".green().bold()
    );

    Ok(())
}

async fn update_all(verbose: bool) -> Result<()> {
    println!("  {} Upgrading all packages...", "→".dimmed());

    shell::run_command_with_output(
        "apt-get",
        &["upgrade", "-y", "-qq"],
        verbose,
    )
    .await?;

    // Also update auxiliary tools
    update_auxiliary_tools(verbose).await?;

    println!("  {} All packages upgraded", "✓".green());
    Ok(())
}

async fn update_nginx(verbose: bool) -> Result<()> {
    println!("  {} Updating Nginx...", "→".dimmed());

    shell::run_command_with_output(
        "apt-get",
        &["install", "--only-upgrade", "-y", "-qq", "nginx"],
        verbose,
    )
    .await?;

    shell::run_command("systemctl", &["reload", "nginx"]).await?;

    println!("  {} Nginx updated", "✓".green());
    Ok(())
}

async fn update_php(verbose: bool) -> Result<()> {
    println!("  {} Updating PHP...", "→".dimmed());

    // Update all installed PHP versions
    for version in &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4"] {
        let fpm = format!("php{}-fpm", version);
        if shell::run_command("dpkg", &["-l", &fpm]).await.is_ok() {
            shell::run_command_with_output(
                "apt-get",
                &["install", "--only-upgrade", "-y", "-qq", &format!("php{}*", version)],
                verbose,
            )
            .await?;

            shell::run_command("systemctl", &["reload", &fpm]).await.ok();
        }
    }

    println!("  {} PHP updated", "✓".green());
    Ok(())
}

async fn update_database(verbose: bool) -> Result<()> {
    println!("  {} Updating database server...", "→".dimmed());

    // Try MariaDB first
    if shell::run_command("dpkg", &["-l", "mariadb-server"]).await.is_ok() {
        shell::run_command_with_output(
            "apt-get",
            &["install", "--only-upgrade", "-y", "-qq", "mariadb-server", "mariadb-client"],
            verbose,
        )
        .await?;
    }

    // Then MySQL
    if shell::run_command("dpkg", &["-l", "mysql-server"]).await.is_ok() {
        shell::run_command_with_output(
            "apt-get",
            &["install", "--only-upgrade", "-y", "-qq", "mysql-server", "mysql-client"],
            verbose,
        )
        .await?;
    }

    println!("  {} Database server updated", "✓".green());
    Ok(())
}

async fn update_redis(verbose: bool) -> Result<()> {
    println!("  {} Updating Redis...", "→".dimmed());

    shell::run_command_with_output(
        "apt-get",
        &["install", "--only-upgrade", "-y", "-qq", "redis-server"],
        verbose,
    )
    .await?;

    shell::run_command("systemctl", &["reload", "redis-server"]).await.ok();

    println!("  {} Redis updated", "✓".green());
    Ok(())
}

async fn update_nodejs(verbose: bool) -> Result<()> {
    println!("  {} Updating Node.js and PM2...", "→".dimmed());

    // Update nvm and Node.js
    let nvm_update = r#"
        export HOME=/root
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"

        # Update nvm itself
        cd "$NVM_DIR" && git fetch --tags origin && git checkout `git describe --abbrev=0 --tags --match "v[0-9]*" $(git rev-list --tags --max-count=1)` 2>/dev/null || true

        # Update current Node.js to latest LTS
        nvm install --lts --reinstall-packages-from=current

        # Update PM2
        npm install -g pm2@latest
        pm2 update
    "#;

    shell::run_shell_script(nvm_update, verbose).await?;

    println!("  {} Node.js and PM2 updated", "✓".green());
    Ok(())
}

async fn update_security_tool(packages: &str, verbose: bool) -> Result<()> {
    println!("  {} Updating {}...", "→".dimmed(), packages.split_whitespace().next().unwrap_or(packages));

    let args: Vec<&str> = std::iter::once("install")
        .chain(std::iter::once("--only-upgrade"))
        .chain(std::iter::once("-y"))
        .chain(std::iter::once("-qq"))
        .chain(packages.split_whitespace())
        .collect();

    shell::run_command_with_output("apt-get", &args, verbose).await?;

    println!("  {} {} updated", "✓".green(), packages.split_whitespace().next().unwrap_or(packages));
    Ok(())
}

async fn update_mysqltuner(verbose: bool) -> Result<()> {
    println!("  {} Updating MySQLTuner...", "→".dimmed());

    shell::run_command_with_output(
        "curl",
        &[
            "-sL",
            "https://raw.githubusercontent.com/major/MySQLTuner-perl/master/mysqltuner.pl",
            "-o", "/usr/local/bin/mysqltuner",
        ],
        verbose,
    ).await?;

    println!("  {} MySQLTuner updated", "✓".green());
    Ok(())
}

async fn update_auxiliary_tools(verbose: bool) -> Result<()> {
    println!("  {} Updating auxiliary tools...", "→".dimmed());

    // Update acme.sh
    let acme_update = r#"
        export HOME=/root
        ~/.acme.sh/acme.sh --upgrade
    "#;
    shell::run_shell_script(acme_update, verbose).await.ok();

    // Update WP-CLI
    shell::run_command("wp", &["cli", "update", "--yes"]).await.ok();

    println!("  {} Auxiliary tools updated", "✓".green());
    Ok(())
}
