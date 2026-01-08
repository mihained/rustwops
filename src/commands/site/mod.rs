use clap::Subcommand;

use crate::Cli;

pub mod cache;
pub mod create;
pub mod delete;
pub mod info;
pub mod list;

#[derive(Clone, Subcommand)]
pub enum SiteCommand {
    /// Create a new website
    Create {
        /// Domain name
        domain: String,

        /// Site type
        #[arg(long, value_enum, default_value = "php")]
        r#type: SiteType,

        /// PHP version (for wp/php sites). Auto-detects latest if not specified.
        #[arg(long)]
        php: Option<String>,

        /// Enable MySQL database
        #[arg(long)]
        mysql: bool,

        /// Cache type
        #[arg(long, value_enum)]
        cache: Option<CacheType>,

        /// Enable SSL certificate
        #[arg(long)]
        ssl: bool,

        /// Issue wildcard SSL certificate
        #[arg(long)]
        wildcard: bool,

        /// DNS provider for wildcard SSL
        #[arg(long, value_enum)]
        dns: Option<DnsProvider>,

        /// Upstream port (for proxy type)
        #[arg(long)]
        upstream: Option<u16>,
    },

    /// Delete a website
    Delete {
        /// Domain name
        domain: String,

        /// Delete all (files + database)
        #[arg(long)]
        all: bool,

        /// Delete only files
        #[arg(long)]
        files: bool,

        /// Delete only database
        #[arg(long)]
        db: bool,
    },

    /// Update website configuration
    Update {
        /// Domain name
        domain: String,

        /// Change PHP version
        #[arg(long)]
        php: Option<String>,

        /// Change cache type
        #[arg(long, value_enum)]
        cache: Option<CacheType>,
    },

    /// List all websites
    List {
        /// Filter by site type
        #[arg(long, value_enum)]
        r#type: Option<SiteType>,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },

    /// Show website information
    Info {
        /// Domain name
        domain: String,
    },

    /// View website logs
    Log {
        /// Domain name
        domain: String,

        /// Log type
        #[arg(long, value_enum, default_value = "access")]
        r#type: LogType,

        /// Follow log output
        #[arg(long)]
        tail: bool,

        /// Number of lines to show
        #[arg(short, default_value = "50")]
        n: usize,
    },

    /// Enable a website
    Enable {
        /// Domain name
        domain: String,
    },

    /// Disable a website
    Disable {
        /// Domain name
        domain: String,
    },

    /// Run WP-CLI command on a WordPress site
    Wp {
        /// Domain name
        domain: String,

        /// WP-CLI arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Manage PM2 for Node.js sites
    Pm2 {
        /// Domain name
        domain: String,

        /// PM2 command
        #[arg(value_enum)]
        action: Pm2Action,
    },

    /// Purge cache for a WordPress site
    CachePurge {
        /// Domain name
        domain: String,

        /// Purge all caches (FastCGI + Redis object cache)
        #[arg(long)]
        all: bool,

        /// Purge only FastCGI/page cache
        #[arg(long)]
        page: bool,

        /// Purge only Redis object cache
        #[arg(long)]
        object: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SiteType {
    /// WordPress site
    Wp,
    /// PHP site
    #[default]
    Php,
    /// Static site
    Static,
    /// Reverse proxy
    Proxy,
    /// Node.js with PM2
    Node,
}

impl std::fmt::Display for SiteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SiteType::Wp => write!(f, "wp"),
            SiteType::Php => write!(f, "php"),
            SiteType::Static => write!(f, "static"),
            SiteType::Proxy => write!(f, "proxy"),
            SiteType::Node => write!(f, "node"),
        }
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum CacheType {
    /// No caching
    None,
    /// FastCGI cache (Nginx)
    Fastcgi,
    /// Redis object cache
    Redis,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum DnsProvider {
    Cloudflare,
    Digitalocean,
    Route53,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum LogType {
    Access,
    Error,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Pm2Action {
    Start,
    Stop,
    Restart,
    Logs,
    Status,
}

pub async fn execute(command: SiteCommand, cli: &Cli) -> anyhow::Result<()> {
    match command {
        SiteCommand::Create {
            domain,
            r#type,
            php,
            mysql,
            cache,
            ssl,
            wildcard,
            dns,
            upstream,
        } => {
            // Auto-detect PHP version if not specified
            let php_version = match php {
                Some(v) => v,
                None => {
                    if matches!(r#type, SiteType::Wp | SiteType::Php) {
                        crate::config::php::detect_latest_version().await?
                    } else {
                        String::new() // Not needed for static/proxy/node
                    }
                }
            };
            create::execute(
                &domain,
                r#type,
                &php_version,
                mysql,
                cache,
                ssl,
                wildcard,
                dns,
                upstream,
                cli,
            )
            .await
        }
        SiteCommand::Delete {
            domain,
            all,
            files,
            db,
        } => delete::execute(&domain, all, files, db, cli).await,
        SiteCommand::Update { .. } => {
            anyhow::bail!("Site update not yet implemented. Coming soon!")
        }
        SiteCommand::List { r#type, detailed } => list::execute(r#type, detailed, cli).await,
        SiteCommand::Info { domain } => info::execute(&domain, cli).await,
        SiteCommand::Log { .. } => {
            anyhow::bail!("Site log not yet implemented. Coming soon!")
        }
        SiteCommand::Enable { .. } => {
            anyhow::bail!("Site enable not yet implemented. Coming soon!")
        }
        SiteCommand::Disable { .. } => {
            anyhow::bail!("Site disable not yet implemented. Coming soon!")
        }
        SiteCommand::Wp { .. } => {
            anyhow::bail!("WP-CLI wrapper not yet implemented. Coming soon!")
        }
        SiteCommand::Pm2 { .. } => {
            anyhow::bail!("PM2 wrapper not yet implemented. Coming soon!")
        }
        SiteCommand::CachePurge {
            domain,
            all,
            page,
            object,
        } => cache::purge(&domain, all, page, object, cli).await,
    }
}
