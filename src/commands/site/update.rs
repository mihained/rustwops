use anyhow::Result;
use colored::Colorize;

use crate::commands::site::CacheType;
use crate::config::{nginx, php};
use crate::database;
use crate::utils::shell;
use crate::Cli;

/// Update site configuration (PHP version, cache type)
pub async fn execute(
    domain: &str,
    new_php: Option<String>,
    new_cache: Option<CacheType>,
    cli: &Cli,
) -> Result<()> {
    // Check if site exists
    let site = database::sites::get_by_domain(domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Site {} not found", domain))?;

    if new_php.is_none() && new_cache.is_none() {
        anyhow::bail!("Please specify --php or --cache to update");
    }

    println!(
        "{} Updating site {}...\n",
        "→".bright_cyan().bold(),
        domain.bright_white()
    );

    // Track what changed
    let mut changes = Vec::new();

    // Handle PHP version change
    if let Some(ref php_version) = new_php {
        update_php_version(domain, &site, php_version, cli.verbose).await?;
        changes.push(format!("PHP version: {}", php_version));
    }

    // Handle cache type change
    if let Some(cache_type) = new_cache {
        update_cache_type(domain, &site, cache_type, cli.verbose).await?;
        changes.push(format!(
            "Cache type: {}",
            match cache_type {
                CacheType::None => "none",
                CacheType::Fastcgi => "fastcgi",
                CacheType::Redis => "redis",
            }
        ));
    }

    // Reload services
    println!("\n  {} Reloading services...", "→".bright_cyan());

    // Test nginx config first
    shell::run_command("nginx", &["-t"]).await?;

    // Reload nginx
    shell::run_command("systemctl", &["reload", "nginx"]).await?;
    println!("  {} Reloaded nginx", "✓".green());

    // Reload PHP-FPM for the new version
    let php_version = new_php
        .as_ref()
        .or(site.php_version.as_ref())
        .cloned()
        .unwrap_or_default();

    if !php_version.is_empty() {
        let service = format!("php{}-fpm", php_version);
        shell::run_command("systemctl", &["reload", &service]).await?;
        println!("  {} Reloaded {}", "✓".green(), service);
    }

    println!(
        "\n{} Site {} updated successfully!",
        "✓".green().bold(),
        domain.bright_white()
    );

    println!("\n  Changes:");
    for change in changes {
        println!("    {} {}", "•".bright_cyan(), change);
    }

    Ok(())
}

/// Update PHP version for a site
async fn update_php_version(
    domain: &str,
    site: &database::sites::Site,
    new_version: &str,
    verbose: bool,
) -> Result<()> {
    use std::io::{self, Write};

    let old_version = site.php_version.as_deref().unwrap_or("");

    // Validate site type supports PHP
    if site.site_type == "static" || site.site_type == "proxy" || site.site_type == "node" {
        anyhow::bail!(
            "Site type '{}' does not use PHP. Cannot change PHP version.",
            site.site_type
        );
    }

    // Check if new version is installed
    let installed = php::get_installed_versions().await;
    if !installed.contains(&new_version.to_string()) {
        anyhow::bail!(
            "PHP {} is not installed. Installed versions: {}",
            new_version,
            installed.join(", ")
        );
    }

    if old_version == new_version {
        println!(
            "  {} Site already using PHP {}",
            "→".bright_cyan(),
            new_version
        );
        return Ok(());
    }

    print!(
        "  {} Updating PHP {} -> {}...",
        "→".bright_cyan(),
        old_version,
        new_version
    );
    io::stdout().flush().ok();

    // Parse site type and cache type
    let site_type = match site.site_type.as_str() {
        "wp" => crate::commands::site::SiteType::Wp,
        "php" => crate::commands::site::SiteType::Php,
        _ => crate::commands::site::SiteType::Php,
    };

    let cache_type = site.cache_type.as_deref().and_then(|c| match c {
        "fastcgi" => Some(CacheType::Fastcgi),
        "redis" => Some(CacheType::Redis),
        "none" => Some(CacheType::None),
        _ => None,
    });

    // Regenerate nginx config with new PHP version
    nginx::create_site_config(
        domain,
        site_type,
        new_version,
        cache_type,
        &site.webroot,
        None,
    )
    .await?;

    // Create new PHP-FPM pool
    php::create_pool(domain, new_version).await?;

    // Delete old PHP-FPM pool
    if !old_version.is_empty() && old_version != new_version {
        php::delete_pool(domain, old_version).await?;

        // Reload old PHP-FPM service
        let old_service = format!("php{}-fpm", old_version);
        let _ = shell::run_command("systemctl", &["reload", &old_service]).await;
    }

    // Update database
    database::sites::update_php_version(domain, new_version).await?;

    println!(" {}", "done".green());

    if verbose {
        println!("    - Regenerated nginx config");
        println!("    - Created PHP-FPM pool for PHP {}", new_version);
        if !old_version.is_empty() && old_version != new_version {
            println!("    - Removed PHP-FPM pool for PHP {}", old_version);
        }
    }

    Ok(())
}

/// Update cache type for a site
async fn update_cache_type(
    domain: &str,
    site: &database::sites::Site,
    new_cache: CacheType,
    verbose: bool,
) -> Result<()> {
    use std::io::{self, Write};

    let old_cache = site.cache_type.as_deref().unwrap_or("none");
    let new_cache_str = match new_cache {
        CacheType::None => "none",
        CacheType::Fastcgi => "fastcgi",
        CacheType::Redis => "redis",
    };

    // Only WordPress sites support caching
    if site.site_type != "wp" {
        anyhow::bail!(
            "Cache configuration is only available for WordPress sites. {} is a {} site.",
            domain,
            site.site_type
        );
    }

    if old_cache == new_cache_str {
        println!(
            "  {} Site already using {} cache",
            "→".bright_cyan(),
            new_cache_str
        );
        return Ok(());
    }

    print!(
        "  {} Updating cache {} -> {}...",
        "→".bright_cyan(),
        old_cache,
        new_cache_str
    );
    io::stdout().flush().ok();

    let webroot = &site.webroot;
    let php_version = site.php_version.as_deref().unwrap_or("8.3");

    // Regenerate nginx config with new cache type
    nginx::create_site_config(
        domain,
        crate::commands::site::SiteType::Wp,
        php_version,
        Some(new_cache),
        webroot,
        None,
    )
    .await?;

    println!(" {}", "done".green());

    // Handle WordPress plugin changes
    println!("  {} Updating WordPress plugins...", "→".bright_cyan());
    update_wordpress_cache_plugins(domain, webroot, old_cache, new_cache, verbose).await?;

    // Update database
    database::sites::update_cache(domain, Some(new_cache)).await?;

    if verbose {
        println!("    - Regenerated nginx config");
        println!("    - Updated cache type to {}", new_cache_str);
    }

    Ok(())
}

/// Update WordPress cache plugins when changing cache type
async fn update_wordpress_cache_plugins(
    _domain: &str,
    webroot: &str,
    old_cache: &str,
    new_cache: CacheType,
    verbose: bool,
) -> Result<()> {
    // Determine what plugins to install/remove
    let needs_nginx_helper = matches!(new_cache, CacheType::Fastcgi | CacheType::Redis);
    let needs_redis_cache = matches!(new_cache, CacheType::Redis);

    let had_nginx_helper = old_cache == "fastcgi" || old_cache == "redis";
    let had_redis_cache = old_cache == "redis";

    // Handle Nginx Helper plugin
    if needs_nginx_helper && !had_nginx_helper {
        // Install Nginx Helper
        print!("    {} Installing Nginx Helper...", "→".bright_cyan());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        shell::run_command_with_output(
            "wp",
            &[
                "plugin",
                "install",
                "nginx-helper",
                "--activate",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await?;

        // Configure Nginx Helper
        let cache_method = match new_cache {
            CacheType::Fastcgi => "fastcgi",
            CacheType::Redis => "redis",
            CacheType::None => "none",
        };
        configure_nginx_helper(webroot, cache_method, verbose).await?;

        println!(" {}", "done".green());
    } else if !needs_nginx_helper && had_nginx_helper {
        // Deactivate Nginx Helper (keep installed for potential reuse)
        print!("    {} Deactivating Nginx Helper...", "→".bright_cyan());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let _ = shell::run_command_with_output(
            "wp",
            &[
                "plugin",
                "deactivate",
                "nginx-helper",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await;

        println!(" {}", "done".green());
    } else if needs_nginx_helper && had_nginx_helper {
        // Update Nginx Helper configuration for new cache type
        print!(
            "    {} Updating Nginx Helper configuration...",
            "→".bright_cyan()
        );
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let cache_method = match new_cache {
            CacheType::Fastcgi => "fastcgi",
            CacheType::Redis => "redis",
            CacheType::None => "none",
        };
        configure_nginx_helper(webroot, cache_method, verbose).await?;

        println!(" {}", "done".green());
    }

    // Handle Redis Object Cache plugin
    if needs_redis_cache && !had_redis_cache {
        // Install Redis Object Cache
        print!("    {} Installing Redis Object Cache...", "→".bright_cyan());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        shell::run_command_with_output(
            "wp",
            &[
                "plugin",
                "install",
                "redis-cache",
                "--activate",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await?;

        // Configure Redis
        configure_redis_object_cache(webroot, verbose).await?;

        // Enable Redis object cache
        let _ = shell::run_command_with_output(
            "wp",
            &[
                "redis",
                "enable",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await;

        println!(" {}", "done".green());
    } else if !needs_redis_cache && had_redis_cache {
        // Disable and deactivate Redis Object Cache
        print!("    {} Disabling Redis Object Cache...", "→".bright_cyan());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        // Disable Redis object cache
        let _ = shell::run_command_with_output(
            "wp",
            &[
                "redis",
                "disable",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await;

        // Deactivate plugin
        let _ = shell::run_command_with_output(
            "wp",
            &[
                "plugin",
                "deactivate",
                "redis-cache",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await;

        println!(" {}", "done".green());
    }

    // Clear any existing cache when changing cache type
    print!("    {} Clearing caches...", "→".bright_cyan());
    std::io::Write::flush(&mut std::io::stdout()).ok();

    // Clear FastCGI cache
    let cache_path = "/var/cache/nginx/fastcgi";
    if std::path::Path::new(cache_path).exists() {
        let _ = shell::run_command("find", &[cache_path, "-type", "f", "-delete"]).await;
    }

    // Flush WordPress object cache
    let _ = shell::run_command_with_output(
        "wp",
        &[
            "cache",
            "flush",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    println!(" {}", "done".green());

    Ok(())
}

/// Configure Nginx Helper plugin
async fn configure_nginx_helper(webroot: &str, cache_method: &str, verbose: bool) -> Result<()> {
    let (enable_purge, cache_method_key) = match cache_method {
        "fastcgi" => ("1", "enable_fastcgi"),
        "redis" => ("1", "enable_redis"),
        _ => ("0", "enable_fastcgi"),
    };

    // Add the cache path constant to wp-config.php
    if cache_method == "fastcgi" {
        let _ = shell::run_command_with_output(
            "wp",
            &[
                "config",
                "set",
                "RT_WP_NGINX_HELPER_CACHE_PATH",
                "'/var/cache/nginx/fastcgi'",
                "--raw",
                "--type=constant",
                &format!("--path={}", webroot),
                "--allow-root",
            ],
            verbose,
        )
        .await;
    }

    // Configure plugin options via PHP
    let php_code = format!(
        r#"
$options = get_option('rt_wp_nginx_helper_options', array());
$options['enable_purge'] = '{}';
$options['cache_method'] = '{}';
$options['purge_homepage_on_edit'] = '1';
$options['purge_homepage_on_del'] = '1';
$options['purge_archive_on_edit'] = '1';
$options['purge_archive_on_del'] = '1';
$options['purge_archive_on_new_comment'] = '1';
$options['purge_archive_on_deleted_comment'] = '1';
$options['purge_page_on_mod'] = '1';
$options['purge_page_on_new_comment'] = '1';
$options['purge_page_on_deleted_comment'] = '1';
$options['purge_method'] = 'unlink_files';
$options['nginx_cache_path'] = '/var/cache/nginx/fastcgi';
$options['enable_stamp'] = '1';
$options['redis_hostname'] = '127.0.0.1';
$options['redis_port'] = '6379';
$options['redis_prefix'] = 'nginx-cache:';
update_option('rt_wp_nginx_helper_options', $options);

// Add purge capability to administrator role
$admin = get_role('administrator');
if ($admin && !$admin->has_cap('Nginx Helper | Purge cache')) {{
    $admin->add_cap('Nginx Helper | Purge cache');
}}

echo 'Nginx Helper configured';
"#,
        enable_purge, cache_method_key
    );

    shell::run_command_with_output(
        "wp",
        &[
            "eval",
            &php_code,
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await?;

    Ok(())
}

/// Configure Redis Object Cache plugin
async fn configure_redis_object_cache(webroot: &str, verbose: bool) -> Result<()> {
    // Set Redis configuration in wp-config.php
    let _ = shell::run_command_with_output(
        "wp",
        &[
            "config",
            "set",
            "WP_REDIS_HOST",
            "'127.0.0.1'",
            "--raw",
            "--type=constant",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    let _ = shell::run_command_with_output(
        "wp",
        &[
            "config",
            "set",
            "WP_REDIS_PORT",
            "6379",
            "--raw",
            "--type=constant",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    let _ = shell::run_command_with_output(
        "wp",
        &[
            "config",
            "set",
            "WP_REDIS_DATABASE",
            "0",
            "--raw",
            "--type=constant",
            &format!("--path={}", webroot),
            "--allow-root",
        ],
        verbose,
    )
    .await;

    Ok(())
}
