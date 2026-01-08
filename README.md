# RustWops

A high-performance CLI tool for managing web server stacks on Ubuntu. Built in Rust for speed and reliability.

## Features

- **Stack Management**: Install, update, and remove Nginx, PHP, MariaDB/MySQL, Redis, and Node.js
- **Site Management**: Create and manage static, PHP, WordPress, Node.js, and reverse proxy sites
- **SSL Certificates**: Automated Let's Encrypt certificates via acme.sh (HTTP and DNS validation)
- **Staging Environments**: Create staging copies of production sites with database cloning
- **Security Tools**: Fail2Ban, ClamAV antivirus, and MySQLTuner integration
- **Log Viewing**: Comprehensive log viewing with filtering by site, status code, IP, and more
- **Backup System**: Full site backups with optional S3 upload
- **Interactive Mode**: User-friendly TUI for all operations
- **Performance Optimized**: Tuned configurations based on WordOps best practices

## Requirements

- Ubuntu 22.04 or 24.04
- Root access
- 1GB+ RAM recommended

## Installation

### From Binary (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/mihained/rustwops/main/install.sh | sudo bash
```

### From Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/mihained/rustwops.git
cd rustwops
cargo build --release

# Install
sudo cp target/release/rw /usr/local/bin/
```

## Quick Start

### Interactive Mode

Simply run `rw` without arguments to launch the interactive menu:

```bash
sudo rw
```

### Install Full Stack

```bash
# Install all components (Nginx, PHP 8.3, MariaDB, Redis, Node.js)
sudo rw stack install --all

# Or select specific components
sudo rw stack install nginx php mysql redis
```

### Create a WordPress Site

```bash
# Basic WordPress site with SSL
sudo rw site create example.com --type wp --ssl

# WordPress with FastCGI page caching (recommended for high traffic)
sudo rw site create example.com --type wp --ssl --cache fastcgi

# WordPress with Redis object caching (reduces database load)
sudo rw site create example.com --type wp --ssl --cache redis
```

### Create a PHP Site

```bash
sudo rw site create myapp.com --type php --php 8.3
```

### Create a Static Site

```bash
sudo rw site create static.example.com --type static --ssl
```

## Commands

### Stack Management

```bash
rw stack install [--all] [components...]   # Install stack components
rw stack remove [components...] [--purge]  # Remove components
rw stack update [components...]            # Update components
rw stack status                            # Show stack status
rw stack php-versions                      # List PHP versions
rw stack php-install <version>             # Install additional PHP version
```

### Site Management

```bash
rw site create <domain> [options]          # Create a new site
rw site delete <domain> [--all]            # Delete a site
rw site list                               # List all sites
rw site info <domain>                      # Show site details
rw site cache-purge <domain>               # Purge cache for WordPress site
rw site pm2 <domain> <action>              # PM2 management for Node.js sites
```

#### Site Creation Options

| Option | Description |
|--------|-------------|
| `--type <type>` | Site type: `wp`, `php`, `static`, `node`, `proxy` |
| `--php <version>` | PHP version (e.g., `8.3`, `8.2`) |
| `--ssl` | Enable SSL certificate |
| `--wildcard` | Issue wildcard certificate |
| `--cache <type>` | Cache type: `fastcgi`, `redis` |
| `--upstream <port>` | Upstream port for proxy/node sites |

#### Cache Management (WordPress)

```bash
rw site cache-purge <domain>               # Purge all caches (page + object)
rw site cache-purge <domain> --page        # Purge page cache only (FastCGI)
rw site cache-purge <domain> --object      # Purge object cache only (Redis)
```

When using `--cache fastcgi` or `--cache redis`, RustWops automatically:
- Installs and configures the **Nginx Helper** plugin for cache purging
- Installs **Redis Object Cache** plugin for Redis caching
- Adds `X-Cache-Status` header showing HIT/MISS/BYPASS
- Configures automatic cache purging when content is edited

#### PM2 Management (Node.js Sites)

```bash
rw site pm2 <domain> start                 # Start the PM2 app
rw site pm2 <domain> stop                  # Stop the PM2 app
rw site pm2 <domain> restart               # Restart the PM2 app
rw site pm2 <domain> status                # Show detailed PM2 status
rw site pm2 <domain> logs                  # View logs in real-time
```

### SSL Certificates

```bash
rw ssl issue <domain>                      # Issue certificate (HTTP validation)
rw ssl issue <domain> --wildcard --dns cloudflare  # Wildcard via DNS
rw ssl renew [domain]                      # Renew certificates
rw ssl list                                # List certificates
```

### Staging Environments

```bash
rw staging create <domain> [--prefix staging]  # Create staging site
rw staging sync <domain> --prod-to-stage       # Sync production to staging
rw staging sync <domain> --stage-to-prod       # Promote staging to production
rw staging delete <domain>                     # Delete staging environment
rw staging info <domain>                       # Show staging info
```

### Security Tools

```bash
rw security status                         # Show security tools status
rw security mysqltuner                     # Run MySQLTuner analysis
rw security scan [--path /var/www]         # Run ClamAV scan
rw security update-definitions             # Update virus definitions
rw security fail2ban status                # Show Fail2Ban status
rw security fail2ban banned                # Show banned IPs
rw security fail2ban unban <ip>            # Unban an IP address
rw security fail2ban ban <ip> -j <jail>    # Ban an IP address
```

### Backup

```bash
rw backup create <domain>                  # Create full backup (files + DB)
rw backup create <domain> --db-only        # Backup database only
rw backup create <domain> --files-only     # Backup files only
rw backup create <domain> --name "label"   # Custom backup name
rw backup list                             # List all backups
rw backup list --detailed                  # Detailed backup info
rw backup restore <id>                     # Restore from backup ID
rw backup restore <id> --target <domain>   # Restore to different domain
rw backup delete <id>                      # Delete specific backup
rw backup delete --older-than 30           # Delete backups older than 30 days
rw backup config [options]                 # Configure backup settings
rw backup config-show                      # Show backup configuration
```

#### Backup Configuration

```bash
rw backup config --dir /path/to/backups    # Set backup directory
rw backup config --retention 30            # Keep backups for 30 days
rw backup config --s3-bucket mybucket      # Configure S3 bucket
rw backup config --s3-region us-east-1     # Configure S3 region
rw backup config --schedule "0 3 * * *"    # Schedule daily backup at 3am
```

### Log Viewing

```bash
rw log site <domain>                       # View site access/error logs
rw log site <domain> --errors              # Show only error logs
rw log site <domain> --access              # Show only access logs
rw log site <domain> --php                 # Show PHP-FPM logs for site
rw log site <domain> --follow              # Follow logs in real-time
rw log site <domain> --status 404          # Filter by HTTP status code
rw log site <domain> --ip 1.2.3.4          # Filter by IP address
rw log site                                # Summary of all sites logs
rw log nginx [--errors]                    # View global nginx logs
rw log php [version]                       # View PHP-FPM logs
rw log mysql                               # View MySQL/MariaDB logs
rw log fail2ban [--bans]                   # View Fail2Ban logs
```

### Services

```bash
rw service status                          # Show all service status
rw service start <service>                 # Start a service
rw service stop <service>                  # Stop a service
rw service restart <service>               # Restart a service
rw service reload <service>                # Reload a service
```

## Configuration

RustWops stores its configuration in `/etc/rustwops/`:

- `/etc/rustwops/config.toml` - Main configuration
- `/etc/rustwops/credentials/` - Database and service credentials
- `/var/lib/rustwops/` - SQLite database and backups
- `/var/log/rustwops/` - Log files

## Stack Components

### Nginx
- Optimized for high performance
- FastCGI caching support
- Security headers configured
- Gzip compression enabled

### PHP
- Multiple version support (7.4, 8.0, 8.1, 8.2, 8.3, 8.4)
- PHP-FPM with dynamic process management
- OPcache optimized
- Common extensions pre-installed

### MariaDB
- InnoDB buffer pool tuned to system RAM
- Secure installation automated
- Per-site database isolation

### Redis
- Memory limits based on system RAM
- Used for object caching and sessions

### Node.js
- Installed via NVM for version flexibility
- PM2 process manager included
- Systemd integration

## Security

RustWops includes several security features:

- **Fail2Ban**: Intrusion prevention with jails for SSH, Nginx, and WordPress
- **ClamAV**: Antivirus scanning with scheduled updates
- **MySQLTuner**: Database optimization recommendations
- **Secure defaults**: Strong SSL/TLS configuration, security headers

## Directory Structure

```
/var/www/<domain>/
├── prod/
│   └── public/          # Production webroot
└── staging/
    └── public/          # Staging webroot (if created)

/etc/nginx/
├── sites-available/     # Nginx site configs
├── sites-enabled/       # Enabled sites (symlinks)
└── snippets/            # Reusable config snippets

/etc/php/<version>/fpm/
└── pool.d/              # PHP-FPM pool configs
```

## Development

### Building

```bash
cargo build --release
```

### Running Tests

```bash
# Inside Docker container with stack installed
cargo test
```

### Docker Development Environment

```bash
docker-compose up -d
docker exec -it rustwops-dev bash
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## Acknowledgments

- Inspired by [WordOps](https://github.com/WordOps/WordOps) and [EasyEngine](https://github.com/EasyEngine/easyengine)
- Built with Rust and love
