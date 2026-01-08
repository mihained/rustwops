# Changelog

All notable changes to RustWops will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.1] - 2026-01-08

### Fixed
- Create nginx FastCGI cache directory before config test (fixes stack install failure)
- Add symlinks for node, npm, pm2 to /usr/local/bin (fixes PM2 commands not found)

## [0.6.0] - 2026-01-08

### Added
- **PM2 management for Node.js sites**
  - `rw site pm2 <domain> start` - Start the PM2 app
  - `rw site pm2 <domain> stop` - Stop the PM2 app
  - `rw site pm2 <domain> restart` - Restart the PM2 app
  - `rw site pm2 <domain> status` - Show detailed PM2 status
  - `rw site pm2 <domain> logs` - View PM2 logs in real-time
  - Available in interactive mode under Node.js site actions
- **Backup configuration command**
  - `rw backup config --dir <path>` - Set backup directory
  - `rw backup config --retention <days>` - Set retention policy
  - `rw backup config --s3-bucket <name>` - Configure S3 bucket
  - `rw backup config --s3-region <region>` - Configure S3 region
  - `rw backup config --schedule <cron>` - Set backup schedule (creates cron job)
  - `rw backup config-show` - Display current backup configuration
  - Available in interactive mode under Backup menu

### Changed
- Interactive menu now shows PM2 management option for Node.js sites
- Backup menu now includes configuration and show config options

## [0.5.0] - 2026-01-08

### Added
- **Privilege separation** - run without root, sudo prompts only when needed
  - Read-only commands (list, info, status) work without root
  - Write operations prompt for sudo automatically
  - Interactive mode works as regular user

## [0.4.0] - 2026-01-08

### Added
- **WordPress caching with Nginx Helper and Redis Object Cache plugins**
  - `rw site create --cache fastcgi` - FastCGI page caching with auto-purge
  - `rw site create --cache redis` - Redis object caching for database queries
  - Auto-installs and configures Nginx Helper plugin for cache management
  - Auto-installs and configures Redis Object Cache plugin for Redis sites
  - Cache automatically purges when posts/pages are edited in WordPress admin
  - `RT_WP_NGINX_HELPER_CACHE_PATH` constant auto-configured in wp-config.php
- **Cache purge command** for WordPress sites
  - `rw site cache-purge <domain>` - Purge all caches
  - `rw site cache-purge <domain> --page` - Purge FastCGI page cache only
  - `rw site cache-purge <domain> --object` - Purge Redis object cache only
  - `rw site cache-purge <domain> --all` - Purge both page and object cache
  - Available in interactive mode under site actions
- **Nginx FastCGI cache configuration**
  - `fastcgi_cache_path` zone configured in nginx.conf
  - Cache bypass for logged-in users, POST requests, admin pages
  - WooCommerce cart/checkout pages excluded from cache
  - `X-Cache-Status` header shows HIT/MISS/BYPASS status
  - Access logs include cache status: `[HIT]`, `[MISS]`, `[BYPASS]`
- **Backup command module** with full backup/restore functionality
  - `rw backup create <domain>` - Create compressed backup (tar.gz)
  - `rw backup create --db-only` - Backup database only
  - `rw backup create --files-only` - Backup files only
  - `rw backup create --name <label>` - Custom backup name
  - `rw backup restore <id>` - Restore from backup ID
  - `rw backup restore --target <domain>` - Restore to different domain
  - `rw backup restore --db-only/--files-only` - Selective restore
  - `rw backup list` - List all backups in table format
  - `rw backup list --detailed` - Detailed backup information
  - `rw backup delete <id>` - Delete specific backup
  - `rw backup delete --older-than <days>` - Retention policy cleanup
  - MySQL dumps with gzip compression
  - Metadata JSON with site type, PHP version, DB name
- **Log viewing command module** with comprehensive log access
  - `rw log site <domain>` - View site-specific nginx access/error logs
  - `rw log site --errors` - Show only error logs
  - `rw log site --access` - Show only access logs
  - `rw log site --php` - Show PHP-FPM logs for site
  - `rw log site --follow` - Follow logs in real-time (Ctrl+C to stop)
  - `rw log site --status <code>` - Filter by HTTP status code (e.g., 404, 500)
  - `rw log site --ip <address>` - Filter by IP address
  - `rw log site` (no domain) - Summary of all sites logs
  - `rw log nginx` - View global nginx access/error logs
  - `rw log php [version]` - View PHP-FPM logs by version
  - `rw log mysql` - View MySQL/MariaDB logs (journalctl fallback)
  - `rw log fail2ban` - View Fail2Ban logs
  - `rw log fail2ban --bans` - Show only ban/unban actions
- **Interactive menus** for Logs, Backup, and Cache Purge in TUI mode

### Changed
- Main menu now includes Logs and Backup options
- WordPress site actions include "Purge cache" for WP sites with caching
- Access log format includes cache status for sites with FastCGI cache

## [0.3.0] - 2025-01-07

### Added
- **Security command module** with full CLI and interactive support
  - `rw security status` - Show status of all security tools
  - `rw security mysqltuner` - Run MySQLTuner database analysis
  - `rw security scan` - Run ClamAV antivirus scan with quarantine option
  - `rw security update-definitions` - Update ClamAV virus definitions
  - `rw security fail2ban status` - Show Fail2Ban jail status
  - `rw security fail2ban banned` - List all banned IPs
  - `rw security fail2ban ban/unban` - Ban or unban IP addresses
  - `rw security fail2ban logs` - View recent Fail2Ban logs
- **Security tools installation** via stack command
  - Fail2Ban with 6 pre-configured jails (sshd, recidive, nginx-http-auth, nginx-botsearch, nginx-forbidden, wordpress)
  - ClamAV with freshclam service and weekly update cron
  - MySQLTuner for database optimization analysis
- **Interactive Security menu** in TUI mode
- Security tools added to Stack install menu in interactive mode

### Changed
- Main menu now includes Security option
- Stack install menu includes Fail2Ban, ClamAV, and MySQLTuner

## [0.2.0] - 2025-01-07

### Added
- **Stack optimization** based on WordOps best practices
  - Optimized nginx.conf with performance tuning
  - PHP-FPM global configuration with OPcache optimization
  - MariaDB configuration tuned to system RAM
  - Redis configuration with memory limits
  - Sysctl kernel tuning for network performance
- **Nginx security snippets** from WordOps locations.mustache
  - Security rules blocking hidden files, SQL injection, file inclusion
  - Static file caching configuration
  - FastCGI cache snippets
- **MariaDB secure installation** automation
  - Removes anonymous users
  - Disables remote root login
  - Removes test database
  - Generates and stores secure root password
- WordPress installation progress feedback
- PHP version display in interactive mode showing installed vs not installed

### Changed
- Error output no longer shows stack backtrace
- Cleaner error messages for end users

### Fixed
- PHP version selection now validates installation before site creation

## [0.1.0] - 2025-01-06

### Added
- Initial release
- **Stack management**
  - Install/remove/update Nginx, PHP, MariaDB, Redis, Node.js
  - Multiple PHP version support (7.4, 8.0, 8.1, 8.2, 8.3, 8.4)
  - PM2 process manager for Node.js
- **Site management**
  - Static sites
  - PHP sites with per-site FPM pools
  - WordPress sites with automatic installation
  - Node.js sites with PM2 integration
  - Reverse proxy sites
- **SSL certificates**
  - Let's Encrypt via acme.sh
  - HTTP-01 validation
  - DNS validation (Cloudflare, DigitalOcean, Route53)
  - Wildcard certificate support
- **Staging environments**
  - Create staging copies of production sites
  - Database cloning with URL replacement
  - Bidirectional sync (prod-to-stage, stage-to-prod)
- **Backup system**
  - Full site backups (files + database)
  - Compressed archives
  - S3 upload support
- **Interactive mode**
  - Full-featured TUI
  - Menu-driven navigation
  - All CLI features accessible
- **Service management**
  - Start/stop/restart/reload services
  - Status monitoring
- SQLite database for site tracking
- Comprehensive CLI with subcommands
