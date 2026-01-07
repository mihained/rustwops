use clap::Subcommand;

use crate::Cli;

pub mod install;
pub mod remove;
pub mod status;
pub mod update;

#[derive(Clone, Subcommand)]
pub enum StackCommand {
    /// Install server stack components
    Install {
        /// Install all components (Nginx, PHP, MySQL, Redis, Node.js)
        #[arg(long)]
        all: bool,

        /// Components to install
        #[arg(value_enum)]
        components: Vec<Component>,

        /// PHP version to install
        #[arg(long, default_value = "8.3")]
        php_version: String,

        /// Database type
        #[arg(long, value_enum, default_value = "mariadb")]
        db_type: DbType,

        /// Node.js version (LTS)
        #[arg(long, default_value = "20")]
        node_version: String,

        /// Use custom Nginx build (HTTP/3, Brotli support)
        #[arg(long)]
        nginx_custom: bool,
    },

    /// Remove stack components
    Remove {
        /// Components to remove
        #[arg(required = true)]
        components: Vec<Component>,

        /// Purge configuration files
        #[arg(long)]
        purge: bool,
    },

    /// Update stack components
    Update {
        /// Components to update (all if empty)
        components: Vec<Component>,
    },

    /// Show stack status
    Status,

    /// List available PHP versions
    PhpVersions,

    /// Install additional PHP version
    PhpInstall {
        /// PHP version to install
        version: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Component {
    Nginx,
    Php,
    Mysql,
    Redis,
    Nodejs,
    /// Fail2Ban intrusion prevention
    Fail2ban,
    /// MySQLTuner database optimization tool
    Mysqltuner,
    /// ClamAV antivirus
    Clamav,
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum DbType {
    #[default]
    Mariadb,
    Mysql,
}

pub async fn execute(command: StackCommand, cli: &Cli) -> anyhow::Result<()> {
    match command {
        StackCommand::Install {
            all,
            components,
            php_version,
            db_type,
            node_version,
            nginx_custom,
        } => {
            install::execute(
                all,
                components,
                &php_version,
                db_type,
                &node_version,
                nginx_custom,
                cli,
            )
            .await
        }
        StackCommand::Remove { components, purge } => remove::execute(components, purge, cli).await,
        StackCommand::Update { components } => update::execute(components, cli).await,
        StackCommand::Status => status::execute(cli).await,
        StackCommand::PhpVersions => install::list_php_versions(cli).await,
        StackCommand::PhpInstall { version } => install::install_php_version(&version, cli).await,
    }
}
