use clap::Subcommand;

use crate::Cli;

#[derive(Clone, Subcommand)]
pub enum BackupCommand {
    /// Create a backup
    Create {
        /// Domain name (all sites if not specified)
        domain: Option<String>,

        /// Backup name/label
        #[arg(long)]
        name: Option<String>,

        /// Backup only database
        #[arg(long)]
        db_only: bool,

        /// Backup only files
        #[arg(long)]
        files_only: bool,
    },

    /// Restore from backup
    Restore {
        /// Backup ID or file path
        backup: String,

        /// Restore to different domain
        #[arg(long)]
        target: Option<String>,

        /// Restore only database
        #[arg(long)]
        db_only: bool,

        /// Restore only files
        #[arg(long)]
        files_only: bool,
    },

    /// List backups
    List {
        /// Filter by domain
        #[arg(long)]
        domain: Option<String>,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },

    /// Delete backup
    Delete {
        /// Backup ID
        backup_id: Option<String>,

        /// Delete backups older than N days
        #[arg(long)]
        older_than: Option<u32>,
    },

    /// Configure backup settings
    Config {
        /// Backup directory
        #[arg(long)]
        dir: Option<String>,

        /// Retention days
        #[arg(long)]
        retention: Option<u32>,

        /// S3 bucket name
        #[arg(long)]
        s3_bucket: Option<String>,

        /// S3 region
        #[arg(long)]
        s3_region: Option<String>,

        /// Backup schedule (cron format)
        #[arg(long)]
        schedule: Option<String>,
    },

    /// Show backup configuration
    ConfigShow,
}

pub async fn execute(command: BackupCommand, _cli: &Cli) -> anyhow::Result<()> {
    match command {
        BackupCommand::Create { .. } => {
            anyhow::bail!("Backup create not yet implemented. Coming soon!")
        }
        BackupCommand::Restore { .. } => {
            anyhow::bail!("Backup restore not yet implemented. Coming soon!")
        }
        BackupCommand::List { .. } => {
            anyhow::bail!("Backup list not yet implemented. Coming soon!")
        }
        BackupCommand::Delete { .. } => {
            anyhow::bail!("Backup delete not yet implemented. Coming soon!")
        }
        BackupCommand::Config { .. } => {
            anyhow::bail!("Backup config not yet implemented. Coming soon!")
        }
        BackupCommand::ConfigShow => {
            anyhow::bail!("Backup config-show not yet implemented. Coming soon!")
        }
    }
}
