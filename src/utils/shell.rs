use anyhow::Result;
use tokio::process::Command;

/// Run a command and return its stdout
pub async fn run_command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output().await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command '{}' failed: {}", program, stderr)
    }
}

/// Run a command, optionally showing output
pub async fn run_command_with_output(
    program: &str,
    args: &[&str],
    show_output: bool,
) -> Result<String> {
    if show_output {
        let status = Command::new(program).args(args).status().await?;

        if status.success() {
            Ok(String::new())
        } else {
            anyhow::bail!(
                "Command '{}' failed with exit code {:?}",
                program,
                status.code()
            )
        }
    } else {
        run_command(program, args).await
    }
}

/// Run a shell script
pub async fn run_shell_script(script: &str, show_output: bool) -> Result<String> {
    if show_output {
        let status = Command::new("bash").arg("-c").arg(script).status().await?;

        if status.success() {
            Ok(String::new())
        } else {
            anyhow::bail!("Script failed with exit code {:?}", status.code())
        }
    } else {
        let output = Command::new("bash").arg("-c").arg(script).output().await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Script failed: {}", stderr)
        }
    }
}

/// Write content to a file
pub async fn write_file(path: &str, content: &str) -> Result<()> {
    tokio::fs::write(path, content).await?;
    Ok(())
}

/// Read content from a file
pub async fn read_file(path: &str) -> Result<String> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(content)
}

/// Check if a file exists
pub async fn file_exists(path: &str) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Check if a command exists in PATH
pub async fn command_exists(command: &str) -> bool {
    which::which(command).is_ok()
}
