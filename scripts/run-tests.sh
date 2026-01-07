#!/bin/bash
# Run integration tests in Docker
# Usage: ./scripts/run-tests.sh

set -e

echo "=== RustWops Integration Test Runner ==="
echo ""

# Check if we're in Docker
if [ ! -f /.dockerenv ]; then
    echo "ERROR: Tests must be run inside Docker container"
    echo "Use: docker exec rustwops-dev ./scripts/run-tests.sh"
    exit 1
fi

# Build release binary
echo "Building release binary..."
cargo build --release

# Check if stack is installed
if ! systemctl is-active --quiet nginx; then
    echo "Stack not installed. Installing..."
    ./target/release/rw stack install --all
fi

# Clean up any leftover test data
echo "Cleaning up previous test data..."
rm -f /etc/php/*/fpm/pool.d/test-*.conf /etc/php/*/fpm/pool.d/staging.*.conf 2>/dev/null || true
rm -f /etc/nginx/sites-available/test-* /etc/nginx/sites-available/staging.* 2>/dev/null || true
rm -f /etc/nginx/sites-enabled/test-* /etc/nginx/sites-enabled/staging.* 2>/dev/null || true
rm -rf /var/www/test-* 2>/dev/null || true

# Restart services to clear any bad state
echo "Restarting services..."
systemctl restart php8.3-fpm || systemctl start php8.3-fpm
systemctl reload nginx || systemctl restart nginx

# Wait for services
echo "Waiting for services..."
sleep 2

# Run tests
echo ""
echo "=== Running Tests ==="
echo ""

cargo test --test integration_tests -- --test-threads=1 --nocapture

echo ""
echo "=== All Tests Passed ==="
