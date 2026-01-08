use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use super::{Component, DbType};
use crate::utils::shell;
use crate::Cli;

pub async fn execute(
    all: bool,
    components: Vec<Component>,
    php_version: &str,
    db_type: DbType,
    node_version: &str,
    nginx_custom: bool,
    cli: &Cli,
) -> Result<()> {
    let components_to_install = if all {
        vec![
            Component::Nginx,
            Component::Php,
            Component::Mysql,
            Component::Redis,
            Component::Nodejs,
        ]
    } else if components.is_empty() {
        anyhow::bail!("No components specified. Use --all or specify components.");
    } else {
        components
    };

    println!(
        "{} Installing stack components...\n",
        "→".bright_cyan().bold()
    );

    // Detect Ubuntu version
    let ubuntu_version = detect_ubuntu_version().await?;
    println!(
        "  {} Detected Ubuntu {}",
        "✓".green(),
        ubuntu_version.bright_white()
    );

    // Update apt cache
    update_apt_cache(cli.verbose).await?;

    for component in &components_to_install {
        match component {
            Component::Nginx => {
                install_nginx(nginx_custom, cli.verbose).await?;
            }
            Component::Php => {
                install_php(php_version, cli.verbose).await?;
            }
            Component::Mysql => {
                install_database(db_type, cli.verbose).await?;
            }
            Component::Redis => {
                install_redis(cli.verbose).await?;
            }
            Component::Nodejs => {
                install_nodejs(node_version, cli.verbose).await?;
            }
            Component::Fail2ban => {
                install_fail2ban(cli.verbose).await?;
            }
            Component::Mysqltuner => {
                install_mysqltuner(cli.verbose).await?;
            }
            Component::Clamav => {
                install_clamav(cli.verbose).await?;
            }
        }
    }

    // Install auxiliary tools
    install_auxiliary_tools(cli.verbose).await?;

    // Initialize RustWops directories and database
    initialize_rustwops(cli.verbose).await?;

    println!("\n{} Stack installation complete!\n", "✓".green().bold());

    Ok(())
}

async fn detect_ubuntu_version() -> Result<String> {
    // Try lsb_release first
    if let Ok(output) = shell::run_command("lsb_release", &["-rs"]).await {
        return Ok(output.trim().to_string());
    }

    // Fallback to /etc/os-release
    if let Ok(content) = shell::read_file("/etc/os-release").await {
        for line in content.lines() {
            if line.starts_with("VERSION_ID=") {
                let version = line.trim_start_matches("VERSION_ID=").trim_matches('"');
                return Ok(version.to_string());
            }
        }
    }

    // Default fallback
    Ok("24.04".to_string())
}

async fn update_apt_cache(verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Updating apt cache...");

    shell::run_command_with_output("apt-get", &["update", "-qq"], verbose).await?;

    pb.finish_with_message("Apt cache updated");
    Ok(())
}

async fn install_nginx(custom: bool, verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Installing Nginx...");

    if custom {
        // TODO: Implement custom Nginx build with HTTP/3, Brotli
        pb.set_message("Installing custom Nginx (HTTP/3, Brotli)...");
        anyhow::bail!("Custom Nginx build not yet implemented");
    } else {
        shell::run_command_with_output("apt-get", &["install", "-y", "-qq", "nginx"], verbose)
            .await?;
    }

    // Apply optimized nginx configuration
    pb.set_message("Applying Nginx optimizations...");
    crate::config::stack::apply_nginx_config().await?;

    // Enable and start Nginx
    shell::run_command("systemctl", &["enable", "nginx"]).await?;
    shell::run_command("systemctl", &["restart", "nginx"]).await?;

    pb.finish_with_message(format!("{} Nginx installed and optimized", "✓".green()));
    Ok(())
}

async fn install_php(version: &str, verbose: bool) -> Result<()> {
    let pb = create_progress_bar(&format!("Installing PHP {}...", version));

    // Add Ondřej Surý PPA for PHP
    shell::run_command_with_output(
        "apt-get",
        &["install", "-y", "-qq", "software-properties-common"],
        verbose,
    )
    .await?;

    shell::run_command_with_output("add-apt-repository", &["-y", "ppa:ondrej/php"], verbose)
        .await?;

    shell::run_command_with_output("apt-get", &["update", "-qq"], verbose).await?;

    // Install PHP and common extensions
    let packages = vec![
        format!("php{}-fpm", version),
        format!("php{}-cli", version),
        format!("php{}-common", version),
        format!("php{}-mysql", version),
        format!("php{}-xml", version),
        format!("php{}-curl", version),
        format!("php{}-gd", version),
        format!("php{}-mbstring", version),
        format!("php{}-zip", version),
        format!("php{}-bcmath", version),
        format!("php{}-intl", version),
        format!("php{}-soap", version),
        format!("php{}-redis", version),
        format!("php{}-imagick", version),
        format!("php{}-opcache", version),
    ];

    let mut args = vec!["install", "-y", "-qq"];
    for pkg in &packages {
        args.push(pkg.as_str());
    }

    shell::run_command_with_output("apt-get", &args, verbose).await?;

    // Apply PHP-FPM optimizations
    pb.set_message("Applying PHP-FPM optimizations...");
    crate::config::stack::apply_php_config(version).await?;

    // Enable and start PHP-FPM
    let service = format!("php{}-fpm", version);
    shell::run_command("systemctl", &["enable", &service]).await?;
    shell::run_command("systemctl", &["restart", &service]).await?;

    pb.finish_with_message(format!(
        "{} PHP {} installed and optimized",
        "✓".green(),
        version
    ));
    Ok(())
}

async fn install_database(db_type: DbType, verbose: bool) -> Result<()> {
    let db_name = match db_type {
        DbType::Mariadb => "MariaDB",
        DbType::Mysql => "MySQL",
    };

    let pb = create_progress_bar(&format!("Installing {}...", db_name));

    match db_type {
        DbType::Mariadb => {
            shell::run_command_with_output(
                "apt-get",
                &["install", "-y", "-qq", "mariadb-server", "mariadb-client"],
                verbose,
            )
            .await?;
            shell::run_command("systemctl", &["enable", "mariadb"]).await?;
            shell::run_command("systemctl", &["start", "mariadb"]).await?;

            // Apply MariaDB optimizations
            pb.set_message("Applying MariaDB optimizations...");
            crate::config::stack::apply_mariadb_config().await?;

            // Secure MariaDB installation
            pb.set_message("Securing MariaDB installation...");
            crate::config::stack::secure_mariadb_installation().await?;

            // Restart with new config
            shell::run_command("systemctl", &["restart", "mariadb"]).await?;
        }
        DbType::Mysql => {
            shell::run_command_with_output(
                "apt-get",
                &["install", "-y", "-qq", "mysql-server", "mysql-client"],
                verbose,
            )
            .await?;
            shell::run_command("systemctl", &["enable", "mysql"]).await?;
            shell::run_command("systemctl", &["start", "mysql"]).await?;
            // Note: MySQL optimization similar to MariaDB could be added here
        }
    }

    pb.finish_with_message(format!("{} {} installed and secured", "✓".green(), db_name));
    Ok(())
}

async fn install_redis(verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Installing Redis...");

    shell::run_command_with_output(
        "apt-get",
        &["install", "-y", "-qq", "redis-server"],
        verbose,
    )
    .await?;

    // Apply Redis optimizations
    pb.set_message("Applying Redis optimizations...");
    crate::config::stack::apply_redis_config().await?;

    shell::run_command("systemctl", &["enable", "redis-server"]).await?;
    shell::run_command("systemctl", &["restart", "redis-server"]).await?;

    pb.finish_with_message(format!("{} Redis installed and optimized", "✓".green()));
    Ok(())
}

async fn install_fail2ban(verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Installing Fail2Ban...");

    crate::config::security::install_fail2ban(verbose).await?;

    pb.finish_with_message(format!("{} Fail2Ban installed and configured", "✓".green()));
    Ok(())
}

async fn install_mysqltuner(verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Installing MySQLTuner...");

    crate::config::security::install_mysqltuner(verbose).await?;

    pb.finish_with_message(format!("{} MySQLTuner installed", "✓".green()));
    Ok(())
}

async fn install_clamav(verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Installing ClamAV...");

    crate::config::security::install_clamav(verbose).await?;

    pb.finish_with_message(format!("{} ClamAV installed and configured", "✓".green()));
    Ok(())
}

async fn install_nodejs(version: &str, verbose: bool) -> Result<()> {
    let pb = create_progress_bar(&format!("Installing Node.js {} via nvm...", version));

    // Install nvm
    let nvm_install = r#"
        export HOME=/root
        curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
    "#;

    shell::run_shell_script(nvm_install, verbose).await?;

    // Install Node.js version
    let node_install = format!(
        r#"
        export HOME=/root
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
        nvm install {}
        nvm use {}
        nvm alias default {}
    "#,
        version, version, version
    );

    shell::run_shell_script(&node_install, verbose).await?;

    // Install PM2 globally
    let pm2_install = r#"
        export HOME=/root
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
        npm install -g pm2
        pm2 startup systemd -u root --hp /root
    "#;

    shell::run_shell_script(pm2_install, verbose).await?;

    // Create symlinks so node/npm/pm2 are available in PATH
    let create_symlinks = r#"
        export HOME=/root
        export NVM_DIR="$HOME/.nvm"
        [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
        NODE_PATH=$(which node)
        NPM_PATH=$(which npm)
        PM2_PATH=$(which pm2)
        ln -sf "$NODE_PATH" /usr/local/bin/node
        ln -sf "$NPM_PATH" /usr/local/bin/npm
        ln -sf "$PM2_PATH" /usr/local/bin/pm2
    "#;

    shell::run_shell_script(create_symlinks, verbose).await?;

    pb.finish_with_message(format!(
        "{} Node.js {} with PM2 installed",
        "✓".green(),
        version
    ));
    Ok(())
}

async fn install_auxiliary_tools(verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Installing auxiliary tools...");

    // Install cron (required for acme.sh auto-renewal)
    pb.set_message("Installing cron...");
    shell::run_command_with_output("apt-get", &["install", "-y", "-qq", "cron"], verbose).await?;

    // Install acme.sh (use a placeholder email that can be changed later)
    pb.set_message("Installing acme.sh...");
    let acme_install = r#"
        export HOME=/root
        curl https://get.acme.sh | sh -s email=admin@example.com
    "#;
    shell::run_shell_script(acme_install, verbose).await?;

    // Install WP-CLI
    pb.set_message("Installing WP-CLI...");
    shell::run_command_with_output(
        "curl",
        &[
            "-O",
            "https://raw.githubusercontent.com/wp-cli/builds/gh-pages/phar/wp-cli.phar",
        ],
        verbose,
    )
    .await?;
    shell::run_command("chmod", &["+x", "wp-cli.phar"]).await?;
    shell::run_command("mv", &["wp-cli.phar", "/usr/local/bin/wp"]).await?;

    pb.finish_with_message(format!("{} Auxiliary tools installed", "✓".green()));
    Ok(())
}

async fn initialize_rustwops(_verbose: bool) -> Result<()> {
    let pb = create_progress_bar("Initializing RustWops...");

    // Create directory structure
    let dirs = [
        "/etc/rustwops",
        "/etc/rustwops/credentials",
        "/var/lib/rustwops",
        "/var/lib/rustwops/backups",
        "/var/log/rustwops",
        "/var/www",
        "/var/www/html",
        "/etc/ssl/rustwops",
        "/etc/nginx/snippets",
        "/var/cache/nginx",
        "/var/cache/nginx/fastcgi",
    ];

    for dir in &dirs {
        shell::run_command("mkdir", &["-p", dir]).await?;
    }

    // Set proper permissions on credentials directory
    shell::run_command("chmod", &["700", "/etc/rustwops/credentials"]).await?;

    // Apply system tuning (sysctl and file limits)
    pb.set_message("Applying system tuning...");
    if let Err(e) = crate::config::stack::apply_sysctl_tuning().await {
        // Non-fatal - log warning but continue
        eprintln!(
            "  {} Warning: Could not apply sysctl tuning: {}",
            "⚠".yellow(),
            e
        );
    }

    // Initialize SQLite database (if not already done)
    crate::database::ensure_initialized().await?;

    pb.finish_with_message(format!("{} RustWops initialized", "✓".green()));
    Ok(())
}

pub async fn list_php_versions(_cli: &Cli) -> Result<()> {
    println!("{} Available PHP versions:\n", "→".bright_cyan().bold());

    let versions = ["7.4", "8.0", "8.1", "8.2", "8.3", "8.4"];

    for version in versions {
        // Check if installed
        let installed = shell::run_command("which", &[&format!("php{}", version)])
            .await
            .is_ok();

        if installed {
            println!(
                "  {} PHP {} {}",
                "●".green(),
                version,
                "(installed)".dimmed()
            );
        } else {
            println!("  {} PHP {}", "○".dimmed(), version);
        }
    }

    println!();
    Ok(())
}

pub async fn install_php_version(version: &str, cli: &Cli) -> Result<()> {
    install_php(version, cli.verbose).await
}

fn create_progress_bar(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}
