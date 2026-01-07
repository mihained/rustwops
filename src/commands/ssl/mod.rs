use clap::Subcommand;

use crate::Cli;

pub mod issue;

#[derive(Clone, Subcommand)]
pub enum SslCommand {
    /// Issue SSL certificate
    Issue {
        /// Domain name
        domain: String,

        /// Issue wildcard certificate
        #[arg(long)]
        wildcard: bool,

        /// DNS provider for DNS-01 challenge
        #[arg(long, value_enum)]
        dns: Option<DnsProvider>,

        /// Key type
        #[arg(long, value_enum, default_value = "ec-384")]
        key_type: KeyType,

        /// Use Let's Encrypt staging server
        #[arg(long)]
        staging: bool,
    },

    /// Renew SSL certificates
    Renew {
        /// Domain name (all if not specified)
        domain: Option<String>,

        /// Force renewal
        #[arg(long)]
        force: bool,
    },

    /// Revoke SSL certificate
    Revoke {
        /// Domain name
        domain: String,
    },

    /// Show SSL certificate status
    Status {
        /// Domain name (all if not specified)
        domain: Option<String>,
    },

    /// Configure DNS provider credentials
    DnsConfig {
        /// DNS provider
        #[arg(value_enum)]
        provider: DnsProvider,
    },

    /// List supported DNS providers
    DnsProviders,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum DnsProvider {
    Cloudflare,
    Digitalocean,
    Route53,
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum KeyType {
    #[default]
    #[value(name = "ec-384")]
    Ec384,
    #[value(name = "ec-256")]
    Ec256,
    #[value(name = "rsa-2048")]
    Rsa2048,
    #[value(name = "rsa-4096")]
    Rsa4096,
}

impl KeyType {
    pub fn to_acme_arg(&self) -> &str {
        match self {
            KeyType::Ec384 => "--keylength ec-384",
            KeyType::Ec256 => "--keylength ec-256",
            KeyType::Rsa2048 => "--keylength 2048",
            KeyType::Rsa4096 => "--keylength 4096",
        }
    }
}

pub async fn execute(command: SslCommand, cli: &Cli) -> anyhow::Result<()> {
    match command {
        SslCommand::Issue {
            domain,
            wildcard,
            dns,
            key_type,
            staging,
        } => {
            if wildcard {
                let provider = dns.ok_or_else(|| {
                    anyhow::anyhow!("DNS provider required for wildcard (use --dns)")
                })?;
                issue::execute_dns(&domain, provider, key_type, staging, cli.verbose).await
            } else {
                issue::execute_http(&domain, key_type, staging, cli.verbose).await
            }
        }
        SslCommand::Renew { .. } => {
            anyhow::bail!("SSL renew not yet implemented. Coming soon!")
        }
        SslCommand::Revoke { .. } => {
            anyhow::bail!("SSL revoke not yet implemented. Coming soon!")
        }
        SslCommand::Status { .. } => {
            anyhow::bail!("SSL status not yet implemented. Coming soon!")
        }
        SslCommand::DnsConfig { .. } => {
            anyhow::bail!("DNS config not yet implemented. Coming soon!")
        }
        SslCommand::DnsProviders => {
            println!("Supported DNS providers:");
            println!("  - cloudflare");
            println!("  - digitalocean");
            println!("  - route53");
            Ok(())
        }
    }
}
