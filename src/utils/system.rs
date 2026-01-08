use std::env;

/// Check if running as root
pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Get the current user
pub fn current_user() -> String {
    env::var("USER").unwrap_or_else(|_| "unknown".to_string())
}

/// Check if running on Ubuntu
pub async fn is_ubuntu() -> bool {
    if let Ok(content) = tokio::fs::read_to_string("/etc/os-release").await {
        content.contains("Ubuntu")
    } else {
        false
    }
}

/// Get Ubuntu version
pub async fn ubuntu_version() -> Option<String> {
    if let Ok(content) = tokio::fs::read_to_string("/etc/os-release").await {
        for line in content.lines() {
            if line.starts_with("VERSION_ID=") {
                return Some(
                    line.trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_string(),
                );
            }
        }
    }
    None
}

/// Check if systemd is available
pub fn has_systemd() -> bool {
    std::path::Path::new("/run/systemd/system").exists()
}

/// Require root privileges for an operation, showing helpful error if not root
pub fn require_root(operation: &str) -> anyhow::Result<()> {
    if is_root() {
        Ok(())
    } else {
        anyhow::bail!(
            "This operation requires elevated privileges.\n\n\
             To {}, run:\n  \
             sudo rw {}",
            operation,
            std::env::args().skip(1).collect::<Vec<_>>().join(" ")
        )
    }
}
