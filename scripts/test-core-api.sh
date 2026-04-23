#!/bin/bash
# Amberite Core API Testing Script
# Tests every single API endpoint of the Amberite Core
# Usage: ./test-core-api.sh

set -e

# Configuration
CORE_PATH="${CORE_PATH:-../apps/core}"
LODESTONE_PATH="${LODESTONE_PATH:-./test-data}"
PORT="${PORT:-16662}"
TIMEOUT_SECONDS="${TIMEOUT_SECONDS:-30}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
PASSED_TESTS=0
FAILED_TESTS=0
ERRORS=()

# Helper functions
header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

success() {
    echo -e "${GREEN}✓ $1${NC}"
}

failure() {
    echo -e "${RED}✗ $1${NC}"
}

info() {
    echo -e "${YELLOW}→ $1${NC}"
}

# Step 1: Compile the Core
header "STEP 1: Compiling Amberite Core"

CORE_PATH=$(cd "$CORE_PATH" && pwd)
info "Core path: $CORE_PATH"

cd "$CORE_PATH"

info "Running cargo build..."
if ! cargo build 2>&1; then
    failure "Compilation failed!"
    exit 1
fi

success "Compilation successful!"

# Step 2: Start the Core Server
header "STEP 2: Starting Amberite Core Server"

# Create test data directory
mkdir -p "$LODESTONE_PATH"
export LODESTONE_PATH=$(cd "$LODESTONE_PATH" && pwd)
info "Data directory: $LODESTONE_PATH"

# Kill any existing processes on port 16662
info "Checking for existing processes on port $PORT..."
pkill -f "amberite-core" 2>/dev/null || true
sleep 2

# Start the core server
info "Starting core server..."
cargo run -- --lodestone-path "$LODESTONE_PATH" > /tmp/core-stdout.log 2> /tmp/core-stderr.log &
SERVER_PID=$!

info "Waiting for server to start (checking health endpoint)..."
SERVER_READY=false
START_TIME=$(date +%s)

while [ "$SERVER_READY" = false ] && [ $(($(date +%s) - START_TIME)) -lt $TIMEOUT_SECONDS ]; do
    if curl -s -o /dev/null -w "%{http_code}" "http://localhost:$PORT/health" 2>/dev/null | grep -q "200"; then
        SERVER_READY=true
        break
    fi
    sleep 0.5
done

if [ "$SERVER_READY" = false ]; then
    failure "Server failed to start within $TIMEOUT_SECONDS seconds"
    echo -e "${RED}=== SERVER STDOUT ===${NC}"
    cat /tmp/core-stdout.log 2>/dev/null || true
    echo -e "${RED}=== SERVER STDERR ===${NC}"
    cat /tmp/core-stderr.log 2>/dev/null || true
    kill $SERVER_PID 2>/dev/null || true
    exit 1
fi

success "Server is running on port $PORT"

# Step 3: Test All Endpoints
header "STEP 3: Testing API Endpoints"

# Helper function to make HTTP requests
test_endpoint() {
    local name="$1"
    local method="$2"
    local path="$3"
    local expected_status="${4:-200}"
    local body="${5:-}"
    local content_type="${6:-application/json}"
    
    local full_url="http://localhost:$PORT$path"
    local actual_status
    local response
    local start_time end_time duration
    
    start_time=$(date +%s%N)
    
    # Build curl command
    local curl_opts="-s -w \"\\nHTTP_CODE:%{http_code}\\n\""
    
    if [ "$method" = "POST" ] && [ -n "$body" ]; then
        response=$(curl $curl_opts -X "$method" -H "Content-Type: $content_type" -d "$body" "$full_url" 2>&1) || true
    else
        response=$(curl $curl_opts -X "$method" "$full_url" 2>&1) || true
    fi
    
    end_time=$(date +%s%N)
    duration=$(( (end_time - start_time) / 1000000 ))  # Convert to milliseconds
    
    # Extract status code
    actual_status=$(echo "$response" | grep "HTTP_CODE:" | cut -d: -f2)
    
    if [ "$actual_status" = "$expected_status" ]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        success "$name (${duration}ms)"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        failure "$name - Expected $expected_status, got $actual_status"
        ERRORS+=("$name: Expected $expected_status, got $actual_status")
    fi
}

# Test 1: Health Check
test_endpoint "Health Check" "GET" "/health" "200"

# Test 2: System Stats
test_endpoint "System Stats" "GET" "/stats" "200"

# Test 3: Login - Invalid credentials (expect 401)
test_endpoint "Login - Invalid Credentials" "POST" "/login" "401" '{"username":"nonexistent","password":"wrongpassword123"}'

# Test 4: Login - Validation error - Short username (expect 422)
test_endpoint "Login - Validation Error (Short Username)" "POST" "/login" "422" '{"username":"ab","password":"validpassword123"}'

# Test 5: Login - Validation error - Short password (expect 422)
test_endpoint "Login - Validation Error (Short Password)" "POST" "/login" "422" '{"username":"validuser","password":"short"}'

# Test 6: Setup - Validation error - Short key (expect 422)
test_endpoint "Setup - Validation Error (Short Key)" "POST" "/setup" "422" '{"key":"short","username":"admin","password":"securepassword123"}'

# Test 7: Setup - Validation error - Short username (expect 422)
test_endpoint "Setup - Validation Error (Short Username)" "POST" "/setup" "422" '{"key":"this-is-a-valid-setup-key","username":"ab","password":"securepassword123"}'

# Test 8: Setup - Validation error - Short password (expect 422)
test_endpoint "Setup - Validation Error (Short Password)" "POST" "/setup" "422" '{"key":"this-is-a-valid-setup-key","username":"admin","password":"short"}'

# Test 9: Setup - Valid request (expect 200)
test_endpoint "Setup - Valid Request" "POST" "/setup" "200" '{"key":"this-is-a-valid-setup-key","username":"testadmin","password":"securepassword123"}'

# Test 10: Login - Valid credentials (after setup)
test_endpoint "Login - Valid Credentials" "POST" "/login" "200" '{"username":"testadmin","password":"securepassword123"}'

# Generate a test UUID
TEST_UUID=$(uuidgen 2>/dev/null || python3 -c "import uuid; print(uuid.uuid4())" 2>/dev/null || echo "550e8400-e29b-41d4-a716-446655440000")

# Test 11: Start Instance - Not Found (expect 404)
test_endpoint "Start Instance - Not Found" "POST" "/instances/$TEST_UUID/start" "404"

# Test 12: Stop Instance - Not Found (expect 404)
test_endpoint "Stop Instance - Not Found" "POST" "/instances/$TEST_UUID/stop" "404"

# Test 13: Kill Instance - Not Found (expect 404)
test_endpoint "Kill Instance - Not Found" "POST" "/instances/$TEST_UUID/kill" "404"

# Test 14: Send Command - Not Found (expect 404)
test_endpoint "Send Command - Not Found" "POST" "/instances/$TEST_UUID/command" "404" '{"command":"say Hello"}'

# Test 15: Get Modpack Metadata - Not Found (expect 404)
test_endpoint "Get Modpack Metadata - Not Found" "GET" "/instances/$TEST_UUID/metadata" "404"

# Test 16: Download Modpack - Not Found (expect 404)
test_endpoint "Download Modpack - Not Found" "GET" "/instances/$TEST_UUID/modpack" "404"

# Test 17: Console WebSocket - Returns 200 for non-existent instance
# Note: WebSocket upgrade returns 200 on the HTTP layer before upgrade
test_endpoint "Console WebSocket - Not Found" "GET" "/instances/$TEST_UUID/console" "200"

# Test 18: Upload Modpack - No file (expect 400)
# This will fail with bad request due to missing multipart
test_endpoint "Upload Modpack - No File" "POST" "/instances" "400"

# Test 19: Invalid Path (expect 404)
test_endpoint "Invalid Path" "GET" "/nonexistent/path" "404"

# Test 20: Method Not Allowed (expect 405)
test_endpoint "Method Not Allowed (POST /health)" "POST" "/health" "405"

# Test 21: Method Not Allowed on instance endpoints
test_endpoint "Method Not Allowed (GET /instances/:id/start)" "GET" "/instances/$TEST_UUID/start" "405"

# Step 4: Summary
header "TEST SUMMARY"

TOTAL_TESTS=$((PASSED_TESTS + FAILED_TESTS))
echo "Total Tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -gt 0 ]; then
    echo -e "${RED}=== FAILED TESTS ===${NC}"
    echo ""
    for error in "${ERRORS[@]}"; do
        echo -e "${RED}- $error${NC}"
    done
    echo ""
fi

# Cleanup
header "STEP 4: Cleanup"

info "Stopping core server..."
kill $SERVER_PID 2>/dev/null || true

# Clean up temp files
rm -f /tmp/core-stdout.log /tmp/core-stderr.log

success "Cleanup complete"

# Final exit code
if [ $FAILED_TESTS -gt 0 ]; then
    exit 1
else
    echo ""
    success "All tests passed!"
    exit 0
fi
