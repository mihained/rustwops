use anyhow::Result;
use colored::Colorize;

use crate::Cli;

const GITHUB_REPO: &str = "rustwops/rustwops";

pub async fn execute(check_only: bool, _cli: &Cli) -> Result<()> {
    use crate::utils::system::require_root;

    println!("{} Checking for updates...\n", "→".bright_cyan().bold());

    let current_version = env!("CARGO_PKG_VERSION");
    println!("  Current version: {}", current_version.bright_white());

    // Check for latest release
    let latest = check_latest_version().await?;

    if latest.version == current_version {
        println!(
            "\n{} You are running the latest version!",
            "✓".green().bold()
        );
        return Ok(());
    }

    println!("  Latest version:  {}", latest.version.bright_yellow());

    if check_only {
        println!(
            "\n{} Update available! Run 'sudo rw update' to install.",
            "→".bright_cyan()
        );
        return Ok(());
    }

    // Installing update requires root
    require_root("update RustWops")?;

    // Download and install update
    println!("\n{} Downloading update...", "→".bright_cyan());

    download_and_install(&latest.download_url).await?;

    println!(
        "\n{} Updated to version {}!",
        "✓".green().bold(),
        latest.version
    );

    if !latest.release_notes.is_empty() {
        println!("\n{} Release notes:", "→".bright_cyan());
        println!("{}", latest.release_notes);
    }

    Ok(())
}

struct LatestRelease {
    version: String,
    download_url: String,
    release_notes: String,
}

async fn check_latest_version() -> Result<LatestRelease> {
    let client = reqwest::Client::new();

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let response = client
        .get(&url)
        .header("User-Agent", "rustwops")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let version = response["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .trim_start_matches('v')
        .to_string();

    // Find the right asset for this platform
    let arch = std::env::consts::ARCH;
    let asset_name = match arch {
        "x86_64" => "rw-linux-amd64",
        "aarch64" => "rw-linux-arm64",
        _ => "rw-linux-amd64",
    };

    let download_url = response["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|a| {
                a["name"]
                    .as_str()
                    .map(|n| n.contains(asset_name))
                    .unwrap_or(false)
            })
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .unwrap_or("")
        .to_string();

    let release_notes = response["body"].as_str().unwrap_or("").to_string();

    Ok(LatestRelease {
        version,
        download_url,
        release_notes,
    })
}

async fn download_and_install(url: &str) -> Result<()> {
    if url.is_empty() {
        anyhow::bail!("No download URL available for this platform");
    }

    let client = reqwest::Client::new();

    // Download to temp file
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;

    // Write to temp file
    let temp_path = "/tmp/rw-update";
    tokio::fs::write(temp_path, &bytes).await?;

    // Make executable
    crate::utils::shell::run_command("chmod", &["+x", temp_path]).await?;

    // Move to final location
    let current_exe = std::env::current_exe()?;
    let backup_path = format!("{}.bak", current_exe.display());

    // Backup current binary
    crate::utils::shell::run_command("cp", &[current_exe.to_str().unwrap(), &backup_path]).await?;

    // Replace with new binary
    crate::utils::shell::run_command("mv", &[temp_path, current_exe.to_str().unwrap()]).await?;

    Ok(())
}
