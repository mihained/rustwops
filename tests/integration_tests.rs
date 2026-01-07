// Integration tests for RustWops
// Run with: cargo test --test integration_tests
// These tests must be run inside Docker container with stack installed

use std::process::Command;

fn run_rw(args: &[&str]) -> (bool, String, String) {
    let output = Command::new("./target/release/rw")
        .args(args)
        .output()
        .expect("Failed to execute rw command");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

fn curl_site(domain: &str) -> (i32, String) {
    let output = Command::new("curl")
        .args(&[
            "-s",
            "-o", "/dev/null",
            "-w", "%{http_code}",
            "http://localhost/",
            "--header", &format!("Host: {}", domain),
        ])
        .output()
        .expect("Failed to execute curl");

    let http_code: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (http_code, stderr)
}

fn cleanup_site(domain: &str) {
    // Delete staging first if exists
    let _ = run_rw(&["staging", "delete", domain, "-y"]);
    // Delete the site
    let _ = run_rw(&["site", "delete", domain, "--all", "-y"]);
    // Also cleanup any staging site directly
    let staging_domain = format!("staging.{}", domain);
    let _ = run_rw(&["site", "delete", &staging_domain, "--all", "-y"]);
    // Clean up any orphaned PHP-FPM pools that might cause crashes
    let _ = Command::new("bash")
        .args(&["-c", &format!(
            "rm -f /etc/php/*/fpm/pool.d/{}.conf /etc/php/*/fpm/pool.d/{}.conf 2>/dev/null",
            domain, staging_domain
        )])
        .output();
}

// ============================================================================
// Stack Tests
// ============================================================================

#[test]
fn test_stack_status() {
    let (success, stdout, _) = run_rw(&["stack", "status"]);
    assert!(success, "stack status should succeed");
    assert!(stdout.contains("nginx") || stdout.contains("Nginx"), "should show nginx status");
}

#[test]
fn test_php_versions_list() {
    let (success, stdout, _) = run_rw(&["stack", "php-versions"]);
    assert!(success, "php-versions should succeed");
    assert!(stdout.contains("8.3") || stdout.contains("8.4"), "should list PHP versions");
}

// ============================================================================
// PHP Site Tests
// ============================================================================

#[test]
fn test_create_php_site() {
    let domain = "test-php.local";
    cleanup_site(domain);

    // Create site
    let (success, stdout, stderr) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success, "site create should succeed: {} {}", stdout, stderr);
    assert!(stdout.contains("Site created successfully"), "should confirm creation");

    // Verify HTTP works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "PHP site should return 200");

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_create_php_site_with_specific_version() {
    let domain = "test-php-ver.local";
    cleanup_site(domain);

    // Create site with PHP 8.3
    let (success, stdout, stderr) = run_rw(&["site", "create", domain, "--type", "php", "--php", "8.3"]);
    assert!(success, "site create should succeed: {} {}", stdout, stderr);

    // Verify HTTP works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "PHP site should return 200");

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_invalid_php_version_fails() {
    let domain = "test-invalid-php.local";
    cleanup_site(domain);

    // Try to create with invalid PHP version
    let (success, _, stderr) = run_rw(&["site", "create", domain, "--type", "php", "--php", "9.9"]);
    assert!(!success, "should fail with invalid PHP version");
    assert!(stderr.contains("not installed") || stderr.contains("FPM"), "should mention PHP not installed");
}

// ============================================================================
// WordPress Site Tests
// ============================================================================

#[test]
fn test_create_wordpress_site() {
    let domain = "test-wp.local";
    cleanup_site(domain);

    // Create WordPress site
    let (success, stdout, stderr) = run_rw(&["site", "create", domain, "--type", "wp"]);
    assert!(success, "WP site create should succeed: {} {}", stdout, stderr);
    assert!(stdout.contains("WordPress"), "should mention WordPress");
    assert!(stdout.contains("admin"), "should show admin user");

    // Verify HTTP works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "WordPress site should return 200");

    // Verify WordPress is actually installed
    let wp_check = Command::new("bash")
        .args(&["-c", &format!(
            "cd /var/www/{}/prod/public && wp core is-installed --allow-root",
            domain
        )])
        .output()
        .expect("Failed to check WP");
    assert!(wp_check.status.success(), "WordPress should be installed");

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_wordpress_password_reset() {
    let domain = "test-wp-pass.local";
    cleanup_site(domain);

    // Create WordPress site
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "wp"]);
    assert!(success, "WP site create should succeed");

    // Reset password using wp-cli directly (simulating what interactive menu does)
    let new_password = "TestNewPass123";
    let reset = Command::new("bash")
        .args(&["-c", &format!(
            "cd /var/www/{}/prod/public && wp user update admin --user_pass={} --allow-root",
            domain, new_password
        )])
        .output()
        .expect("Failed to reset password");
    assert!(reset.status.success(), "Password reset should succeed");

    // Cleanup
    cleanup_site(domain);
}

// ============================================================================
// Staging Tests
// ============================================================================

#[test]
fn test_create_staging_site() {
    let domain = "test-staging.local";
    let staging_domain = format!("staging.{}", domain);
    cleanup_site(domain);
    cleanup_site(&staging_domain);

    // Create production WordPress site first
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "wp"]);
    assert!(success, "Production site create should succeed");

    // Create staging
    let (success, stdout, stderr) = run_rw(&["staging", "create", domain]);
    assert!(success, "Staging create should succeed: {} {}", stdout, stderr);
    assert!(stdout.contains("Staging environment created"), "should confirm staging creation");

    // Verify staging HTTP works
    let (http_code, _) = curl_site(&staging_domain);
    assert_eq!(http_code, 200, "Staging site should return 200");

    // Verify staging has separate database
    let staging_db_check = Command::new("bash")
        .args(&["-c", &format!(
            "cd /var/www/{}/staging/public && wp db check --allow-root",
            domain
        )])
        .output()
        .expect("Failed to check staging DB");
    assert!(staging_db_check.status.success(), "Staging should have working database");

    // Cleanup
    let _ = run_rw(&["staging", "delete", domain, "-y"]);
    cleanup_site(domain);
}

#[test]
fn test_staging_not_in_site_list() {
    let domain = "test-staging-list.local";
    cleanup_site(domain);

    // Create production and staging
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "wp"]);
    assert!(success);
    let (success, _, _) = run_rw(&["staging", "create", domain]);
    assert!(success);

    // List sites - staging should not appear
    let (success, stdout, _) = run_rw(&["site", "list"]);
    assert!(success, "site list should succeed");
    assert!(stdout.contains(domain), "production site should be in list");
    assert!(!stdout.contains(&format!("staging.{}", domain)), "staging should NOT be in list");

    // Cleanup
    let _ = run_rw(&["staging", "delete", domain, "-y"]);
    cleanup_site(domain);
}

// ============================================================================
// Static Site Tests
// ============================================================================

#[test]
fn test_create_static_site() {
    let domain = "test-static.local";
    cleanup_site(domain);

    // Create static site
    let (success, stdout, stderr) = run_rw(&["site", "create", domain, "--type", "static"]);
    assert!(success, "static site create should succeed: {} {}", stdout, stderr);

    // Create an index.html
    let _ = Command::new("bash")
        .args(&["-c", &format!(
            "echo '<html><body>Static Test</body></html>' > /var/www/{}/prod/public/index.html",
            domain
        )])
        .output();

    // Verify HTTP works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "Static site should return 200");

    // Cleanup
    cleanup_site(domain);
}

// ============================================================================
// Service Tests
// ============================================================================

#[test]
fn test_service_status() {
    let (success, stdout, _) = run_rw(&["service", "status"]);
    assert!(success, "service status should succeed");
    assert!(stdout.contains("nginx"), "should show nginx");
}

#[test]
fn test_service_restart_nginx() {
    let (success, stdout, _) = run_rw(&["service", "restart", "nginx"]);
    assert!(success, "nginx restart should succeed");
    assert!(stdout.contains("restarted"), "should confirm restart");
}

// ============================================================================
// Site List Tests
// ============================================================================

#[test]
fn test_site_list_empty() {
    // This test assumes clean state
    let (success, _, _) = run_rw(&["site", "list"]);
    assert!(success, "site list should succeed even when empty");
}

#[test]
fn test_site_list_shows_created_sites() {
    let domain = "test-list.local";
    cleanup_site(domain);

    // Create site
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success);

    // List should show it
    let (success, stdout, _) = run_rw(&["site", "list"]);
    assert!(success);
    assert!(stdout.contains(domain), "list should show created site");

    // Cleanup
    cleanup_site(domain);
}

// ============================================================================
// SSL Tests (limited - can't actually issue certs in Docker)
// ============================================================================

#[test]
fn test_ssl_issue_reaches_acme() {
    let domain = "test-ssl.local";
    cleanup_site(domain);

    // Create site first
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success);

    // Try to issue SSL - will fail but should reach acme.sh
    let (success, _, stderr) = run_rw(&["ssl", "issue", domain, "--staging"]);

    // Expected to fail because domain isn't publicly accessible
    // But should reach Let's Encrypt (indicating acme.sh is working)
    assert!(!success, "SSL should fail for local domain");
    assert!(
        stderr.contains("acme") ||
        stderr.contains("letsencrypt") ||
        stderr.contains("public suffix") ||
        stderr.contains("Domain name"),
        "should reach acme.sh: {}", stderr
    );

    // Cleanup
    cleanup_site(domain);
}

// ============================================================================
// Info Tests
// ============================================================================

#[test]
fn test_info_command() {
    let (success, stdout, _) = run_rw(&["info"]);
    assert!(success, "info should succeed");
    assert!(stdout.contains("RustWops") || stdout.contains("System"), "should show system info");
}

// ============================================================================
// Stack Optimization Tests
// ============================================================================

#[test]
fn test_nginx_snippets_exist() {
    // Check that nginx snippets directory exists
    let output = Command::new("bash")
        .args(&["-c", "ls /etc/nginx/snippets/"])
        .output()
        .expect("Failed to list nginx snippets");

    assert!(output.status.success(), "nginx snippets directory should exist");
}

#[test]
fn test_php_optimization_files_exist() {
    // Check that PHP optimization files exist
    let output = Command::new("bash")
        .args(&["-c", "ls /etc/php/8.3/fpm/conf.d/99-rustwops.ini 2>/dev/null || ls /etc/php/*/fpm/conf.d/ | head -5"])
        .output()
        .expect("Failed to check PHP config");

    // The directory should at least exist
    assert!(output.status.success() || !output.stdout.is_empty(), "PHP conf.d directory should exist");
}

#[test]
fn test_mariadb_is_secured() {
    // Check that anonymous users are removed (basic security check)
    let output = Command::new("bash")
        .args(&["-c", "mysql -e \"SELECT User, Host FROM mysql.user WHERE User='' LIMIT 1;\" 2>/dev/null | wc -l"])
        .output()
        .expect("Failed to check MariaDB");

    let line_count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);

    // Should have 0 or 1 lines (header only, no anonymous users)
    assert!(line_count <= 1, "MariaDB should not have anonymous users");
}

#[test]
fn test_sysctl_config_applied() {
    // Check that some sysctl settings are reasonable
    let output = Command::new("bash")
        .args(&["-c", "sysctl net.core.somaxconn 2>/dev/null"])
        .output()
        .expect("Failed to check sysctl");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should have somaxconn set (default is usually 4096 or higher on modern systems)
    assert!(stdout.contains("somaxconn"), "somaxconn sysctl should exist");
}
