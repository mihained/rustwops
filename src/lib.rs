pub mod commands;
pub mod config;
pub mod database;
pub mod interactive;
pub mod utils;

// Re-export main types
pub use commands::site::{CacheType, DnsProvider, SiteType};

use clap::{Parser, Subcommand};

/// RustWops - High-performance web server stack management
#[derive(Parser)]
#[command(name = "rw")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Skip confirmation prompts
    #[arg(short, long, global = true)]
    pub yes: bool,

    /// Output format: text, json, yaml
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Yaml,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage server stack (Nginx, PHP, MySQL, Redis, Node.js)
    Stack {
        #[command(subcommand)]
        command: commands::stack::StackCommand,
    },

    /// Manage websites and applications
    Site {
        #[command(subcommand)]
        command: commands::site::SiteCommand,
    },

    /// Manage SSL certificates
    Ssl {
        #[command(subcommand)]
        command: commands::ssl::SslCommand,
    },

    /// Manage staging environments
    Staging {
        #[command(subcommand)]
        command: commands::staging::StagingCommand,
    },

    /// Manage backups
    Backup {
        #[command(subcommand)]
        command: commands::backup::BackupCommand,
    },

    /// Manage services
    Service {
        #[command(subcommand)]
        command: commands::service::ServiceCommand,
    },

    /// Security tools (Fail2Ban, ClamAV, MySQLTuner)
    Security {
        #[command(subcommand)]
        command: commands::security::SecurityCommand,
    },

    /// View logs (sites, nginx, php, mysql, fail2ban)
    Log {
        #[command(subcommand)]
        command: commands::log::LogCommand,
    },

    /// Start the REST API server (for dashboard)
    #[cfg(feature = "api")]
    Api {
        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,

        /// Port
        #[arg(long, default_value = "8080")]
        port: u16,
    },

    /// Show system information
    Info,

    /// Update RustWops to latest version
    Update {
        /// Check for updates without installing
        #[arg(long)]
        check: bool,
    },
}
