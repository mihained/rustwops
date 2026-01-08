use anyhow::Result;

use crate::commands::site::{CacheType, SiteType};

const SITES_AVAILABLE: &str = "/etc/nginx/sites-available";

pub async fn create_site_config(
    domain: &str,
    site_type: SiteType,
    php_version: &str,
    cache: Option<CacheType>,
    webroot: &str,
    upstream: Option<u16>,
) -> Result<()> {
    // Ensure nginx directories exist
    tokio::fs::create_dir_all(SITES_AVAILABLE).await?;
    tokio::fs::create_dir_all("/etc/nginx/sites-enabled").await?;
    tokio::fs::create_dir_all("/var/log/nginx").await?;

    let config = match site_type {
        SiteType::Wp => generate_wordpress_config(domain, php_version, cache, webroot),
        SiteType::Php => generate_php_config(domain, php_version, webroot),
        SiteType::Static => generate_static_config(domain, webroot),
        SiteType::Proxy => generate_proxy_config(domain, upstream.unwrap_or(3000)),
        SiteType::Node => generate_node_config(domain, upstream.unwrap_or(3000)),
    };

    let config_path = format!("{}/{}", SITES_AVAILABLE, domain);
    tokio::fs::write(&config_path, config).await?;

    Ok(())
}

fn generate_wordpress_config(
    domain: &str,
    php_version: &str,
    cache: Option<CacheType>,
    webroot: &str,
) -> String {
    let (cache_config, php_cache_directives, log_format) = match cache {
        Some(CacheType::Fastcgi) => (
            fastcgi_cache_config(),
            fastcgi_php_cache_directives(),
            "cached", // Use log format with cache status
        ),
        Some(CacheType::Redis) => ("".to_string(), "".to_string(), "main"),
        _ => ("".to_string(), "".to_string(), "main"),
    };

    format!(
        r#"# RustWops managed - {domain}
# Type: WordPress
# Cache: {cache_type}

server {{
    listen 80;
    listen [::]:80;
    server_name {domain};

    root {webroot};
    index index.php index.html;

    access_log /var/log/nginx/{domain}.access.log {log_format};
    error_log /var/log/nginx/{domain}.error.log;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

{cache_config}

    location / {{
        try_files $uri $uri/ /index.php?$args;
    }}

    location ~ \.php$ {{
        try_files $uri =404;
        fastcgi_split_path_info ^(.+\.php)(/.+)$;
        fastcgi_pass unix:/run/php/php{php_version}-fpm-{domain}.sock;
        fastcgi_index index.php;
        include fastcgi_params;
        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
        fastcgi_param PATH_INFO $fastcgi_path_info;
{php_cache_directives}
    }}

    # WordPress security
    location ~ /\. {{ deny all; }}
    location ~* /(?:uploads|files)/.*\.php$ {{ deny all; }}
    location ~* ^/wp-content/.*\.(txt|md|exe|sh|bak|inc|pot|po|mo|log|sql)$ {{ deny all; }}
    location ~* /xmlrpc\.php$ {{ deny all; }}

    # Static files
    location ~* \.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2|ttf|eot)$ {{
        expires 1y;
        add_header Cache-Control "public, immutable";
        log_not_found off;
    }}

    # Gzip
    gzip on;
    gzip_vary on;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_types text/plain text/css text/xml application/json application/javascript application/rss+xml application/atom+xml image/svg+xml;
}}
"#,
        cache_type = match cache {
            Some(CacheType::Fastcgi) => "FastCGI",
            Some(CacheType::Redis) => "Redis",
            _ => "None",
        }
    )
}

fn generate_php_config(domain: &str, php_version: &str, webroot: &str) -> String {
    format!(
        r#"# RustWops managed - {domain}
# Type: PHP

server {{
    listen 80;
    listen [::]:80;
    server_name {domain};

    root {webroot};
    index index.php index.html;

    access_log /var/log/nginx/{domain}.access.log;
    error_log /var/log/nginx/{domain}.error.log;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;

    location / {{
        try_files $uri $uri/ /index.php?$query_string;
    }}

    location ~ \.php$ {{
        try_files $uri =404;
        fastcgi_split_path_info ^(.+\.php)(/.+)$;
        fastcgi_pass unix:/run/php/php{php_version}-fpm-{domain}.sock;
        fastcgi_index index.php;
        include fastcgi_params;
        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
        fastcgi_param PATH_INFO $fastcgi_path_info;
    }}

    location ~ /\. {{ deny all; }}

    # Static files
    location ~* \.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2|ttf|eot)$ {{
        expires 1y;
        add_header Cache-Control "public, immutable";
        log_not_found off;
    }}

    gzip on;
    gzip_vary on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml;
}}
"#
    )
}

fn generate_static_config(domain: &str, webroot: &str) -> String {
    format!(
        r#"# RustWops managed - {domain}
# Type: Static

server {{
    listen 80;
    listen [::]:80;
    server_name {domain};

    root {webroot};
    index index.html index.htm;

    access_log /var/log/nginx/{domain}.access.log;
    error_log /var/log/nginx/{domain}.error.log;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;

    location / {{
        try_files $uri $uri/ =404;
    }}

    location ~ /\. {{ deny all; }}

    # Static files caching
    location ~* \.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2|ttf|eot|html)$ {{
        expires 1y;
        add_header Cache-Control "public, immutable";
    }}

    gzip on;
    gzip_vary on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml;
}}
"#
    )
}

fn generate_proxy_config(domain: &str, upstream_port: u16) -> String {
    format!(
        r#"# RustWops managed - {domain}
# Type: Reverse Proxy

upstream {domain_underscore} {{
    server 127.0.0.1:{upstream_port};
    keepalive 64;
}}

server {{
    listen 80;
    listen [::]:80;
    server_name {domain};

    access_log /var/log/nginx/{domain}.access.log;
    error_log /var/log/nginx/{domain}.error.log;

    location / {{
        proxy_pass http://{domain_underscore};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }}
}}
"#,
        domain_underscore = domain.replace(['.', '-'], "_")
    )
}

fn generate_node_config(domain: &str, upstream_port: u16) -> String {
    format!(
        r#"# RustWops managed - {domain}
# Type: Node.js with PM2

upstream {domain_underscore} {{
    server 127.0.0.1:{upstream_port};
    keepalive 64;
}}

server {{
    listen 80;
    listen [::]:80;
    server_name {domain};

    access_log /var/log/nginx/{domain}.access.log;
    error_log /var/log/nginx/{domain}.error.log;

    location / {{
        proxy_pass http://{domain_underscore};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }}

    # Static files (if any)
    location /static/ {{
        alias /var/www/{domain}/prod/public/;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }}
}}
"#,
        domain_underscore = domain.replace(['.', '-'], "_")
    )
}

fn fastcgi_cache_config() -> String {
    r#"
    # FastCGI Cache - Skip conditions
    set $skip_cache 0;

    # POST requests - don't cache
    if ($request_method = POST) { set $skip_cache 1; }

    # URLs with query strings - don't cache
    if ($query_string != "") { set $skip_cache 1; }

    # Admin and other WordPress pages - don't cache
    if ($request_uri ~* "/wp-admin/|/wp-login.php|/xmlrpc.php|wp-.*.php|^/feed/*|/tag/.*/feed/*|index.php|sitemap(_index)?.xml") {
        set $skip_cache 1;
    }

    # Logged in users or recent commenters - don't cache
    if ($http_cookie ~* "comment_author|wordpress_[a-f0-9]+|wp-postpass|wordpress_no_cache|wordpress_logged_in|woocommerce_cart_hash|woocommerce_items_in_cart") {
        set $skip_cache 1;
    }

    # WooCommerce pages - don't cache
    if ($request_uri ~* "/cart.*|/checkout.*|/my-account.*") {
        set $skip_cache 1;
    }
"#
    .to_string()
}

fn fastcgi_php_cache_directives() -> String {
    r#"
        # FastCGI Cache directives
        fastcgi_cache_bypass $skip_cache;
        fastcgi_no_cache $skip_cache;
        fastcgi_cache WORDPRESS;
        fastcgi_cache_valid 200 60m;
        fastcgi_cache_valid 301 302 10m;
        fastcgi_cache_valid 404 1m;
        fastcgi_cache_methods GET HEAD;

        # Add cache status header (HIT/MISS/BYPASS/EXPIRED)
        add_header X-FastCGI-Cache $upstream_cache_status always;
        add_header X-Cache-Status $upstream_cache_status always;"#
        .to_string()
}

/// Add SSL configuration to existing site
pub async fn add_ssl_config(domain: &str, cert_path: &str, key_path: &str) -> Result<()> {
    let config_path = format!("{}/{}", SITES_AVAILABLE, domain);
    let content = tokio::fs::read_to_string(&config_path).await?;

    // Replace listen directives and add SSL config
    let ssl_config = format!(
        r#"
    listen 443 ssl http2;
    listen [::]:443 ssl http2;

    ssl_certificate {cert_path};
    ssl_certificate_key {key_path};

    # SSL settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;
    ssl_session_tickets off;

    # HSTS
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
"#
    );

    // Add HTTP to HTTPS redirect
    let redirect_block = format!(
        r#"
# HTTP redirect
server {{
    listen 80;
    listen [::]:80;
    server_name {domain};
    return 301 https://$host$request_uri;
}}
"#
    );

    // Replace the listen 80 lines with SSL config
    let new_content = content
        .replace("listen 80;", &ssl_config)
        .replace("listen [::]:80;", "");

    let final_content = format!("{}\n\n{}", redirect_block, new_content);

    tokio::fs::write(&config_path, final_content).await?;

    Ok(())
}
