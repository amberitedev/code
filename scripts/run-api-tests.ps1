#!/usr/bin/env pwsh
# Simple API Test Runner for Amberite Core

param(
    [int]$Port = 16662,
    [string]$TestDataPath = "C:\temp\amberite-test-data"
)

$ErrorActionPreference = "Stop"

# Create test data directory
if (!(Test-Path $TestDataPath)) {
    New-Item -ItemType Directory -Path $TestDataPath -Force | Out-Null
}

# Kill any existing processes on port
Write-Host "Checking for existing processes on port $Port..."
Get-NetTCPConnection -LocalPort $Port -ErrorAction SilentlyContinue | ForEach-Object { 
    Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue 
}
Start-Sleep -Seconds 2

# Start the server
Write-Host "Starting Amberite Core server..."
$env:LODESTONE_PATH = $TestDataPath
$env:LIBCLANG_PATH = "C:\Users\ilai\scoop\apps\llvm\current\bin"
$env:Path = "C:\Users\ilai\scoop\shims;$env:Path"

$stdoutLog = Join-Path $env:TEMP "core-stdout.log"
$stderrLog = Join-Path $env:TEMP "core-stderr.log"

$serverProcess = Start-Process -FilePath "cargo" `
    -ArgumentList "run", "--", "--lodestone-path", $TestDataPath `
    -WorkingDirectory "C:\Users\ilai\amberite\apps\core" `
    -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput $stdoutLog `
    -RedirectStandardError $stderrLog

# Wait for server to start
Write-Host "Waiting for server to be ready..."
$ready = $false
$startTime = Get-Date
$timeout = 60

while (!$ready -and ((Get-Date) - $startTime).TotalSeconds -lt $timeout) {
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:$Port/health" -Method GET -TimeoutSec 2 -ErrorAction Stop
        if ($response.status -eq "healthy") {
            $ready = $true
            break
        }
    }
    catch {
        Start-Sleep -Milliseconds 500
    }
}

if (!$ready) {
    Write-Host "ERROR: Server failed to start within $timeout seconds" -ForegroundColor Red
    Write-Host "=== STDOUT ==="
    Get-Content $stdoutLog -ErrorAction SilentlyContinue | Select-Object -Last 20
    Write-Host "=== STDERR ==="
    Get-Content $stderrLog -ErrorAction SilentlyContinue | Select-Object -Last 20
    Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

Write-Host "Server is running on port $Port" -ForegroundColor Green
Write-Host ""

# Run tests
$testResults = @()
$testId = 0

function Test-Endpoint($Name, $Method, $Path, $ExpectedStatus, $Body = $null) {
    $script:testId++
    $url = "http://localhost:$Port$Path"
    $startTime = Get-Date
    
    $result = @{
        TestId = $testId
        Name = $Name
        Method = $Method
        Path = $Path
        ExpectedStatus = $ExpectedStatus
        ActualStatus = $null
        ResponseTime = 0
        Body = $null
        Error = $null
        Success = $false
    }
    
    try {
        $params = @{
            Uri = $url
            Method = $Method
            TimeoutSec = 10
            ErrorAction = "Stop"
        }
        
        if ($Body) {
            $params.Body = ($Body | ConvertTo-Json)
            $params.ContentType = "application/json"
        }
        
        $response = Invoke-RestMethod @params
        $result.ActualStatus = 200
        $result.Body = $response
        $result.Success = ($result.ActualStatus -eq $ExpectedStatus)
    }
    catch [System.Net.WebException] {
        $result.ActualStatus = [int]$_.Exception.Response.StatusCode
        try {
            $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
            $result.Body = $reader.ReadToEnd() | ConvertFrom-Json -ErrorAction SilentlyContinue
        }
        catch {}
        $result.Success = ($result.ActualStatus -eq $ExpectedStatus)
    }
    catch {
        $result.Error = $_.Exception.Message
        $result.Success = $false
    }
    
    $result.ResponseTime = ((Get-Date) - $startTime).TotalMilliseconds
    $script:testResults += $result
    
    $status = if ($result.Success) { "PASS" } else { "FAIL" }
    $color = if ($result.Success) { "Green" } else { "Red" }
    Write-Host ("[{0:D2}] {1,-45} {2,-4} {3,4:F1}ms - {4}" -f $testId, $Name, $Method, $result.ResponseTime, $status) -ForegroundColor $color
    
    if (!$result.Success -and !$result.Error) {
        Write-Host "     Expected: HTTP $ExpectedStatus, Got: HTTP $($result.ActualStatus)" -ForegroundColor Yellow
        if ($result.Body) {
            Write-Host "     Response: $($result.Body | ConvertTo-Json -Compress)" -ForegroundColor Gray
        }
    }
    elseif ($result.Error) {
        Write-Host "     Error: $($result.Error)" -ForegroundColor Red
    }
    
    return $result
}

Write-Host "=== RUNNING API TESTS ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: Health Check
Test-Endpoint "Health Check" "GET" "/health" 200

# Test 2: System Stats
Test-Endpoint "System Stats" "GET" "/stats" 200

# Test 3: Login - Invalid Credentials
Test-Endpoint "Login - Invalid Credentials" "POST" "/login" 401 @{ username = "bad"; password = "bad" }

# Test 4: Login - Short Username (validation)
Test-Endpoint "Login - Validation (Short Username)" "POST" "/login" 422 @{ username = "ab"; password = "validpassword123" }

# Test 5: Login - Short Password (validation)
Test-Endpoint "Login - Validation (Short Password)" "POST" "/login" 422 @{ username = "validuser"; password = "short" }

# Test 6: Setup - Short Key
Test-Endpoint "Setup - Validation (Short Key)" "POST" "/setup" 422 @{ key = "short"; username = "admin"; password = "securepassword123" }

# Test 7: Setup - Valid Request
Test-Endpoint "Setup - Valid Request" "POST" "/setup" 200 @{ key = "this-is-a-valid-setup-key"; username = "testadmin"; password = "securepassword123" }

# Test 8: Login - Valid Credentials
Test-Endpoint "Login - Valid Credentials" "POST" "/login" 200 @{ username = "testadmin"; password = "securepassword123" }

# Test 9: Start Instance - Not Found
$testUuid = [guid]::NewGuid().ToString()
Test-Endpoint "Start Instance - Not Found" "POST" "/instances/$testUuid/start" 404

# Test 10: Stop Instance - Not Found
Test-Endpoint "Stop Instance - Not Found" "POST" "/instances/$testUuid/stop" 404

# Test 11: Kill Instance - Not Found
Test-Endpoint "Kill Instance - Not Found" "POST" "/instances/$testUuid/kill" 404

# Test 12: Send Command - Not Found
Test-Endpoint "Send Command - Not Found" "POST" "/instances/$testUuid/command" 404 @{ command = "say Hello" }

# Test 13: Get Modpack Metadata - Not Found
Test-Endpoint "Get Modpack Metadata - Not Found" "GET" "/instances/$testUuid/metadata" 404

# Test 14: Download Modpack - Not Found
Test-Endpoint "Download Modpack - Not Found" "GET" "/instances/$testUuid/modpack" 404

# Test 15: Console WebSocket Endpoint
Test-Endpoint "Console WebSocket Endpoint" "GET" "/instances/$testUuid/console" 200

# Test 16: Upload Modpack - No File
Test-Endpoint "Upload Modpack - No File" "POST" "/instances" 400

# Test 17: Invalid Path
Test-Endpoint "Invalid Path" "GET" "/nonexistent/path" 404

# Test 18: Method Not Allowed (POST /health)
Test-Endpoint "Method Not Allowed (POST /health)" "POST" "/health" 405

# Test 19: Method Not Allowed (GET /start)
Test-Endpoint "Method Not Allowed (GET /start)" "GET" "/instances/$testUuid/start" 405

Write-Host ""
Write-Host "=== TEST SUMMARY ===" -ForegroundColor Cyan

$passed = ($testResults | Where-Object { $_.Success }).Count
$failed = ($testResults | Where-Object { !$_.Success }).Count
$total = $testResults.Count

Write-Host "Total Tests: $total"
Write-Host "Passed: $passed" -ForegroundColor Green
Write-Host "Failed: $failed" -ForegroundColor Red
Write-Host "Success Rate: $([math]::Round(($passed / $total) * 100, 2))%"

# Generate JSON report
$report = @{
    summary = @{
        total = $total
        passed = $passed
        failed = $failed
        success_rate = [math]::Round(($passed / $total) * 100, 2)
        timestamp = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    }
    tests = $testResults
    failed_tests = @($testResults | Where-Object { !$_.Success })
}

$reportPath = Join-Path $PSScriptRoot "test-report.json"
$report | ConvertTo-Json -Depth 10 | Set-Content $reportPath
Write-Host ""
Write-Host "Report saved to: $reportPath" -ForegroundColor Cyan

# Cleanup
Write-Host ""
Write-Host "Stopping server..." -ForegroundColor Yellow
Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
Remove-Item $stdoutLog -ErrorAction SilentlyContinue
Remove-Item $stderrLog -ErrorAction SilentlyContinue

if ($failed -gt 0) {
    exit 1
}
exit 0
