// Security tools configuration for RustWops
// Fail2Ban, MySQLTuner, ClamAV

use anyhow::Result;
use crate::utils::shell;

// =============================================================================
// Fail2Ban Configuration
// =============================================================================

/// Generate Fail2Ban jail configuration
pub fn generate_fail2ban_jail_conf() -> String {
    r#"# RustWops Fail2Ban Configuration
# Based on WordOps best practices

[DEFAULT]
# Ignore localhost
ignoreip = 127.0.0.1/8 ::1

# Ban settings
bantime = 3600
findtime = 600
maxretry = 5

# Use iptables for banning
banaction = iptables-multiport

[sshd]
enabled = true
port = ssh
filter = sshd
logpath = /var/log/auth.log
maxretry = 3
bantime = 86400

[recidive]
enabled = true
logpath = /var/log/fail2ban.log
banaction = iptables-allports
bantime = 604800
findtime = 86400
maxretry = 3

[nginx-http-auth]
enabled = true
port = http,https
filter = nginx-http-auth
logpath = /var/log/nginx/*error*.log
maxretry = 5

[nginx-botsearch]
enabled = true
port = http,https
filter = nginx-botsearch
logpath = /var/log/nginx/*access*.log
maxretry = 2
bantime = 86400

[nginx-forbidden]
enabled = true
port = http,https
filter = nginx-forbidden
logpath = /var/log/nginx/*access*.log
maxretry = 5

[wordpress]
enabled = true
port = http,https
filter = wordpress
logpath = /var/log/nginx/*access*.log
maxretry = 5
bantime = 3600
"#.to_string()
}

/// Generate WordPress Fail2Ban filter
pub fn generate_wordpress_filter() -> String {
    r#"# RustWops WordPress Fail2Ban Filter
# Blocks brute force attacks on wp-login.php and xmlrpc.php

[Definition]
failregex = ^<HOST> .* "POST /wp-login\.php
            ^<HOST> .* "POST /xmlrpc\.php
            ^<HOST> .* "POST .*/wp-login\.php
            ^<HOST> .* "POST .*/xmlrpc\.php

ignoreregex =

# Notes:
# - This filter catches POST requests to WordPress login and XMLRPC endpoints
# - It works for WordPress installed in root or subdirectories
"#.to_string()
}

/// Generate nginx-forbidden filter
pub fn generate_nginx_forbidden_filter() -> String {
    r#"# RustWops Nginx Forbidden Filter
# Blocks repeated 403 forbidden errors

[Definition]
failregex = ^<HOST> .* "(GET|POST|HEAD) .* HTTP/\d\.\d" 403

ignoreregex =

# Notes:
# - Catches clients repeatedly triggering 403 errors
# - Usually indicates scanning or attack attempts
"#.to_string()
}

/// Install and configure Fail2Ban
pub async fn install_fail2ban(verbose: bool) -> Result<()> {
    use std::io::{self, Write};

    // Install Fail2Ban
    print!("  Installing Fail2Ban...");
    io::stdout().flush().ok();
    shell::run_command_with_output(
        "apt-get",
        &["install", "-y", "-qq", "fail2ban"],
        verbose,
    ).await?;
    println!(" done");

    // Create jail.local configuration
    print!("  Configuring Fail2Ban jails...");
    io::stdout().flush().ok();
    tokio::fs::write("/etc/fail2ban/jail.local", generate_fail2ban_jail_conf()).await?;
    println!(" done");

    // Create WordPress filter
    print!("  Creating WordPress filter...");
    io::stdout().flush().ok();
    tokio::fs::write("/etc/fail2ban/filter.d/wordpress.conf", generate_wordpress_filter()).await?;
    println!(" done");

    // Create nginx-forbidden filter
    print!("  Creating nginx-forbidden filter...");
    io::stdout().flush().ok();
    tokio::fs::write("/etc/fail2ban/filter.d/nginx-forbidden.conf", generate_nginx_forbidden_filter()).await?;
    println!(" done");

    // Enable and start Fail2Ban
    shell::run_command("systemctl", &["enable", "fail2ban"]).await?;
    shell::run_command("systemctl", &["restart", "fail2ban"]).await?;

    Ok(())
}

// =============================================================================
// MySQLTuner Configuration
// =============================================================================

/// Install MySQLTuner
pub async fn install_mysqltuner(verbose: bool) -> Result<()> {
    use std::io::{self, Write};

    // Download MySQLTuner
    print!("  Downloading MySQLTuner...");
    io::stdout().flush().ok();
    shell::run_command_with_output(
        "curl",
        &[
            "-sL",
            "https://raw.githubusercontent.com/major/MySQLTuner-perl/master/mysqltuner.pl",
            "-o", "/usr/local/bin/mysqltuner",
        ],
        verbose,
    ).await?;
    println!(" done");

    // Make executable
    shell::run_command("chmod", &["+x", "/usr/local/bin/mysqltuner"]).await?;

    // Install Perl if not present (required for MySQLTuner)
    print!("  Installing Perl dependencies...");
    io::stdout().flush().ok();
    shell::run_command_with_output(
        "apt-get",
        &["install", "-y", "-qq", "perl", "libdbi-perl", "libdbd-mysql-perl"],
        verbose,
    ).await?;
    println!(" done");

    Ok(())
}

// =============================================================================
// ClamAV Configuration
// =============================================================================

/// Generate ClamAV freshclam update script
pub fn generate_freshclam_script() -> String {
    r#"#!/bin/bash
# RustWops ClamAV Update Script
# Runs weekly via cron to update virus definitions

# Stop freshclam service if running
systemctl stop clamav-freshclam 2>/dev/null || true

# Update virus definitions
/usr/bin/freshclam --quiet

# Restart freshclam service
systemctl start clamav-freshclam 2>/dev/null || true

# Log the update
echo "$(date): ClamAV definitions updated" >> /var/log/rustwops/clamav-update.log
"#.to_string()
}

/// Generate ClamAV scan script
pub fn generate_clamscan_script() -> String {
    r#"#!/bin/bash
# RustWops ClamAV Scan Script
# Scans web directories for malware

LOG_FILE="/var/log/rustwops/clamav-scan.log"
QUARANTINE_DIR="/var/lib/rustwops/quarantine"
SCAN_DIR="/var/www"

# Create quarantine directory if it doesn't exist
mkdir -p "$QUARANTINE_DIR"

# Log start time
echo "$(date): Starting ClamAV scan of $SCAN_DIR" >> "$LOG_FILE"

# Run scan
# --infected: Only show infected files
# --recursive: Scan subdirectories
# --move: Move infected files to quarantine
clamscan --infected --recursive --move="$QUARANTINE_DIR" "$SCAN_DIR" >> "$LOG_FILE" 2>&1

# Log completion
echo "$(date): ClamAV scan completed" >> "$LOG_FILE"
echo "---" >> "$LOG_FILE"
"#.to_string()
}

/// Install and configure ClamAV
pub async fn install_clamav(verbose: bool) -> Result<()> {
    use std::io::{self, Write};

    // Install ClamAV
    print!("  Installing ClamAV...");
    io::stdout().flush().ok();
    shell::run_command_with_output(
        "apt-get",
        &["install", "-y", "-qq", "clamav", "clamav-daemon", "clamav-freshclam"],
        verbose,
    ).await?;
    println!(" done");

    // Stop freshclam service before updating
    let _ = shell::run_command("systemctl", &["stop", "clamav-freshclam"]).await;

    // Initial virus definition update
    print!("  Updating virus definitions (this may take a while)...");
    io::stdout().flush().ok();
    let _ = shell::run_command_with_output("freshclam", &[], verbose).await;
    println!(" done");

    // Create update script
    print!("  Creating update script...");
    io::stdout().flush().ok();
    tokio::fs::write("/opt/freshclam.sh", generate_freshclam_script()).await?;
    shell::run_command("chmod", &["+x", "/opt/freshclam.sh"]).await?;
    println!(" done");

    // Create scan script
    print!("  Creating scan script...");
    io::stdout().flush().ok();
    tokio::fs::write("/opt/clamscan.sh", generate_clamscan_script()).await?;
    shell::run_command("chmod", &["+x", "/opt/clamscan.sh"]).await?;
    println!(" done");

    // Create quarantine directory
    shell::run_command("mkdir", &["-p", "/var/lib/rustwops/quarantine"]).await?;
    shell::run_command("chmod", &["700", "/var/lib/rustwops/quarantine"]).await?;

    // Add weekly cron job for virus definition updates
    print!("  Setting up weekly update cron...");
    io::stdout().flush().ok();
    let cron_content = "# RustWops ClamAV weekly update\n0 3 * * 0 root /opt/freshclam.sh\n";
    tokio::fs::write("/etc/cron.d/rustwops-clamav", cron_content).await?;
    println!(" done");

    // Enable and start ClamAV services
    shell::run_command("systemctl", &["enable", "clamav-freshclam"]).await?;
    shell::run_command("systemctl", &["start", "clamav-freshclam"]).await?;
    shell::run_command("systemctl", &["enable", "clamav-daemon"]).await?;
    // Note: clamav-daemon may take a while to start as it loads definitions
    let _ = shell::run_command("systemctl", &["start", "clamav-daemon"]).await;

    Ok(())
}

// =============================================================================
// Install All Security Tools
// =============================================================================

/// Install all security tools (Fail2Ban, MySQLTuner, ClamAV)
pub async fn install_all_security_tools(verbose: bool) -> Result<()> {
    install_fail2ban(verbose).await?;
    install_mysqltuner(verbose).await?;
    install_clamav(verbose).await?;
    Ok(())
}
