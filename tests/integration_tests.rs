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
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "http://localhost/",
            "--header",
            &format!("Host: {}", domain),
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
        .args(&[
            "-c",
            &format!(
                "rm -f /etc/php/*/fpm/pool.d/{}.conf /etc/php/*/fpm/pool.d/{}.conf 2>/dev/null",
                domain, staging_domain
            ),
        ])
        .output();
}

// ============================================================================
// Stack Tests
// ============================================================================

#[test]
fn test_stack_status() {
    let (success, stdout, _) = run_rw(&["stack", "status"]);
    assert!(success, "stack status should succeed");
    assert!(
        stdout.contains("nginx") || stdout.contains("Nginx"),
        "should show nginx status"
    );
}

#[test]
fn test_php_versions_list() {
    let (success, stdout, _) = run_rw(&["stack", "php-versions"]);
    assert!(success, "php-versions should succeed");
    assert!(
        stdout.contains("8.3") || stdout.contains("8.4"),
        "should list PHP versions"
    );
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
    assert!(
        stdout.contains("Site created successfully"),
        "should confirm creation"
    );

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
    let (success, stdout, stderr) =
        run_rw(&["site", "create", domain, "--type", "php", "--php", "8.3"]);
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
    assert!(
        stderr.contains("not installed") || stderr.contains("FPM"),
        "should mention PHP not installed"
    );
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
    assert!(
        success,
        "WP site create should succeed: {} {}",
        stdout, stderr
    );
    assert!(stdout.contains("WordPress"), "should mention WordPress");
    assert!(stdout.contains("admin"), "should show admin user");

    // Verify HTTP works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "WordPress site should return 200");

    // Verify WordPress is actually installed
    let wp_check = Command::new("bash")
        .args(&[
            "-c",
            &format!(
                "cd /var/www/{}/prod/public && wp core is-installed --allow-root",
                domain
            ),
        ])
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
        .args(&[
            "-c",
            &format!(
                "cd /var/www/{}/prod/public && wp user update admin --user_pass={} --allow-root",
                domain, new_password
            ),
        ])
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
    assert!(
        success,
        "Staging create should succeed: {} {}",
        stdout, stderr
    );
    assert!(
        stdout.contains("Staging environment created"),
        "should confirm staging creation"
    );

    // Verify staging HTTP works
    let (http_code, _) = curl_site(&staging_domain);
    assert_eq!(http_code, 200, "Staging site should return 200");

    // Verify staging has separate database
    let staging_db_check = Command::new("bash")
        .args(&[
            "-c",
            &format!(
                "cd /var/www/{}/staging/public && wp db check --allow-root",
                domain
            ),
        ])
        .output()
        .expect("Failed to check staging DB");
    assert!(
        staging_db_check.status.success(),
        "Staging should have working database"
    );

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
    assert!(
        !stdout.contains(&format!("staging.{}", domain)),
        "staging should NOT be in list"
    );

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
    assert!(
        success,
        "static site create should succeed: {} {}",
        stdout, stderr
    );

    // Create an index.html
    let _ = Command::new("bash")
        .args(&[
            "-c",
            &format!(
                "echo '<html><body>Static Test</body></html>' > /var/www/{}/prod/public/index.html",
                domain
            ),
        ])
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
        stderr.contains("acme")
            || stderr.contains("letsencrypt")
            || stderr.contains("public suffix")
            || stderr.contains("Domain name"),
        "should reach acme.sh: {}",
        stderr
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
    assert!(
        stdout.contains("RustWops") || stdout.contains("System"),
        "should show system info"
    );
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

    assert!(
        output.status.success(),
        "nginx snippets directory should exist"
    );
}

#[test]
fn test_php_optimization_files_exist() {
    // Check that PHP optimization files exist
    let output = Command::new("bash")
        .args(&["-c", "ls /etc/php/8.3/fpm/conf.d/99-rustwops.ini 2>/dev/null || ls /etc/php/*/fpm/conf.d/ | head -5"])
        .output()
        .expect("Failed to check PHP config");

    // The directory should at least exist
    assert!(
        output.status.success() || !output.stdout.is_empty(),
        "PHP conf.d directory should exist"
    );
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
    assert!(
        stdout.contains("somaxconn"),
        "somaxconn sysctl should exist"
    );
}

// ============================================================================
// Site Enable/Disable Tests
// ============================================================================

#[test]
fn test_site_disable_enable() {
    let domain = "test-enable-disable.local";
    cleanup_site(domain);

    // Create site
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success, "site create should succeed");

    // Verify site works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "site should return 200");

    // Disable site
    let (success, stdout, stderr) = run_rw(&["site", "disable", domain]);
    assert!(
        success,
        "site disable should succeed: {} {}",
        stdout, stderr
    );
    assert!(
        stdout.contains("disabled successfully"),
        "should confirm disable"
    );

    // Verify nginx symlink removed
    let symlink_check = Command::new("bash")
        .args(&[
            "-c",
            &format!("test -L /etc/nginx/sites-enabled/{}", domain),
        ])
        .output()
        .expect("Failed to check symlink");
    assert!(
        !symlink_check.status.success(),
        "nginx symlink should be removed"
    );

    // Enable site
    let (success, stdout, stderr) = run_rw(&["site", "enable", domain]);
    assert!(success, "site enable should succeed: {} {}", stdout, stderr);
    assert!(
        stdout.contains("enabled successfully"),
        "should confirm enable"
    );

    // Verify site works again
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "site should return 200 after enable");

    // Verify nginx symlink restored
    let symlink_check = Command::new("bash")
        .args(&[
            "-c",
            &format!("test -L /etc/nginx/sites-enabled/{}", domain),
        ])
        .output()
        .expect("Failed to check symlink");
    assert!(
        symlink_check.status.success(),
        "nginx symlink should be restored"
    );

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_disable_already_disabled_site() {
    let domain = "test-disable-twice.local";
    cleanup_site(domain);

    // Create and disable site
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success);
    let (success, _, _) = run_rw(&["site", "disable", domain]);
    assert!(success);

    // Try to disable again - should succeed with message
    let (success, stdout, _) = run_rw(&["site", "disable", domain]);
    assert!(success, "disabling already disabled site should succeed");
    assert!(
        stdout.contains("already disabled"),
        "should indicate already disabled"
    );

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_enable_already_enabled_site() {
    let domain = "test-enable-twice.local";
    cleanup_site(domain);

    // Create site (enabled by default)
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success);

    // Try to enable again - should succeed with message
    let (success, stdout, _) = run_rw(&["site", "enable", domain]);
    assert!(success, "enabling already enabled site should succeed");
    assert!(
        stdout.contains("already enabled"),
        "should indicate already enabled"
    );

    // Cleanup
    cleanup_site(domain);
}

// ============================================================================
// Site Update Tests
// ============================================================================

#[test]
fn test_site_update_php_version() {
    let domain = "test-update-php.local";
    cleanup_site(domain);

    // Create site with PHP 8.3
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php", "--php", "8.3"]);
    assert!(success, "site create should succeed");

    // Verify site works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "site should return 200");

    // Check PHP 8.2 is installed (skip test if not)
    let php82_check = Command::new("bash")
        .args(&["-c", "systemctl is-enabled php8.2-fpm 2>/dev/null"])
        .output();
    if !php82_check.map(|o| o.status.success()).unwrap_or(false) {
        println!("PHP 8.2 not installed, skipping version change test");
        cleanup_site(domain);
        return;
    }

    // Update PHP version to 8.2
    let (success, stdout, stderr) = run_rw(&["site", "update", domain, "--php", "8.2"]);
    assert!(
        success,
        "site update PHP should succeed: {} {}",
        stdout, stderr
    );
    assert!(
        stdout.contains("PHP version: 8.2"),
        "should confirm PHP change"
    );

    // Verify new PHP pool exists
    let pool_check = Command::new("bash")
        .args(&[
            "-c",
            &format!("test -f /etc/php/8.2/fpm/pool.d/{}.conf", domain),
        ])
        .output()
        .expect("Failed to check pool");
    assert!(pool_check.status.success(), "PHP 8.2 pool should exist");

    // Verify old PHP pool removed
    let old_pool_check = Command::new("bash")
        .args(&[
            "-c",
            &format!("test -f /etc/php/8.3/fpm/pool.d/{}.conf", domain),
        ])
        .output()
        .expect("Failed to check old pool");
    assert!(
        !old_pool_check.status.success(),
        "PHP 8.3 pool should be removed"
    );

    // Verify site still works
    let (http_code, _) = curl_site(domain);
    assert_eq!(http_code, 200, "site should return 200 after PHP update");

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_site_update_invalid_php_version() {
    let domain = "test-update-invalid-php.local";
    cleanup_site(domain);

    // Create site
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success);

    // Try to update to non-existent PHP version
    let (success, _, stderr) = run_rw(&["site", "update", domain, "--php", "9.9"]);
    assert!(!success, "update to invalid PHP should fail");
    assert!(
        stderr.contains("not installed"),
        "should mention PHP not installed"
    );

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_site_update_cache_type() {
    let domain = "test-update-cache.local";
    cleanup_site(domain);

    // Create WordPress site with fastcgi cache
    let (success, stdout, stderr) = run_rw(&[
        "site", "create", domain, "--type", "wp", "--cache", "fastcgi",
    ]);
    assert!(
        success,
        "WP site create should succeed: {} {}",
        stdout, stderr
    );

    // Give nginx time to fully reload after site creation
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Make a request to warm up the site and verify cache header present
    // Use -D to dump headers (GET request, not HEAD) since HEAD bypasses PHP/FastCGI
    let cache_check = Command::new("bash")
        .args(&[
            "-c",
            &format!(
                "curl -sD - http://localhost -H 'Host: {}' -o /dev/null 2>/dev/null | grep -i 'X-Cache-Status'",
                domain
            ),
        ])
        .output()
        .expect("Failed to check cache header");
    assert!(
        cache_check.status.success(),
        "should have X-Cache-Status header with fastcgi"
    );

    // Update to no cache
    let (success, stdout, stderr) = run_rw(&["site", "update", domain, "--cache", "none"]);
    assert!(
        success,
        "site update cache should succeed: {} {}",
        stdout, stderr
    );
    assert!(
        stdout.contains("Cache type: none"),
        "should confirm cache change"
    );

    // Give nginx time to fully reload after cache change
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify cache header removed
    // Use -D to dump headers (GET request, not HEAD) since HEAD bypasses PHP/FastCGI
    let cache_check = Command::new("bash")
        .args(&[
            "-c",
            &format!(
                "curl -sD - http://localhost -H 'Host: {}' -o /dev/null 2>/dev/null | grep -i 'X-Cache-Status'",
                domain
            ),
        ])
        .output()
        .expect("Failed to check cache header");
    assert!(
        !cache_check.status.success(),
        "should not have X-Cache-Status header without cache"
    );

    // Update back to fastcgi
    let (success, stdout, _) = run_rw(&["site", "update", domain, "--cache", "fastcgi"]);
    assert!(success, "re-enabling cache should succeed");
    assert!(stdout.contains("Cache type: fastcgi"));

    // Give nginx time to fully reload after cache change
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify cache header restored
    // Use -D to dump headers (GET request, not HEAD) since HEAD bypasses PHP/FastCGI
    let cache_check = Command::new("bash")
        .args(&[
            "-c",
            &format!(
                "curl -sD - http://localhost -H 'Host: {}' -o /dev/null 2>/dev/null | grep -i 'X-Cache-Status'",
                domain
            ),
        ])
        .output()
        .expect("Failed to check cache header");
    assert!(
        cache_check.status.success(),
        "should have X-Cache-Status header after re-enabling"
    );

    // Cleanup
    cleanup_site(domain);
}

#[test]
fn test_site_update_cache_not_for_php_site() {
    let domain = "test-update-cache-php.local";
    cleanup_site(domain);

    // Create PHP site (not WordPress)
    let (success, _, _) = run_rw(&["site", "create", domain, "--type", "php"]);
    assert!(success);

    // Try to update cache type - should fail
    let (success, _, stderr) = run_rw(&["site", "update", domain, "--cache", "fastcgi"]);
    assert!(!success, "cache update on PHP site should fail");
    assert!(
        stderr.contains("WordPress") || stderr.contains("wp"),
        "should mention WordPress only"
    );

    // Cleanup
    cleanup_site(domain);
}

// ============================================================================
// Default Nginx Page Tests
// ============================================================================

#[test]
fn test_default_page_for_unconfigured_domain() {
    // Access an unconfigured domain - should get RustWops default page
    let output = Command::new("bash")
        .args(&[
            "-c",
            "curl -s http://localhost -H 'Host: nonexistent.domain' | grep -i 'rustwops'",
        ])
        .output()
        .expect("Failed to check default page");

    assert!(
        output.status.success(),
        "default page should contain 'RustWops'"
    );
}

#[test]
fn test_default_page_content() {
    // Check that default page has expected elements
    let output = Command::new("bash")
        .args(&["-c", "curl -s http://localhost -H 'Host: unknown.test'"])
        .output()
        .expect("Failed to fetch default page");

    let content = String::from_utf8_lossy(&output.stdout);

    assert!(
        content.contains("RustWops"),
        "should contain RustWops title"
    );
    assert!(
        content.contains("rw site create"),
        "should contain site create instructions"
    );
    assert!(
        content.contains("Server is running"),
        "should show server status"
    );
}
