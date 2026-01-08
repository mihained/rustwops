// Stack optimization configurations based on WordOps best practices
// https://github.com/WordOps/WordOps

use crate::utils::shell;
use anyhow::Result;

// =============================================================================
// Nginx Optimization
// =============================================================================

/// Generate optimized nginx.conf based on system resources
pub fn generate_nginx_conf() -> String {
    r#"# RustWops Optimized Nginx Configuration
# Based on WordOps best practices

user www-data;
worker_processes auto;
worker_cpu_affinity auto;
worker_rlimit_nofile 100000;
pid /run/nginx.pid;

# Load modules
include /etc/nginx/modules-enabled/*.conf;

events {
    worker_connections 50000;
    multi_accept on;
    use epoll;
}

http {
    ##
    # Basic Settings
    ##
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 8;
    keepalive_requests 500;
    types_hash_max_size 2048;
    server_tokens off;
    reset_timedout_connection on;

    # Timeouts
    client_body_timeout 60;
    client_header_timeout 60;
    send_timeout 60;

    # Buffer sizes
    client_body_buffer_size 128k;
    client_max_body_size 100m;
    client_header_buffer_size 1k;
    large_client_header_buffers 4 32k;

    # MIME types
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    ##
    # SSL Settings
    ##
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384';
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:50m;
    ssl_session_timeout 1d;
    ssl_session_tickets off;
    ssl_buffer_size 4k;

    # OCSP Stapling
    ssl_stapling on;
    ssl_stapling_verify on;
    resolver 8.8.8.8 8.8.4.4 1.1.1.1 1.0.0.1 valid=300s;
    resolver_timeout 5s;

    ##
    # Security Headers
    ##
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    ##
    # Logging Settings
    ##
    access_log off;
    error_log /var/log/nginx/error.log;

    # Custom log format with cache status
    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for"';

    # Log format with FastCGI cache status (HIT/MISS/BYPASS/EXPIRED)
    log_format cached '$remote_addr - $remote_user [$time_local] "$request" '
                      '$status $body_bytes_sent "$http_referer" '
                      '"$http_user_agent" [$upstream_cache_status]';

    ##
    # Gzip Settings
    ##
    gzip on;
    gzip_disable "msie6";
    gzip_vary on;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_buffers 16 8k;
    gzip_http_version 1.1;
    gzip_min_length 256;
    gzip_types
        application/atom+xml
        application/javascript
        application/json
        application/ld+json
        application/manifest+json
        application/rss+xml
        application/vnd.geo+json
        application/vnd.ms-fontobject
        application/x-font-ttf
        application/x-web-app-manifest+json
        application/xhtml+xml
        application/xml
        font/opentype
        image/bmp
        image/svg+xml
        image/x-icon
        text/cache-manifest
        text/css
        text/plain
        text/vcard
        text/vnd.rim.location.xloc
        text/vtt
        text/x-component
        text/x-cross-domain-policy
        text/xml;

    ##
    # FastCGI Settings
    ##
    fastcgi_buffers 16 16k;
    fastcgi_buffer_size 32k;
    fastcgi_read_timeout 300;
    fastcgi_send_timeout 300;
    fastcgi_connect_timeout 60;

    ##
    # FastCGI Cache
    ##
    fastcgi_cache_path /var/cache/nginx/fastcgi levels=1:2 keys_zone=WORDPRESS:100m inactive=60m max_size=1g;
    fastcgi_cache_key "$scheme$request_method$host$request_uri";
    fastcgi_cache_use_stale error timeout invalid_header updating http_500 http_503;
    fastcgi_cache_lock on;
    fastcgi_cache_lock_timeout 5s;

    ##
    # Rate Limiting
    ##
    limit_req_zone $binary_remote_addr zone=one:10m rate=1r/s;
    limit_req_zone $binary_remote_addr zone=two:10m rate=10r/s;

    ##
    # Virtual Host Configs
    ##
    include /etc/nginx/conf.d/*.conf;
    include /etc/nginx/sites-enabled/*;
}
"#.to_string()
}

/// Apply nginx configuration
pub async fn apply_nginx_config() -> Result<()> {
    let config = generate_nginx_conf();

    // Backup original config
    if tokio::fs::metadata("/etc/nginx/nginx.conf").await.is_ok() {
        let _ = shell::run_command(
            "cp",
            &["/etc/nginx/nginx.conf", "/etc/nginx/nginx.conf.backup"],
        )
        .await;
    }

    // Write new config
    tokio::fs::write("/etc/nginx/nginx.conf", config).await?;

    // Create snippets directory
    shell::run_command("mkdir", &["-p", "/etc/nginx/snippets"]).await?;

    // Write security snippets
    tokio::fs::write(
        "/etc/nginx/snippets/security.conf",
        generate_nginx_security_snippet(),
    )
    .await?;
    tokio::fs::write(
        "/etc/nginx/snippets/static-files.conf",
        generate_nginx_static_snippet(),
    )
    .await?;
    tokio::fs::write(
        "/etc/nginx/snippets/fastcgi-cache.conf",
        generate_nginx_fastcgi_cache_snippet(),
    )
    .await?;

    // Create custom default site
    apply_default_site().await?;

    // Create cache directories required by nginx config
    shell::run_command("mkdir", &["-p", "/var/cache/nginx/fastcgi"]).await?;

    // Test config
    shell::run_command("nginx", &["-t"]).await?;

    Ok(())
}

/// Apply custom default nginx site configuration
async fn apply_default_site() -> Result<()> {
    // Ensure directories exist
    shell::run_command("mkdir", &["-p", "/var/www/html"]).await?;
    shell::run_command("mkdir", &["-p", "/etc/nginx/sites-available"]).await?;
    shell::run_command("mkdir", &["-p", "/etc/nginx/sites-enabled"]).await?;

    // Write default site configuration
    tokio::fs::write(
        "/etc/nginx/sites-available/default",
        generate_default_site_config(),
    )
    .await?;

    // Write default HTML page
    tokio::fs::write("/var/www/html/index.html", generate_default_html_page()).await?;

    // Enable default site
    let _ = tokio::fs::remove_file("/etc/nginx/sites-enabled/default").await;
    tokio::fs::symlink(
        "/etc/nginx/sites-available/default",
        "/etc/nginx/sites-enabled/default",
    )
    .await?;

    Ok(())
}

/// Generate default site nginx configuration
fn generate_default_site_config() -> String {
    r#"# RustWops Default Site
# This page is shown when no site matches the request

server {
    listen 80 default_server;
    listen [::]:80 default_server;

    server_name _;

    root /var/www/html;
    index index.html;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    location / {
        try_files $uri $uri/ =404;
    }

    # Block access to hidden files
    location ~ /\. {
        deny all;
        access_log off;
        log_not_found off;
    }

    # Disable logging for favicon and robots
    location = /favicon.ico {
        log_not_found off;
        access_log off;
    }

    location = /robots.txt {
        log_not_found off;
        access_log off;
    }
}
"#
    .to_string()
}

/// Generate default HTML page
fn generate_default_html_page() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Welcome to RustWops</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #e4e4e4;
        }
        .container {
            text-align: center;
            padding: 2rem;
            max-width: 600px;
        }
        .logo {
            font-size: 4rem;
            margin-bottom: 1rem;
        }
        h1 {
            font-size: 2.5rem;
            font-weight: 700;
            margin-bottom: 0.5rem;
            background: linear-gradient(90deg, #e94560, #ff6b6b);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }
        .tagline {
            font-size: 1.1rem;
            color: #a0a0a0;
            margin-bottom: 2rem;
        }
        .status {
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 12px;
            padding: 1.5rem;
            margin-bottom: 2rem;
        }
        .status-item {
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 0.5rem;
            margin: 0.5rem 0;
        }
        .status-dot {
            width: 8px;
            height: 8px;
            background: #4ade80;
            border-radius: 50%;
            animation: pulse 2s infinite;
        }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .info {
            font-size: 0.9rem;
            color: #808080;
        }
        .info p {
            margin: 0.5rem 0;
        }
        code {
            background: rgba(255, 255, 255, 0.1);
            padding: 0.2rem 0.5rem;
            border-radius: 4px;
            font-family: 'SF Mono', Monaco, 'Courier New', monospace;
            font-size: 0.85rem;
        }
        a {
            color: #e94560;
            text-decoration: none;
        }
        a:hover {
            text-decoration: underline;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="logo">🦀</div>
        <h1>RustWops</h1>
        <p class="tagline">High-performance web server stack management</p>

        <div class="status">
            <div class="status-item">
                <span class="status-dot"></span>
                <span>Server is running</span>
            </div>
            <div class="status-item">
                <span class="status-dot"></span>
                <span>Nginx is operational</span>
            </div>
        </div>

        <div class="info">
            <p>No site is configured for this domain.</p>
            <p>Create a site with: <code>rw site create example.com</code></p>
            <p>Or launch interactive mode: <code>sudo rw</code></p>
        </div>
    </div>
</body>
</html>
"#
    .to_string()
}

/// Generate nginx security snippet (based on WordOps locations.mustache)
pub fn generate_nginx_security_snippet() -> String {
    r#"# RustWops Security Snippet
# Based on WordOps best practices
# Include with: include /etc/nginx/snippets/security.conf;

# Block access to hidden files (except .well-known)
location ~ /\.(?!well-known) {
    deny all;
    access_log off;
    log_not_found off;
}

# Block access to sensitive files
location ~* (?:\.(?:bak|conf|dist|fla|in[ci]|log|orig|psd|sh|sql|sw[op])|~)$ {
    deny all;
    access_log off;
    log_not_found off;
}

# Block access to readme, license, changelog files
location ~* (?:readme|license|changelog|contributing)(?:\.txt|\.md|\.html)?$ {
    deny all;
    access_log off;
    log_not_found off;
}

# Block access to version control
location ~ /\.(?:git|svn|hg)/ {
    deny all;
    access_log off;
    log_not_found off;
}

# Block access to archives
location ~* \.(zip|gz|tar|bz2|7z|rar)$ {
    deny all;
    access_log off;
    log_not_found off;
}

# Block access to potentially dangerous files
location ~* /(wp-config\.php|xmlrpc\.php|timthumb\.php|thumbs_editor\.php) {
    deny all;
    access_log off;
    log_not_found off;
}

# Block SQL injection attempts
location ~* "(\'|\")(.*)(drop|insert|md5|select|union)" {
    deny all;
    access_log off;
    log_not_found off;
}

# Block base64 encoded exploits
location ~* "(base64_encode|base64_decode|eval\()" {
    deny all;
    access_log off;
    log_not_found off;
}

# Block file inclusion attempts
location ~* "(php://|file://|ftp://|https?://)" {
    deny all;
    access_log off;
    log_not_found off;
}

# Block directory traversal
location ~* "(\.\.)" {
    deny all;
    access_log off;
    log_not_found off;
}

# Let's Encrypt ACME challenge
location ^~ /.well-known/acme-challenge/ {
    default_type "text/plain";
    root /var/www/html;
    allow all;
}
"#
    .to_string()
}

/// Generate nginx static files caching snippet
pub fn generate_nginx_static_snippet() -> String {
    r#"# RustWops Static Files Snippet
# Based on WordOps best practices
# Include with: include /etc/nginx/snippets/static-files.conf;

# Favicon
location = /favicon.ico {
    expires max;
    add_header Cache-Control "public, immutable";
    access_log off;
    log_not_found off;
}

# Robots.txt
location = /robots.txt {
    expires 1d;
    access_log off;
    log_not_found off;
}

# Media files (images, videos, audio)
location ~* \.(ogg|ogv|svg|svgz|eot|otf|woff|woff2|ttf|mp4|m4a|mp3|wav|flac|webm|webp|avif|gif|png|jpg|jpeg|ico|bmp|tiff|cur)$ {
    expires max;
    add_header Cache-Control "public, immutable";
    add_header Access-Control-Allow-Origin "*";
    access_log off;
    log_not_found off;
}

# CSS and JavaScript
location ~* \.(css|js)(\.map)?$ {
    expires 1y;
    add_header Cache-Control "public, immutable";
    add_header Access-Control-Allow-Origin "*";
    access_log off;
    log_not_found off;
}

# Fonts
location ~* \.(eot|ttf|woff|woff2)$ {
    expires max;
    add_header Cache-Control "public, immutable";
    add_header Access-Control-Allow-Origin "*";
    access_log off;
}

# Documents
location ~* \.(pdf|doc|docx|xls|xlsx|ppt|pptx|txt)$ {
    expires 1M;
    add_header Cache-Control "public";
    access_log off;
}
"#.to_string()
}

/// Generate nginx FastCGI cache snippet
pub fn generate_nginx_fastcgi_cache_snippet() -> String {
    r#"# RustWops FastCGI Cache Snippet
# Based on WordOps best practices
# Include with: include /etc/nginx/snippets/fastcgi-cache.conf;

# Cache path definition (put in http block or separate conf.d file)
# fastcgi_cache_path /var/cache/nginx levels=1:2 keys_zone=WORDPRESS:100m inactive=60m max_size=1g;

# Skip cache for certain requests
set $skip_cache 0;

# POST requests
if ($request_method = POST) {
    set $skip_cache 1;
}

# URLs with query strings
if ($query_string != "") {
    set $skip_cache 1;
}

# Admin and other WordPress pages
if ($request_uri ~* "/wp-admin/|/wp-login.php|/xmlrpc.php|wp-.*.php|^/feed/*|/tag/.*/feed/*|index.php|sitemap(_index)?.xml|[a-z0-9_-]+-sitemap([0-9]+)?.xml") {
    set $skip_cache 1;
}

# Logged in users or recent commenters
if ($http_cookie ~* "comment_author|wordpress_[a-f0-9]+|wp-postpass|wordpress_no_cache|wordpress_logged_in|woocommerce_cart_hash|woocommerce_items_in_cart") {
    set $skip_cache 1;
}

# WooCommerce pages
if ($request_uri ~* "/cart.*|/checkout.*|/my-account.*|/addons.*") {
    set $skip_cache 1;
}

# FastCGI cache settings (use in location ~ \.php$)
# fastcgi_cache_bypass $skip_cache;
# fastcgi_no_cache $skip_cache;
# fastcgi_cache WORDPRESS;
# fastcgi_cache_valid 200 60m;
# fastcgi_cache_valid 301 302 10m;
# fastcgi_cache_valid 404 1m;
# fastcgi_cache_methods GET HEAD;
# fastcgi_cache_lock on;
# add_header X-FastCGI-Cache $upstream_cache_status;
"#.to_string()
}

// =============================================================================
// PHP-FPM Optimization
// =============================================================================

/// Generate optimized PHP-FPM global configuration
pub fn generate_php_fpm_conf(php_version: &str) -> String {
    format!(
        r#"; RustWops Optimized PHP-FPM Configuration
; Based on WordOps best practices

[global]
pid = /run/php/php{php_version}-fpm.pid
error_log = /var/log/php{php_version}-fpm.log
log_level = notice

; Emergency restart settings
emergency_restart_threshold = 10
emergency_restart_interval = 1m
process_control_timeout = 10s

include=/etc/php/{php_version}/fpm/pool.d/*.conf
"#
    )
}

/// Generate optimized php.ini settings
pub fn generate_php_ini_optimizations() -> String {
    r#"; RustWops PHP Optimizations
; Place in /etc/php/VERSION/fpm/conf.d/99-rustwops.ini

; Security
expose_php = Off
allow_url_fopen = On
allow_url_include = Off

; Resource Limits
max_execution_time = 300
max_input_time = 300
max_input_vars = 20000
memory_limit = 256M

; Upload Limits
post_max_size = 100M
upload_max_filesize = 100M

; Error Handling
display_errors = Off
display_startup_errors = Off
log_errors = On
error_reporting = E_ALL & ~E_DEPRECATED & ~E_STRICT

; OPcache Settings
opcache.enable = 1
opcache.memory_consumption = 256
opcache.interned_strings_buffer = 32
opcache.max_accelerated_files = 10000
opcache.revalidate_freq = 5
opcache.validate_timestamps = 1
opcache.save_comments = 1
opcache.fast_shutdown = 1

; Session Settings
session.gc_maxlifetime = 1440
session.gc_probability = 1
session.gc_divisor = 1000
"#
    .to_string()
}

/// Apply PHP-FPM configuration
pub async fn apply_php_config(php_version: &str) -> Result<()> {
    // Write PHP-FPM global config
    let fpm_conf = generate_php_fpm_conf(php_version);
    let fpm_path = format!("/etc/php/{}/fpm/php-fpm.conf", php_version);

    // Backup original
    if tokio::fs::metadata(&fpm_path).await.is_ok() {
        let backup_path = format!("{}.backup", fpm_path);
        let _ = shell::run_command("cp", &[&fpm_path, &backup_path]).await;
    }

    tokio::fs::write(&fpm_path, fpm_conf).await?;

    // Write PHP.ini optimizations
    let ini_path = format!("/etc/php/{}/fpm/conf.d/99-rustwops.ini", php_version);
    tokio::fs::write(&ini_path, generate_php_ini_optimizations()).await?;

    // Also apply to CLI
    let cli_ini_path = format!("/etc/php/{}/cli/conf.d/99-rustwops.ini", php_version);
    tokio::fs::write(&cli_ini_path, generate_php_ini_optimizations()).await?;

    Ok(())
}

// =============================================================================
// MariaDB/MySQL Optimization
// =============================================================================

/// Get system RAM in MB
pub async fn get_system_ram_mb() -> Result<u64> {
    let meminfo = shell::read_file("/proc/meminfo").await?;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: u64 = parts[1].parse().unwrap_or(1048576); // Default 1GB
                return Ok(kb / 1024);
            }
        }
    }
    Ok(1024) // Default 1GB
}

/// Generate optimized MariaDB configuration based on system RAM
pub async fn generate_mariadb_conf() -> Result<String> {
    let ram_mb = get_system_ram_mb().await?;

    // Calculate InnoDB buffer pool (30% of RAM, min 128M, max 70% of RAM)
    let buffer_pool = ((ram_mb as f64 * 0.3) as u64)
        .max(128)
        .min((ram_mb as f64 * 0.7) as u64);

    // Calculate InnoDB instances (1 per GB, max 64)
    let buffer_instances = (ram_mb / 1024).clamp(1, 64);

    // Calculate log file size (25% of buffer pool)
    let log_file_size = (buffer_pool as f64 * 0.25) as u64;

    // Calculate tmp_table_size based on RAM
    let tmp_table_size = if ram_mb >= 65536 {
        256
    } else if ram_mb >= 8192 {
        128
    } else if ram_mb >= 2048 {
        64
    } else {
        32
    };

    Ok(format!(
        r#"# RustWops Optimized MariaDB Configuration
# Based on WordOps best practices
# System RAM: {} MB

[client]
port = 3306
socket = /var/run/mysqld/mysqld.sock
default-character-set = utf8mb4

[mysqld]
# Basic Settings
user = mysql
pid-file = /var/run/mysqld/mysqld.pid
socket = /var/run/mysqld/mysqld.sock
port = 3306
basedir = /usr
datadir = /var/lib/mysql
tmpdir = /tmp
lc-messages-dir = /usr/share/mysql
bind-address = 127.0.0.1

# Character Set
character-set-server = utf8mb4
collation-server = utf8mb4_unicode_ci

# Connection Settings
max_connections = 100
max_connect_errors = 100000
wait_timeout = 30
interactive_timeout = 60
thread_cache_size = 128

# Buffer Settings
sort_buffer_size = 4M
read_buffer_size = 2M
read_rnd_buffer_size = 1M
join_buffer_size = 4M

# Temp Table Settings
tmp_table_size = {tmp_table_size}M
max_heap_table_size = {tmp_table_size}M

# Query Cache (disabled for MariaDB 10.4+)
query_cache_type = 0
query_cache_size = 0

# InnoDB Settings
default_storage_engine = InnoDB
innodb_buffer_pool_size = {buffer_pool}M
innodb_buffer_pool_instances = {buffer_instances}
innodb_log_file_size = {log_file_size}M
innodb_log_buffer_size = 16M
innodb_file_per_table = 1
innodb_flush_log_at_trx_commit = 2
innodb_flush_method = O_DIRECT
innodb_io_capacity = 1000
innodb_io_capacity_max = 2000
innodb_read_io_threads = 4
innodb_write_io_threads = 4

# Transaction Isolation
transaction-isolation = READ-COMMITTED

# Binary Logging (disabled by default)
# log-bin = /var/log/mysql/mariadb-bin
# expire_logs_days = 7
# max_binlog_size = 100M

# Slow Query Log
slow_query_log = 1
slow_query_log_file = /var/log/mysql/mariadb-slow.log
long_query_time = 10

# MyISAM Settings (for system tables)
key_buffer_size = 16M
myisam_sort_buffer_size = 64M
myisam_recover_options = FORCE,BACKUP

# Disable performance schema to reduce memory usage
performance_schema = 0

# File Limits
open_files_limit = 65535
table_open_cache = 4000
table_definition_cache = 4000

[mysqldump]
quick
quote-names
max_allowed_packet = 64M

[mysql]
no-auto-rehash

[isamchk]
key_buffer = 16M
"#,
        ram_mb,
        tmp_table_size = tmp_table_size,
        buffer_pool = buffer_pool,
        buffer_instances = buffer_instances,
        log_file_size = log_file_size
    ))
}

/// Run MariaDB secure installation equivalent
pub async fn secure_mariadb_installation() -> Result<()> {
    // Generate random root password
    let root_password = crate::utils::password::generate(32);

    // SQL commands equivalent to mysql_secure_installation
    let secure_sql = format!(
        r#"
        -- Set root password
        ALTER USER 'root'@'localhost' IDENTIFIED BY '{}';

        -- Remove anonymous users
        DELETE FROM mysql.user WHERE User='';
        DELETE FROM mysql.global_priv WHERE User='';

        -- Disallow remote root login
        DELETE FROM mysql.user WHERE User='root' AND Host NOT IN ('localhost', '127.0.0.1', '::1');
        DELETE FROM mysql.global_priv WHERE User='root' AND Host NOT IN ('localhost', '127.0.0.1', '::1');

        -- Remove test database
        DROP DATABASE IF EXISTS test;
        DELETE FROM mysql.db WHERE Db='test' OR Db='test\\_%';

        -- Flush privileges
        FLUSH PRIVILEGES;
    "#,
        root_password
    );

    shell::run_command("mysql", &["-e", &secure_sql]).await?;

    // Store root password securely
    let creds_dir = "/etc/rustwops/credentials";
    shell::run_command("mkdir", &["-p", creds_dir]).await?;
    shell::run_command("chmod", &["700", creds_dir]).await?;

    let creds_content = format!("[client]\nuser=root\npassword={}\n", root_password);
    tokio::fs::write(format!("{}/mysql.cnf", creds_dir), &creds_content).await?;
    shell::run_command("chmod", &["600", &format!("{}/mysql.cnf", creds_dir)]).await?;

    // Create .my.cnf for root user
    tokio::fs::write("/root/.my.cnf", &creds_content).await?;
    shell::run_command("chmod", &["600", "/root/.my.cnf"]).await?;

    Ok(())
}

/// Apply MariaDB configuration
pub async fn apply_mariadb_config() -> Result<()> {
    let config = generate_mariadb_conf().await?;

    // Ensure directory exists
    shell::run_command("mkdir", &["-p", "/etc/mysql/mariadb.conf.d"]).await?;

    // Write config
    tokio::fs::write("/etc/mysql/mariadb.conf.d/99-rustwops.cnf", config).await?;

    Ok(())
}

// =============================================================================
// Redis Optimization
// =============================================================================

/// Generate optimized Redis configuration based on system RAM
pub async fn generate_redis_conf() -> Result<String> {
    let ram_mb = get_system_ram_mb().await?;

    // Calculate maxmemory (10% for <1GB, 20% for >=1GB)
    let maxmemory = if ram_mb < 1024 {
        (ram_mb as f64 * 0.1) as u64
    } else {
        (ram_mb as f64 * 0.2) as u64
    }
    .max(64); // Minimum 64MB

    Ok(format!(
        r#"# RustWops Optimized Redis Configuration
# Based on WordOps best practices
# System RAM: {} MB

# Network
bind 127.0.0.1 ::1
port 6379
tcp-backlog 32768
unixsocket /var/run/redis/redis-server.sock
unixsocketperm 775
timeout 0
tcp-keepalive 300

# General
daemonize yes
supervised systemd
pidfile /var/run/redis/redis-server.pid
loglevel notice
logfile /var/log/redis/redis-server.log
databases 16

# Snapshotting (persistence)
save 900 1
save 300 10
save 60 10000
stop-writes-on-bgsave-error yes
rdbcompression yes
rdbchecksum yes
dbfilename dump.rdb
dir /var/lib/redis

# Memory Management
maxmemory {}mb
maxmemory-policy allkeys-lru
maxmemory-samples 5

# Lazy Freeing
lazyfree-lazy-eviction yes
lazyfree-lazy-expire yes
lazyfree-lazy-server-del yes
replica-lazy-flush yes

# Append Only Mode (more durable, slightly slower)
appendonly no
appendfilename "appendonly.aof"
appendfsync everysec
no-appendfsync-on-rewrite no
auto-aof-rewrite-percentage 100
auto-aof-rewrite-min-size 64mb
aof-load-truncated yes

# Slow Log
slowlog-log-slower-than 10000
slowlog-max-len 128

# Client Output Buffer Limits
client-output-buffer-limit normal 0 0 0
client-output-buffer-limit replica 256mb 64mb 60
client-output-buffer-limit pubsub 32mb 8mb 60

# Security
protected-mode yes
"#,
        ram_mb, maxmemory
    ))
}

/// Apply Redis configuration
pub async fn apply_redis_config() -> Result<()> {
    let config = generate_redis_conf().await?;

    // Backup original
    if tokio::fs::metadata("/etc/redis/redis.conf").await.is_ok() {
        let _ = shell::run_command(
            "cp",
            &["/etc/redis/redis.conf", "/etc/redis/redis.conf.backup"],
        )
        .await;
    }

    tokio::fs::write("/etc/redis/redis.conf", config).await?;

    // Ensure redis user can access socket directory
    shell::run_command("mkdir", &["-p", "/var/run/redis"]).await?;
    shell::run_command("chown", &["redis:redis", "/var/run/redis"]).await?;

    // Add www-data to redis group for socket access
    let _ = shell::run_command("usermod", &["-aG", "redis", "www-data"]).await;

    Ok(())
}

// =============================================================================
// System Kernel Tuning (sysctl)
// =============================================================================

/// Generate optimized sysctl configuration
pub fn generate_sysctl_conf() -> String {
    r#"# RustWops System Tuning
# Based on WordOps best practices
# /etc/sysctl.d/99-rustwops.conf

# Kernel Security
kernel.sysrq = 0
kernel.core_uses_pid = 1
kernel.kptr_restrict = 2
kernel.dmesg_restrict = 1

# Memory
vm.swappiness = 10
vm.dirty_ratio = 60
vm.dirty_background_ratio = 5
vm.vfs_cache_pressure = 50
vm.overcommit_memory = 0

# Network Security
net.ipv4.conf.default.rp_filter = 1
net.ipv4.conf.all.rp_filter = 1
net.ipv4.tcp_syncookies = 1
net.ipv4.conf.all.accept_redirects = 0
net.ipv4.conf.default.accept_redirects = 0
net.ipv6.conf.all.accept_redirects = 0
net.ipv6.conf.default.accept_redirects = 0
net.ipv4.conf.all.send_redirects = 0
net.ipv4.conf.default.send_redirects = 0
net.ipv4.conf.all.accept_source_route = 0
net.ipv4.conf.default.accept_source_route = 0
net.ipv6.conf.all.accept_source_route = 0
net.ipv6.conf.default.accept_source_route = 0
net.ipv4.icmp_echo_ignore_broadcasts = 1
net.ipv4.icmp_ignore_bogus_error_responses = 1

# Network Performance
net.core.somaxconn = 32768
net.core.netdev_max_backlog = 32768
net.core.rmem_default = 31457280
net.core.rmem_max = 67108864
net.core.wmem_default = 31457280
net.core.wmem_max = 67108864
net.core.optmem_max = 25165824

# TCP Settings
net.ipv4.tcp_rmem = 8192 87380 33554432
net.ipv4.tcp_wmem = 8192 65536 33554432
net.ipv4.tcp_mem = 786432 1048576 26777216
net.ipv4.udp_rmem_min = 16384
net.ipv4.udp_wmem_min = 16384
net.ipv4.tcp_max_syn_backlog = 65536
net.ipv4.tcp_max_tw_buckets = 1440000
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_keepalive_time = 600
net.ipv4.tcp_keepalive_probes = 5
net.ipv4.tcp_keepalive_intvl = 15
net.ipv4.tcp_window_scaling = 1
net.ipv4.tcp_fastopen = 3
net.ipv4.tcp_mtu_probing = 1
net.ipv4.tcp_sack = 1
net.ipv4.tcp_timestamps = 1
net.ipv4.tcp_ecn = 1

# IPv4 Local Port Range
net.ipv4.ip_local_port_range = 1024 65535

# File Descriptors
fs.file-max = 2097152
fs.nr_open = 2097152
fs.inotify.max_user_watches = 524288
"#
    .to_string()
}

/// Generate file descriptor limits configuration
pub fn generate_limits_conf() -> String {
    r#"# RustWops File Descriptor Limits
# /etc/security/limits.d/99-rustwops.conf

*               soft    nofile          500000
*               hard    nofile          500000
root            soft    nofile          500000
root            hard    nofile          500000
www-data        soft    nofile          500000
www-data        hard    nofile          500000
mysql           soft    nofile          500000
mysql           hard    nofile          500000
redis           soft    nofile          500000
redis           hard    nofile          500000
"#
    .to_string()
}

/// Apply system tuning
pub async fn apply_sysctl_tuning() -> Result<()> {
    // Write sysctl config
    tokio::fs::write("/etc/sysctl.d/99-rustwops.conf", generate_sysctl_conf()).await?;

    // Write limits config
    tokio::fs::write(
        "/etc/security/limits.d/99-rustwops.conf",
        generate_limits_conf(),
    )
    .await?;

    // Apply sysctl settings
    shell::run_command("sysctl", &["-p", "/etc/sysctl.d/99-rustwops.conf"]).await?;

    Ok(())
}

// =============================================================================
// Apply All Stack Optimizations
// =============================================================================

/// Apply all stack optimizations
pub async fn apply_all_optimizations(php_version: &str, secure_mysql: bool) -> Result<()> {
    // Apply nginx optimization
    apply_nginx_config().await?;

    // Apply PHP optimization
    apply_php_config(php_version).await?;

    // Apply MariaDB configuration
    apply_mariadb_config().await?;

    // Secure MariaDB installation if requested
    if secure_mysql {
        secure_mariadb_installation().await?;
    }

    // Apply Redis configuration
    apply_redis_config().await?;

    // Apply system tuning
    apply_sysctl_tuning().await?;

    Ok(())
}
