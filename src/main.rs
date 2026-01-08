use clap::Parser;
use colored::Colorize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rustwops::{commands, interactive, Cli, Commands};

fn setup_logging(verbose: bool) {
    let filter = if verbose {
        "rustwops=debug,info"
    } else {
        "rustwops=info,warn"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();
}

fn print_banner() {
    println!(
        "{}",
        r#"
  ____           _   __        __
 |  _ \ _   _ ___| |_ \ \      / /__  _ __  ___
 | |_) | | | / __| __| \ \ /\ / / _ \| '_ \/ __|
 |  _ <| |_| \__ \ |_   \ V  V / (_) | |_) \__ \
 |_| \_\\__,_|___/\__|   \_/\_/ \___/| .__/|___/
                                     |_|
"#
        .bright_cyan()
    );
    println!(
        "  {} {}\n",
        "RustWops".bright_white().bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("\n{} {}\n", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    // If no arguments provided, launch interactive mode
    if std::env::args().len() == 1 {
        setup_logging(false);
        // Interactive mode supports per-action sudo escalation
        return interactive::run().await;
    }

    let cli = Cli::parse();

    setup_logging(cli.verbose);

    // Root checks are now done per-command based on operation type

    match &cli.command {
        Commands::Stack { command } => {
            commands::stack::execute(command.clone(), &cli).await?;
        }
        Commands::Site { command } => {
            commands::site::execute(command.clone(), &cli).await?;
        }
        Commands::Ssl { command } => {
            commands::ssl::execute(command.clone(), &cli).await?;
        }
        Commands::Staging { command } => {
            commands::staging::execute(command.clone(), &cli).await?;
        }
        Commands::Backup { command } => {
            commands::backup::execute(command.clone(), &cli).await?;
        }
        Commands::Service { command } => {
            commands::service::execute(command.clone(), &cli).await?;
        }
        Commands::Security { command } => {
            commands::security::execute(command.clone(), &cli).await?;
        }
        Commands::Log { command } => {
            commands::log::execute(command.clone(), &cli).await?;
        }
        #[cfg(feature = "api")]
        Commands::Api { bind, port } => {
            commands::api::start_server(bind, *port).await?;
        }
        Commands::Info => {
            print_banner();
            commands::info::execute(&cli).await?;
        }
        Commands::Update { check } => {
            commands::update::execute(*check, &cli).await?;
        }
    }

    Ok(())
}
