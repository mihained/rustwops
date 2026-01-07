use anyhow::Result;
use colored::Colorize;
use tabled::{Table, Tabled};

use super::SiteType;
use crate::database;
use crate::Cli;

#[derive(Tabled)]
struct SiteRow {
    #[tabled(rename = "Domain")]
    domain: String,
    #[tabled(rename = "Type")]
    site_type: String,
    #[tabled(rename = "PHP")]
    php: String,
    #[tabled(rename = "SSL")]
    ssl: String,
    #[tabled(rename = "Status")]
    status: String,
}

pub async fn execute(filter_type: Option<SiteType>, detailed: bool, _cli: &Cli) -> Result<()> {
    let all_sites = database::sites::list().await?;

    // Get staging domains to filter them out
    let staging_entries = database::staging::list().await.unwrap_or_default();
    let staging_domains: Vec<&str> = staging_entries
        .iter()
        .map(|s| s.staging_domain.as_str())
        .collect();

    // Filter out staging sites - only show production sites
    let sites: Vec<_> = all_sites
        .into_iter()
        .filter(|s| !staging_domains.contains(&s.domain.as_str()))
        .collect();

    if sites.is_empty() {
        println!("{} No sites found.\n", "→".bright_cyan());
        return Ok(());
    }

    // Filter by type if specified
    let sites: Vec<_> = if let Some(site_type) = filter_type {
        let type_str = site_type.to_string();
        sites
            .into_iter()
            .filter(|s| s.site_type == type_str)
            .collect()
    } else {
        sites
    };

    if sites.is_empty() {
        println!("{} No sites found matching filter.\n", "→".bright_cyan());
        return Ok(());
    }

    println!("{} Sites ({}):\n", "→".bright_cyan().bold(), sites.len());

    let rows: Vec<SiteRow> = sites
        .iter()
        .map(|site| {
            let ssl_status = if site.has_ssl {
                "✓".green().to_string()
            } else {
                "✗".dimmed().to_string()
            };

            let status = if site.enabled {
                "● enabled".green().to_string()
            } else {
                "○ disabled".yellow().to_string()
            };

            SiteRow {
                domain: site.domain.clone(),
                site_type: site.site_type.clone(),
                php: site.php_version.clone().unwrap_or_else(|| "-".to_string()),
                ssl: ssl_status,
                status,
            }
        })
        .collect();

    let table = Table::new(&rows).to_string();
    println!("{}", table);

    if detailed {
        println!("\n{} Detailed Information:\n", "→".bright_cyan());
        for site in &sites {
            println!("  {} {}", "●".bright_cyan(), site.domain.bright_white());
            println!("    Type: {}", site.site_type);
            if let Some(ref php) = site.php_version {
                println!("    PHP: {}", php);
            }
            if let Some(ref cache) = site.cache_type {
                println!("    Cache: {}", cache);
            }
            println!("    Webroot: {}", site.webroot);
            println!("    Created: {}", site.created_at);
            println!();
        }
    }

    Ok(())
}
