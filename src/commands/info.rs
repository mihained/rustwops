use anyhow::Result;
use colored::Colorize;

use crate::utils::shell;
use crate::Cli;

pub async fn execute(_cli: &Cli) -> Result<()> {
    println!("{} System Information:\n", "→".bright_cyan().bold());

    // OS Info
    let os_info = get_os_info().await?;
    println!("  {} Operating System:", "●".bright_cyan());
    println!("    Distribution: {}", os_info.distribution);
    println!("    Version:      {}", os_info.version);
    println!("    Kernel:       {}", os_info.kernel);
    println!("    Arch:         {}", os_info.arch);

    // Hardware
    let hw_info = get_hardware_info().await?;
    println!("\n  {} Hardware:", "●".bright_cyan());
    println!("    CPU:          {}", hw_info.cpu);
    println!("    CPU Cores:    {}", hw_info.cores);
    println!("    Memory:       {}", hw_info.memory);
    println!("    Disk:         {}", hw_info.disk);

    // Network
    let net_info = get_network_info().await?;
    println!("\n  {} Network:", "●".bright_cyan());
    println!("    Hostname:     {}", net_info.hostname);
    for ip in &net_info.ips {
        println!("    IP:           {}", ip);
    }

    // RustWops
    println!("\n  {} RustWops:", "●".bright_cyan());
    println!("    Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("    Config:       /etc/rustwops/config.toml");
    println!("    Database:     /var/lib/rustwops/sites.db");
    println!("    Logs:         /var/log/rustwops/");

    // Site summary
    if let Ok(count) = get_site_count().await {
        println!("\n  {} Sites:", "●".bright_cyan());
        println!("    Total:        {}", count.total);
        println!("    WordPress:    {}", count.wordpress);
        println!("    PHP:          {}", count.php);
        println!("    Static:       {}", count.static_sites);
        println!("    Node.js:      {}", count.node);
    }

    println!();

    Ok(())
}

struct OsInfo {
    distribution: String,
    version: String,
    kernel: String,
    arch: String,
}

async fn get_os_info() -> Result<OsInfo> {
    let distribution = shell::run_command("lsb_release", &["-is"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let version = shell::run_command("lsb_release", &["-rs"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let kernel = shell::run_command("uname", &["-r"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let arch = shell::run_command("uname", &["-m"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    Ok(OsInfo {
        distribution,
        version,
        kernel,
        arch,
    })
}

struct HardwareInfo {
    cpu: String,
    cores: String,
    memory: String,
    disk: String,
}

async fn get_hardware_info() -> Result<HardwareInfo> {
    // CPU model
    let cpu = shell::run_command("sh", &[
        "-c",
        "grep 'model name' /proc/cpuinfo | head -1 | cut -d':' -f2",
    ])
    .await
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|_| "Unknown".to_string());

    // CPU cores
    let cores = shell::run_command("nproc", &[])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    // Memory
    let memory = shell::run_command("sh", &[
        "-c",
        "free -h | grep Mem | awk '{print $2}'",
    ])
    .await
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|_| "Unknown".to_string());

    // Disk
    let disk = shell::run_command("sh", &[
        "-c",
        "df -h / | tail -1 | awk '{print $2 \" total, \" $4 \" available\"}'",
    ])
    .await
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|_| "Unknown".to_string());

    Ok(HardwareInfo {
        cpu,
        cores,
        memory,
        disk,
    })
}

struct NetworkInfo {
    hostname: String,
    ips: Vec<String>,
}

async fn get_network_info() -> Result<NetworkInfo> {
    let hostname = shell::run_command("hostname", &["-f"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let ips_output = shell::run_command("hostname", &["-I"])
        .await
        .unwrap_or_default();

    let ips: Vec<String> = ips_output
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    Ok(NetworkInfo { hostname, ips })
}

struct SiteCount {
    total: usize,
    wordpress: usize,
    php: usize,
    static_sites: usize,
    node: usize,
}

async fn get_site_count() -> Result<SiteCount> {
    let sites = crate::database::sites::list().await?;

    let wordpress = sites.iter().filter(|s| s.site_type == "wp").count();
    let php = sites.iter().filter(|s| s.site_type == "php").count();
    let static_sites = sites.iter().filter(|s| s.site_type == "static").count();
    let node = sites.iter().filter(|s| s.site_type == "node").count();

    Ok(SiteCount {
        total: sites.len(),
        wordpress,
        php,
        static_sites,
        node,
    })
}
