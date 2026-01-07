use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use crate::database;
use crate::utils::shell;
use crate::Cli;

pub async fn execute(domain: &str, all: bool, files: bool, db: bool, cli: &Cli) -> Result<()> {
    println!(
        "{} Deleting site: {}\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Check if site exists
    if !database::sites::exists(domain).await? {
        anyhow::bail!("Site {} does not exist", domain);
    }

    // Determine what to delete
    let delete_files = all || files;
    let delete_db = all || db;

    if !delete_files && !delete_db {
        anyhow::bail!("Specify --all, --files, or --db");
    }

    // Confirm deletion
    if !cli.yes {
        let mut msg = format!("Delete site {}?", domain);
        if delete_files {
            msg.push_str(" (files)");
        }
        if delete_db {
            msg.push_str(" (database)");
        }

        let confirm = Confirm::new().with_prompt(&msg).default(false).interact()?;

        if !confirm {
            println!("{}", "Aborted.".yellow());
            return Ok(());
        }
    }

    // Get site info
    let site = database::sites::get(domain).await?;

    // Disable site first
    disable_site(domain).await?;
    println!("  {} Disabled Nginx site", "✓".green());

    // Delete files
    if delete_files {
        delete_site_files(domain).await?;
        println!("  {} Deleted site files", "✓".green());

        // Remove PHP-FPM pool
        if let Some(php_version) = &site.php_version {
            delete_php_pool(domain, php_version).await?;
            println!("  {} Removed PHP-FPM pool", "✓".green());
        }

        // Remove Nginx config
        delete_nginx_config(domain).await?;
        println!("  {} Removed Nginx config", "✓".green());

        // Remove SSL certificates
        delete_ssl_certs(domain).await?;
    }

    // Delete database
    if delete_db {
        if let Some(db_info) = database::databases::get_by_domain(domain).await? {
            delete_database(&db_info.db_name, &db_info.db_user).await?;
            println!("  {} Deleted database", "✓".green());
        }
    }

    // Remove from RustWops database
    if all {
        database::sites::delete(domain).await?;
    }

    // Reload Nginx (if installed)
    if shell::command_exists("nginx").await {
        let _ = shell::run_command("systemctl", &["reload", "nginx"]).await;
        println!("  {} Reloaded Nginx", "✓".green());
    }

    println!(
        "\n{} Site {} deleted successfully!\n",
        "✓".green().bold(),
        domain
    );

    Ok(())
}

async fn disable_site(domain: &str) -> Result<()> {
    let enabled = format!("/etc/nginx/sites-enabled/{}", domain);
    shell::run_command("rm", &["-f", &enabled]).await?;
    Ok(())
}

async fn delete_site_files(domain: &str) -> Result<()> {
    let webroot = format!("/var/www/{}", domain);
    shell::run_command("rm", &["-rf", &webroot]).await?;
    Ok(())
}

async fn delete_php_pool(domain: &str, php_version: &str) -> Result<()> {
    let pool_file = format!("/etc/php/{}/fpm/pool.d/{}.conf", php_version, domain);
    shell::run_command("rm", &["-f", &pool_file]).await?;

    // Try to reload PHP-FPM, but don't fail if service doesn't exist
    let fpm = format!("php{}-fpm", php_version);
    let _ = shell::run_command("systemctl", &["reload", &fpm]).await;

    Ok(())
}

async fn delete_nginx_config(domain: &str) -> Result<()> {
    let available = format!("/etc/nginx/sites-available/{}", domain);
    shell::run_command("rm", &["-f", &available]).await?;
    Ok(())
}

async fn delete_ssl_certs(domain: &str) -> Result<()> {
    let cert_dir = format!("/etc/ssl/rustwops/{}", domain);
    shell::run_command("rm", &["-rf", &cert_dir]).await?;

    // Also remove from acme.sh
    let acme_remove = format!(
        r#"
        export HOME=/root
        ~/.acme.sh/acme.sh --remove -d {} 2>/dev/null || true
        "#,
        domain
    );
    shell::run_shell_script(&acme_remove, false).await.ok();

    Ok(())
}

async fn delete_database(db_name: &str, db_user: &str) -> Result<()> {
    let sql = format!(
        r#"
        DROP DATABASE IF EXISTS `{}`;
        DROP USER IF EXISTS '{}'@'localhost';
        "#,
        db_name, db_user
    );

    shell::run_command("mysql", &["-e", &sql]).await?;
    Ok(())
}
