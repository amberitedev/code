#!/usr/bin/env pwsh
# Amberite Core API Testing Script
# Tests every single API endpoint of the Amberite Core
# Usage: .\test-core-api.ps1

param(
    [string]$CorePath = "..\apps\core",
    [string]$LodestonePath = ".\test-data",
    [int]$Port = 16662,
    [int]$TimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "Continue"

# Colors for output
$Red = "`e[31m"
$Green = "`e[32m"
$Yellow = "`e[33m"
$Blue = "`e[34m"
$Reset = "`e[0m"

# Test results
$script:TestResults = @()
$script:PassedTests = 0
$script:FailedTests = 0
$script:Errors = @()

function Write-Header($text) {
    Write-Host ""
    Write-Host "$Blue========================================$Reset"
    Write-Host "$Blue$text$Reset"
    Write-Host "$Blue========================================$Reset"
    Write-Host ""
}

function Write-Success($text) {
    Write-Host "$Green✓ $text$Reset"
}

function Write-Failure($text) {
    Write-Host "$Red✗ $text$Reset"
}

function Write-Info($text) {
    Write-Host "$Yellow→ $text$Reset"
}

# Step 1: Compile the Core
Write-Header "STEP 1: Compiling Amberite Core"

$CorePath = Resolve-Path $CorePath
Write-Info "Core path: $CorePath"

Push-Location $CorePath

try {
    Write-Info "Running cargo build..."
    $compileOutput = cargo build 2>&1
    $compileExitCode = $LASTEXITCODE
    
    if ($compileExitCode -ne 0) {
        Write-Failure "Compilation failed!"
        Write-Host ""
        Write-Host "$Red=== COMPILATION ERRORS ===$Reset"
        $compileOutput | ForEach-Object { Write-Host $_ }
        exit 1
    }
    
    Write-Success "Compilation successful!"
}
finally {
    Pop-Location
}

# Step 2: Start the Core Server
Write-Header "STEP 2: Starting Amberite Core Server"

# Create test data directory if it doesn't exist
if (!(Test-Path $LodestonePath)) {
    New-Item -ItemType Directory -Path $LodestonePath | Out-Null
}

$env:LODESTONE_PATH = Resolve-Path $LodestonePath
Write-Info "Data directory: $env:LODESTONE_PATH"

# Kill any existing processes on port 16662
Write-Info "Checking for existing processes on port $Port..."
$existingProcess = Get-NetTCPConnection -LocalPort $Port -ErrorAction SilentlyContinue | Select-Object -First 1
if ($existingProcess) {
    Write-Info "Found existing process, attempting to stop..."
    Stop-Process -Id $existingProcess.OwningProcess -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

# Start the core server
Write-Info "Starting core server..."
$serverProcess = Start-Process -FilePath "cargo" -ArgumentList "run", "--", "--lodestone-path", $env:LODESTONE_PATH -WorkingDirectory $CorePath -PassThru -WindowStyle Hidden -RedirectStandardOutput "$env:TEMP\core-stdout.log" -RedirectStandardError "$env:TEMP\core-stderr.log"

Write-Info "Waiting for server to start (checking health endpoint)..."
$serverReady = $false
$startTime = Get-Date

while (!$serverReady -and ((Get-Date) - $startTime).TotalSeconds -lt $TimeoutSeconds) {
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:$Port/health" -Method GET -TimeoutSec 2 -ErrorAction SilentlyContinue
        if ($response.status -eq "healthy") {
            $serverReady = $true
            break
        }
    }
    catch {
        Start-Sleep -Milliseconds 500
    }
}

if (!$serverReady) {
    Write-Failure "Server failed to start within $TimeoutSeconds seconds"
    Write-Host "$Red=== SERVER STDOUT ===$Reset"
    Get-Content "$env:TEMP\core-stdout.log" -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_ }
    Write-Host "$Red=== SERVER STDERR ===$Reset"
    Get-Content "$env:TEMP\core-stderr.log" -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_ }
    Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

Write-Success "Server is running on port $Port"

# Step 3: Test All Endpoints
Write-Header "STEP 3: Testing API Endpoints"

# Helper function to make HTTP requests
function Test-Endpoint {
    param(
        [string]$Name,
        [string]$Method,
        [string]$Path,
        [hashtable]$Headers = @{},
        [object]$Body = $null,
        [int]$ExpectedStatus = 200,
        [scriptblock]$Validation = $null,
        [switch]$IsWebSocket = $false
    )
    
    $fullUrl = "http://localhost:$Port$Path"
    $testResult = @{
        Name = $Name
        Method = $Method
        Path = $Path
        Url = $fullUrl
        ExpectedStatus = $ExpectedStatus
        ActualStatus = $null
        Success = $false
        Error = $null
        ResponseTime = 0
    }
    
    try {
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        
        if ($IsWebSocket) {
            # WebSocket test - just check if upgrade is accepted
            $ws = New-Object System.Net.WebSockets.ClientWebSocket
            $ct = New-Object System.Threading.CancellationToken
            
            # Try to connect
            $task = $ws.ConnectAsync([System.Uri]::new($fullUrl), $ct)
            [void]$task.Wait(5000)
            
            if ($ws.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
                $testResult.ActualStatus = 101
                $ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "Test complete", $ct) | Out-Null
            }
            else {
                throw "WebSocket connection failed"
            }
        }
        else {
            $params = @{
                Uri = $fullUrl
                Method = $Method
                Headers = $Headers
                TimeoutSec = 10
                ErrorAction = "Stop"
            }
            
            if ($Body) {
                $params.Body = ($Body | ConvertTo-Json)
                $params.ContentType = "application/json"
            }
            
            $response = Invoke-RestMethod @params
            $testResult.ActualStatus = 200  # Success means 200
            $testResult.ResponseBody = $response
        }
        
        $stopwatch.Stop()
        $testResult.ResponseTime = $stopwatch.ElapsedMilliseconds
        
        # Validate
        $validationPassed = $true
        if ($Validation) {
            try {
                $validationPassed = & $Validation $response
            }
            catch {
                $validationPassed = $false
                $testResult.Error = "Validation failed: $_"
            }
        }
        
        if ($testResult.ActualStatus -eq $ExpectedStatus -and $validationPassed) {
            $testResult.Success = $true
            $script:PassedTests++
            Write-Success "$Name ($($testResult.ResponseTime)ms)"
        }
        else {
            $testResult.Success = $false
            $script:FailedTests++
            $script:Errors += $testResult
            Write-Failure "$Name - Expected $ExpectedStatus, got $($testResult.ActualStatus)"
            if ($testResult.Error) {
                Write-Host "      Error: $($testResult.Error)"
            }
        }
    }
    catch {
        $stopwatch.Stop()
        $testResult.ResponseTime = $stopwatch.ElapsedMilliseconds
        
        # Try to get status code from exception
        if ($_.Exception.Response) {
            $testResult.ActualStatus = [int]$_.Exception.Response.StatusCode
        }
        else {
            $testResult.ActualStatus = 0
        }
        
        $testResult.Success = $false
        $testResult.Error = $_.Exception.Message
        $script:FailedTests++
        $script:Errors += $testResult
        Write-Failure "$Name - $($_.Exception.Message)"
    }
    
    $script:TestResults += $testResult
}

# Test 1: Health Check
Test-Endpoint -Name "Health Check" -Method "GET" -Path "/health" -ExpectedStatus 200 -Validation {
    param($r)
    $r.status -eq "healthy"
}

# Test 2: System Stats
Test-Endpoint -Name "System Stats" -Method "GET" -Path "/stats" -ExpectedStatus 200 -Validation {
    param($r)
    $r.PSObject.Properties.Name -contains "version" -and
    $r.PSObject.Properties.Name -contains "running_instances"
}

# Test 3: Login - Invalid credentials (expect 401)
Test-Endpoint -Name "Login - Invalid Credentials" -Method "POST" -Path "/login" -Body @{
    username = "nonexistent"
    password = "wrongpassword123"
} -ExpectedStatus 401

# Test 4: Login - Validation error - Short username (expect 422)
Test-Endpoint -Name "Login - Validation Error (Short Username)" -Method "POST" -Path "/login" -Body @{
    username = "ab"
    password = "validpassword123"
} -ExpectedStatus 422

# Test 5: Login - Validation error - Short password (expect 422)
Test-Endpoint -Name "Login - Validation Error (Short Password)" -Method "POST" -Path "/login" -Body @{
    username = "validuser"
    password = "short"
} -ExpectedStatus 422

# Test 6: Setup - Validation error - Short key (expect 422)
Test-Endpoint -Name "Setup - Validation Error (Short Key)" -Method "POST" -Path "/setup" -Body @{
    key = "short"
    username = "admin"
    password = "securepassword123"
} -ExpectedStatus 422

# Test 7: Setup - Validation error - Short username (expect 422)
Test-Endpoint -Name "Setup - Validation Error (Short Username)" -Method "POST" -Path "/setup" -Body @{
    key = "this-is-a-valid-setup-key"
    username = "ab"
    password = "securepassword123"
} -ExpectedStatus 422

# Test 8: Setup - Validation error - Short password (expect 422)
Test-Endpoint -Name "Setup - Validation Error (Short Password)" -Method "POST" -Path "/setup" -Body @{
    key = "this-is-a-valid-setup-key"
    username = "admin"
    password = "short"
} -ExpectedStatus 422

# Test 9: Setup - Valid request (expect 200)
Test-Endpoint -Name "Setup - Valid Request" -Method "POST" -Path "/setup" -Body @{
    key = "this-is-a-valid-setup-key"
    username = "testadmin"
    password = "securepassword123"
} -ExpectedStatus 200 -Validation {
    param($r)
    $r.PSObject.Properties.Name -contains "token"
}

# Test 10: Login - Valid credentials (after setup)
Test-Endpoint -Name "Login - Valid Credentials" -Method "POST" -Path "/login" -Body @{
    username = "testadmin"
    password = "securepassword123"
} -ExpectedStatus 200 -Validation {
    param($r)
    $r.PSObject.Properties.Name -contains "token" -and
    $r.PSObject.Properties.Name -contains "user_id"
}

# Store token for authenticated requests
$authToken = $null
try {
    $loginResponse = Invoke-RestMethod -Uri "http://localhost:$Port/login" -Method POST -Body (@{username = "testadmin"; password = "securepassword123"} | ConvertTo-Json) -ContentType "application/json"
    $authToken = $loginResponse.token
    Write-Success "Obtained auth token for subsequent requests"
}
catch {
    Write-Failure "Failed to obtain auth token: $_"
}

# Generate a test UUID
$testUuid = [guid]::NewGuid().ToString()

# Test 11: Start Instance - Not Found (expect 404)
Test-Endpoint -Name "Start Instance - Not Found" -Method "POST" -Path "/instances/$testUuid/start" -ExpectedStatus 404

# Test 12: Stop Instance - Not Found (expect 404)
Test-Endpoint -Name "Stop Instance - Not Found" -Method "POST" -Path "/instances/$testUuid/stop" -ExpectedStatus 404

# Test 13: Kill Instance - Not Found (expect 404)
Test-Endpoint -Name "Kill Instance - Not Found" -Method "POST" -Path "/instances/$testUuid/kill" -ExpectedStatus 404

# Test 14: Send Command - Not Found (expect 404)
Test-Endpoint -Name "Send Command - Not Found" -Method "POST" -Path "/instances/$testUuid/command" -Body @{
    command = "say Hello"
} -ExpectedStatus 404

# Test 15: Get Modpack Metadata - Not Found (expect 404)
Test-Endpoint -Name "Get Modpack Metadata - Not Found" -Method "GET" -Path "/instances/$testUuid/metadata" -ExpectedStatus 404

# Test 16: Download Modpack - Not Found (expect 404)
Test-Endpoint -Name "Download Modpack - Not Found" -Method "GET" -Path "/instances/$testUuid/modpack" -ExpectedStatus 404

# Test 17: Console WebSocket - Not Found (expect 101 for WebSocket upgrade attempt, but will error)
# Note: WebSocket will attempt upgrade but fail because instance doesn't exist
Test-Endpoint -Name "Console WebSocket - Not Found" -Method "GET" -Path "/instances/$testUuid/console" -ExpectedStatus 200

# Test 18: Upload Modpack - No file (expect 400)
Test-Endpoint -Name "Upload Modpack - No File" -Method "POST" -Path "/instances" -ExpectedStatus 400

# Test 19: Upload Modpack - Invalid content type (expect 400)
Test-Endpoint -Name "Upload Modpack - Invalid Content Type" -Method "POST" -Path "/instances" -Headers @{
    "Content-Type" = "application/json"
} -Body @{
    test = "data"
} -ExpectedStatus 400

# Test 20: Invalid Path (expect 404)
Test-Endpoint -Name "Invalid Path" -Method "GET" -Path "/nonexistent/path" -ExpectedStatus 404

# Test 21: Method Not Allowed (expect 405)
Test-Endpoint -Name "Method Not Allowed (POST /health)" -Method "POST" -Path "/health" -ExpectedStatus 405

# Test 22: Method Not Allowed on instance endpoints
Test-Endpoint -Name "Method Not Allowed (GET /instances/:id/start)" -Method "GET" -Path "/instances/$testUuid/start" -ExpectedStatus 405

# Step 4: Summary
Write-Header "TEST SUMMARY"

$totalTests = $script:PassedTests + $script:FailedTests
Write-Host "Total Tests: $totalTests"
Write-Host "$GreenPassed: $script:PassedTests$Reset"
Write-Host "$RedFailed: $script:FailedTests$Reset"
Write-Host ""

if ($script:FailedTests -gt 0) {
    Write-Host "$Red=== FAILED TESTS ===$Reset"
    Write-Host ""
    
    foreach ($error in $script:Errors) {
        Write-Host "$Red--- $($error.Name) ---$Reset"
        Write-Host "  Method: $($error.Method)"
        Write-Host "  Path: $($error.Path)"
        Write-Host "  URL: $($error.Url)"
        Write-Host "  Expected Status: $($error.ExpectedStatus)"
        Write-Host "  Actual Status: $($error.ActualStatus)"
        Write-Host "  Response Time: $($error.ResponseTime)ms"
        if ($error.Error) {
            Write-Host "  Error: $($error.Error)"
        }
        Write-Host ""
    }
}

# Cleanup
Write-Header "STEP 4: Cleanup"

Write-Info "Stopping core server..."
Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue

# Clean up temp files
Remove-Item "$env:TEMP\core-stdout.log" -ErrorAction SilentlyContinue
Remove-Item "$env:TEMP\core-stderr.log" -ErrorAction SilentlyContinue

Write-Success "Cleanup complete"

# Final exit code
if ($script:FailedTests -gt 0) {
    exit 1
}
else {
    Write-Host ""
    Write-Success "All tests passed!"
    exit 0
}
