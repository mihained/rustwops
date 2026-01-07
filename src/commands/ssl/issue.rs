use anyhow::Result;
use colored::Colorize;

use super::{DnsProvider, KeyType};
use crate::utils::shell;

pub async fn execute_http(domain: &str, key_type: KeyType, staging: bool, verbose: bool) -> Result<()> {
    let mode = if staging { " (STAGING)" } else { "" };
    println!(
        "{} Issuing SSL certificate for {} (HTTP-01){}...\n",
        "→".bright_cyan().bold(),
        domain.bright_white(),
        mode.yellow()
    );

    // Get the actual webroot from the database
    let webroot = match crate::database::sites::get_by_domain(domain).await? {
        Some(site) => site.webroot,
        None => format!("/var/www/{}/prod/public", domain), // fallback
    };
    let cert_dir = format!("/etc/ssl/rustwops/{}", domain);

    // Create certificate directory
    shell::run_command("mkdir", &["-p", &cert_dir]).await?;

    // Build acme.sh command
    let staging_arg = if staging { "--staging" } else { "" };
    let key_arg = key_type.to_acme_arg();

    // Issue certificate using acme.sh (use Let's Encrypt by default)
    let acme_cmd = format!(
        r#"
        export HOME=/root
        ~/.acme.sh/acme.sh --issue \
            -d {domain} \
            --webroot {webroot} \
            {key_arg} \
            --server letsencrypt \
            {staging_arg} \
            --cert-file {cert_dir}/cert.pem \
            --key-file {cert_dir}/privkey.pem \
            --fullchain-file {cert_dir}/fullchain.pem \
            --ca-file {cert_dir}/chain.pem \
            --reloadcmd "systemctl reload nginx"
        "#,
        domain = domain,
        webroot = webroot,
        key_arg = key_arg,
        staging_arg = staging_arg,
        cert_dir = cert_dir
    );

    shell::run_shell_script(&acme_cmd, verbose).await?;

    println!(
        "\n{} SSL certificate issued successfully!{}\n",
        "✓".green().bold(),
        if staging { " (STAGING - not for production)" } else { "" }
    );

    println!("  Certificate: {}/fullchain.pem", cert_dir);
    println!("  Private Key: {}/privkey.pem", cert_dir);

    Ok(())
}

pub async fn execute_dns(domain: &str, provider: DnsProvider, key_type: KeyType, staging: bool, verbose: bool) -> Result<()> {
    let mode = if staging { " (STAGING)" } else { "" };
    println!(
        "{} Issuing wildcard SSL certificate for {} (DNS-01){}...\n",
        "→".bright_cyan().bold(),
        domain.bright_white(),
        mode.yellow()
    );

    let cert_dir = format!("/etc/ssl/rustwops/{}", domain);
    shell::run_command("mkdir", &["-p", &cert_dir]).await?;

    // Load DNS provider credentials
    let dns_env = get_dns_env(provider).await?;
    let dns_plugin = get_dns_plugin(provider);

    // Build acme.sh command
    let staging_arg = if staging { "--staging" } else { "" };
    let key_arg = key_type.to_acme_arg();

    // Issue wildcard certificate (use Let's Encrypt by default)
    let acme_cmd = format!(
        r#"
        export HOME=/root
        {dns_env}
        ~/.acme.sh/acme.sh --issue \
            -d {domain} \
            -d '*.{domain}' \
            --dns {dns_plugin} \
            {key_arg} \
            --server letsencrypt \
            {staging_arg} \
            --cert-file {cert_dir}/cert.pem \
            --key-file {cert_dir}/privkey.pem \
            --fullchain-file {cert_dir}/fullchain.pem \
            --ca-file {cert_dir}/chain.pem \
            --reloadcmd "systemctl reload nginx"
        "#,
        dns_env = dns_env,
        domain = domain,
        dns_plugin = dns_plugin,
        key_arg = key_arg,
        staging_arg = staging_arg,
        cert_dir = cert_dir
    );

    shell::run_shell_script(&acme_cmd, verbose).await?;

    println!(
        "\n{} Wildcard SSL certificate issued successfully!{}\n",
        "✓".green().bold(),
        if staging { " (STAGING - not for production)" } else { "" }
    );

    println!("  Domains: {}, *.{}", domain, domain);
    println!("  Certificate: {}/fullchain.pem", cert_dir);
    println!("  Private Key: {}/privkey.pem", cert_dir);

    Ok(())
}

fn get_dns_plugin(provider: DnsProvider) -> &'static str {
    match provider {
        DnsProvider::Cloudflare => "dns_cf",
        DnsProvider::Digitalocean => "dns_dgon",
        DnsProvider::Route53 => "dns_aws",
    }
}

async fn get_dns_env(provider: DnsProvider) -> Result<String> {
    // Read credentials from config file
    let config_path = "/etc/rustwops/dns-credentials.toml";

    let config_content = shell::read_file(config_path).await.map_err(|_| {
        anyhow::anyhow!(
            "DNS credentials not configured. Run 'rw ssl dns-config {:?}' first.",
            provider
        )
    })?;

    let config: toml::Value = toml::from_str(&config_content)?;

    match provider {
        DnsProvider::Cloudflare => {
            let cf = config.get("cloudflare").ok_or_else(|| {
                anyhow::anyhow!("Cloudflare credentials not found")
            })?;

            if let Some(token) = cf.get("token").and_then(|v| v.as_str()) {
                Ok(format!("export CF_Token='{}'", token))
            } else if let (Some(key), Some(email)) = (
                cf.get("api_key").and_then(|v| v.as_str()),
                cf.get("email").and_then(|v| v.as_str()),
            ) {
                Ok(format!(
                    "export CF_Key='{}'\nexport CF_Email='{}'",
                    key, email
                ))
            } else {
                anyhow::bail!("Cloudflare token or api_key+email required")
            }
        }
        DnsProvider::Digitalocean => {
            let do_config = config.get("digitalocean").ok_or_else(|| {
                anyhow::anyhow!("DigitalOcean credentials not found")
            })?;

            let api_key = do_config
                .get("api_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("DigitalOcean api_key not found"))?;

            Ok(format!("export DO_API_KEY='{}'", api_key))
        }
        DnsProvider::Route53 => {
            let aws = config.get("route53").ok_or_else(|| {
                anyhow::anyhow!("Route53 credentials not found")
            })?;

            let access_key = aws
                .get("access_key_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("AWS access_key_id not found"))?;

            let secret_key = aws
                .get("secret_access_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("AWS secret_access_key not found"))?;

            Ok(format!(
                "export AWS_ACCESS_KEY_ID='{}'\nexport AWS_SECRET_ACCESS_KEY='{}'",
                access_key, secret_key
            ))
        }
    }
}
