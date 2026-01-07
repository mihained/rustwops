# Changelog

All notable changes to RustWops will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
