# Proposal: Running RustWops Without Root

## Current State

Currently, RustWops requires root access for all operations. This is enforced at startup:

```rust
if !cfg!(debug_assertions) && !utils::system::is_root() {
    anyhow::bail!("RustWops must be run as root for system operations");
}
```

## Problem

Requiring root for everything has several drawbacks:
1. Users must always use `sudo rw` even for read-only operations
2. Security risk of running everything as root
3. Poor UX for simple queries like `rw site list`
4. Cannot be used in environments where root is unavailable but sudo is configured

## Proposed Solution

### Privilege Escalation Model

Implement a privilege escalation model where:
1. RustWops runs as the current user by default
2. Operations requiring elevated privileges use `sudo` internally
3. Users can run read-only commands without any special permissions

### Command Classification

#### No Privileges Required (User Mode)
These commands only read from the SQLite database or display information:
- `rw site list` - List sites from database
- `rw site info <domain>` - Show site details from database
- `rw stack status` - Check service status (uses `systemctl status`)
- `rw security status` - Check security tools status
- `rw security fail2ban banned` - List banned IPs
- `rw backup list` - List backups from database
- `rw info` - Show system information
- `rw --version` / `rw --help`
- Interactive mode (for navigation only)

#### Sudo Required (Elevated Mode)
These commands modify system state:

**Package Management:**
- `rw stack install/remove/update` - apt-get operations

**Service Management:**
- `rw service start/stop/restart/reload` - systemctl operations

**Site Management:**
- `rw site create` - Creates directories, nginx configs, PHP pools
- `rw site delete` - Removes files, configs, databases

**SSL:**
- `rw ssl issue/renew` - Writes to /etc/ssl, reloads nginx

**Staging:**
- `rw staging create/sync/delete` - File operations, database operations

**Security:**
- `rw security scan --quarantine` - Moves files
- `rw security update-definitions` - Updates ClamAV
- `rw security fail2ban ban/unban` - Fail2Ban operations
- `rw security mysqltuner` - Database access

**Backup:**
- `rw backup create` - Reads site files, database dump
- `rw backup restore` - Writes site files, database restore

### Implementation Strategy

#### Phase 1: Privilege Detection

```rust
pub enum PrivilegeLevel {
    User,       // Running as regular user
    Sudo,       // Running with sudo
    Root,       // Running as root
}

pub fn get_privilege_level() -> PrivilegeLevel {
    if utils::system::is_root() {
        if std::env::var("SUDO_USER").is_ok() {
            PrivilegeLevel::Sudo
        } else {
            PrivilegeLevel::Root
        }
    } else {
        PrivilegeLevel::User
    }
}
```

#### Phase 2: Command Requirements

Add a trait for commands to declare their privilege requirements:

```rust
pub trait CommandRequirements {
    fn requires_root(&self) -> bool;
    fn privilege_operations(&self) -> Vec<&str>; // For error messages
}
```

#### Phase 3: Sudo Wrapper

Create a helper to run commands with sudo:

```rust
pub async fn run_privileged(cmd: &str, args: &[&str]) -> Result<String> {
    if is_root() {
        // Already root, run directly
        run_command(cmd, args).await
    } else {
        // Escalate with sudo
        let mut sudo_args = vec![cmd];
        sudo_args.extend(args);
        run_command("sudo", &sudo_args).await
    }
}
```

#### Phase 4: Database Access

The SQLite database should be readable by all users:

```rust
// During initialization
let db_path = "/var/lib/rustwops/rustwops.db";
// Set permissions to 644 (owner rw, group/others r)
std::fs::set_permissions(db_path, Permissions::from_mode(0o644))?;
```

For write operations, escalate:

```rust
pub async fn db_write_privileged<F>(operation: F) -> Result<()>
where
    F: FnOnce(&Connection) -> Result<()>,
{
    if is_root() {
        let conn = open_db()?;
        operation(&conn)
    } else {
        // Use a privileged helper script
        run_privileged("rw-db-helper", &["write", ...])?
    }
}
```

### User Experience

#### Before (Current)
```bash
$ rw site list
Error: RustWops must be run as root for system operations

$ sudo rw site list
→ Sites (3):
...
```

#### After (Proposed)
```bash
$ rw site list
→ Sites (3):
+---------------+--------+-----+-----+-----------+
| Domain        | Type   | PHP | SSL | Status    |
+---------------+--------+-----+-----+-----------+
| example.com   | wp     | 8.3 | ✓   | ● enabled |
...

$ rw site create newsite.com --type php
→ This operation requires elevated privileges.
[sudo] password for user:
→ Creating site: newsite.com
...

# Or user can pre-authorize:
$ sudo rw site create newsite.com --type php
→ Creating site: newsite.com
...
```

### Interactive Mode

In interactive mode:
1. Start without requiring root
2. When user selects an action requiring privileges, prompt for sudo
3. Cache sudo credentials for the session (sudo's default behavior)

```rust
async fn execute_with_privilege_check<F, T>(
    operation_name: &str,
    operation: F,
) -> Result<T>
where
    F: FnOnce() -> Future<Output = Result<T>>,
{
    if !is_root() && operation_requires_root(operation_name) {
        println!("→ This operation requires elevated privileges.");

        // Check if we can sudo
        if !can_sudo().await {
            return Err(anyhow!("Cannot obtain elevated privileges"));
        }

        // Re-execute with sudo
        let current_exe = std::env::current_exe()?;
        let args: Vec<String> = std::env::args().collect();
        run_command("sudo", &[current_exe, ...args]).await?;
    } else {
        operation().await
    }
}
```

### File Permissions

Ensure these paths are readable by all users:
- `/var/lib/rustwops/rustwops.db` (mode 644)
- `/etc/rustwops/config.toml` (mode 644)

These should remain root-only:
- `/etc/rustwops/credentials/` (mode 700)
- `/var/lib/rustwops/backups/` (mode 700)

### Sudoers Configuration (Optional)

For environments where password-less sudo is desired for specific operations:

```sudoers
# /etc/sudoers.d/rustwops
%rustwops ALL=(root) NOPASSWD: /usr/local/bin/rw stack *
%rustwops ALL=(root) NOPASSWD: /usr/local/bin/rw site *
%rustwops ALL=(root) NOPASSWD: /usr/local/bin/rw service *
```

### Migration Path

1. **v0.4.0**: Implement privilege detection, allow read-only operations without root
2. **v0.5.0**: Add sudo wrapper for privileged operations
3. **v0.6.0**: Full non-root support with automatic privilege escalation

### Security Considerations

1. **Database integrity**: Ensure non-root users cannot corrupt the database
2. **Credential protection**: Keep credentials in root-only directories
3. **Audit logging**: Log all privileged operations
4. **Input validation**: Validate all inputs before passing to sudo commands

### Risks

1. **Complexity**: More complex privilege management code
2. **Testing**: Need to test both root and non-root paths
3. **Sudo availability**: Some environments may not have sudo configured
4. **Session management**: Sudo credential caching may timeout during long operations

### Alternatives Considered

1. **setuid binary**: Security concerns, not recommended
2. **Capabilities**: Complex to manage, limited to specific operations
3. **PolicyKit**: Overkill for this use case, desktop-focused
4. **Separate daemon**: Too complex, requires additional process management

### Conclusion

The sudo-based privilege escalation model provides the best balance of:
- Security (least privilege principle)
- Usability (seamless privilege escalation)
- Compatibility (works on standard Ubuntu setups)
- Simplicity (uses existing sudo infrastructure)

Implementation should be phased to minimize disruption and allow thorough testing.
