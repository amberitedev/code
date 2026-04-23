#!/usr/bin/env pwsh
# Amberite Core API Testing Script - Comprehensive Version
# Tests every API endpoint with detailed debug output for AI analysis
# Usage: .\test-core-api-comprehensive.ps1

param(
    [string]$CorePath = "..\apps\core",
    [string]$LodestonePath = ".\test-data",
    [int]$Port = 16662,
    [int]$TimeoutSeconds = 30,
    [string]$OutputFormat = "human"  # "human" or "json"
)

$ErrorActionPreference = "Stop"

# Colors
$Red = "`e[91m"
$Green = "`e[92m"
$Yellow = "`e[93m"
$Blue = "`e[94m"
$Cyan = "`e[96m"
$Magenta = "`e[95m"
$Reset = "`e[0m"

# Results collection
$script:TestResults = @()
$script:ServerLogs = @{ Stdout = @(); Stderr = @() }
$script:CompilationErrors = @()
$script:AuthToken = $null
$script:TestCounter = 0

function Write-Section($title) {
    Write-Host ""
    Write-Host "$Cyan========================================$Reset"
    Write-Host "$Cyan$title$Reset"
    Write-Host "$Cyan========================================$Reset"
    Write-Host ""
}

function Write-Subsection($title) {
    Write-Host ""
    Write-Host "$Blue----------------------------------------$Reset"
    Write-Host "$Blue$title$Reset"
    Write-Host "$Blue----------------------------------------$Reset"
}

function Write-Success($msg) { Write-Host "$Green✓ $msg$Reset" }
function Write-Failure($msg) { Write-Host "$Red✗ $msg$Reset" }
function Write-Info($msg) { Write-Host "$Yellow→ $msg$Reset" }
function Write-Debug($label, $content) {
    Write-Host "$Magenta[$label]$Reset"
    if ($content -is [hashtable] -or $content -is [System.Collections.IDictionary]) {
        $content | ConvertTo-Json -Depth 3 | Write-Host
    } else {
        Write-Host $content
    }
    Write-Host ""
}

# Phase 1: Compilation
function Compile-Core() {
    Write-Section "PHASE 1: COMPILATION"
    
    $CorePath = Resolve-Path $CorePath
    Write-Info "Core path: $CorePath"
    
    Push-Location $CorePath
    
    try {
        Write-Info "Running cargo build..."
        $output = & cargo build 2>&1
        $exitCode = $LASTEXITCODE
        
        if ($exitCode -eq 0) {
            Write-Success "Compilation successful!"
            return @{ Success = $true; ExitCode = $exitCode; Output = $output }
        } else {
            Write-Failure "Compilation failed!"
            
            # Parse errors
            $errors = @()
            foreach ($line in $output) {
                if ($line -match 'error\[' -or $line -match '^error:') {
                    $errors += @{
                        Type = "compile_error"
                        Message = $line
                        File = $null
                        Line = $null
                        Suggestion = $null
                    }
                }
                if ($line -match '-->\s+(.+):(\d+):(\d+)') {
                    if ($errors.Count -gt 0) {
                        $errors[$errors.Count - 1].File = $matches[1]
                        $errors[$errors.Count - 1].Line = [int]$matches[2]
                    }
                }
            }
            
            $script:CompilationErrors = $errors
            
            Write-Host "$Red=== COMPILATION ERRORS ===$Reset"
            foreach ($err in $errors) {
                Write-Host "$Red[ERROR] $($err.File):$($err.Line)$Reset"
                Write-Host "  $($err.Message)"
            }
            
            return @{ 
                Success = $false; 
                ExitCode = $exitCode; 
                Output = $output;
                Errors = $errors
            }
        }
    }
    catch {
        Write-Failure "Compilation exception: $_"
        return @{ Success = $false; Error = $_.Exception.Message }
    }
    finally {
        Pop-Location
    }
}

# Phase 2: Start Server
function Start-Server() {
    Write-Section "PHASE 2: SERVER STARTUP"
    
    # Create test data directory
    if (!(Test-Path $LodestonePath)) {
        New-Item -ItemType Directory -Path $LodestonePath | Out-Null
    }
    $env:LODESTONE_PATH = Resolve-Path $LodestonePath
    Write-Info "Data directory: $env:LODESTONE_PATH"
    
    # Kill existing processes
    Write-Info "Checking for existing processes on port $Port..."
    Get-NetTCPConnection -LocalPort $Port -ErrorAction SilentlyContinue | 
        ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 2
    
    # Start server
    Write-Info "Starting core server..."
    $stdoutPath = Join-Path $env:TEMP "core-stdout-$(Get-Random).log"
    $stderrPath = Join-Path $env:TEMP "core-stderr-$(Get-Random).log"
    
    $serverProcess = Start-Process -FilePath "cargo" -ArgumentList "run", "--", "--lodestone-path", $env:LODESTONE_PATH `
        -WorkingDirectory (Resolve-Path $CorePath) -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
    
    $script:ServerStdoutPath = $stdoutPath
    $script:ServerStderrPath = $stderrPath
    $script:ServerProcess = $serverProcess
    
    # Wait for health check
    Write-Info "Waiting for server to start..."
    $startTime = Get-Date
    $ready = $false
    $attempts = 0
    
    while (!$ready -and ((Get-Date) - $startTime).TotalSeconds -lt $TimeoutSeconds) {
        $attempts++
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
    
    $startupTime = ((Get-Date) - $startTime).TotalMilliseconds
    
    if ($ready) {
        Write-Success "Server ready in $([math]::Round($startupTime))ms"
        
        # Capture initial logs
        Start-Sleep -Seconds 1
        $script:ServerLogs.Stdout = Get-Content $stdoutPath -ErrorAction SilentlyContinue | Select-Object -Last 50
        $script:ServerLogs.Stderr = Get-Content $stderrPath -ErrorAction SilentlyContinue | Select-Object -Last 50
        
        return @{ Success = $true; StartupTimeMs = $startupTime; Port = $Port }
    } else {
        Write-Failure "Server failed to start within $TimeoutSeconds seconds"
        
        # Capture logs for debugging
        $script:ServerLogs.Stdout = Get-Content $stdoutPath -ErrorAction SilentlyContinue | Select-Object -Last 50
        $script:ServerLogs.Stderr = Get-Content $stderrPath -ErrorAction SilentlyContinue | Select-Object -Last 50
        
        Write-Subsection "Server Logs (Stdout)"
        $script:ServerLogs.Stdout | ForEach-Object { Write-Host "  $_" }
        
        Write-Subsection "Server Logs (Stderr)"
        $script:ServerLogs.Stderr | ForEach-Object { Write-Host "$Red  $_$Reset" }
        
        Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
        return @{ Success = $false; Errors = @("Server startup timeout") }
    }
}

# Helper: Make HTTP Request
function Invoke-ApiRequest($method, $path, $body, $headers) {
    $url = "http://localhost:$Port$path"
    $startTime = Get-Date
    
    $result = @{
        StatusCode = 0
        Headers = @{}
        Body = $null
        RawBody = ""
        Error = $null
        ResponseTimeMs = 0
    }
    
    try {
        $params = @{
            Uri = $url
            Method = $method
            TimeoutSec = 10
            ErrorAction = "Stop"
        }
        
        if ($headers) {
            $params.Headers = $headers
        }
        
        if ($body) {
            $params.Body = ($body | ConvertTo-Json)
            $params.ContentType = "application/json"
        }
        
        $response = Invoke-RestMethod @params
        
        $result.StatusCode = 200
        $result.Body = $response
        $result.RawBody = ($response | ConvertTo-Json -Depth 5)
    }
    catch [System.Net.WebException] {
        $result.StatusCode = [int]$_.Exception.Response.StatusCode
        try {
            $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
            $result.RawBody = $reader.ReadToEnd()
            $result.Body = $result.RawBody | ConvertFrom-Json -ErrorAction SilentlyContinue
        }
        catch {}
        
        $result.Headers = @{}
        $_.Exception.Response.Headers | ForEach-Object { 
            $result.Headers[$_.Name] = $_.Value 
        }
    }
    catch {
        $result.Error = $_.Exception.Message
    }
    
    $result.ResponseTimeMs = ((Get-Date) - $startTime).TotalMilliseconds
    return $result
}

# Phase 3: Run Tests
function Run-Tests() {
    Write-Section "PHASE 3: RUNNING API TESTS"
    
    $testUuid = [guid]::NewGuid().ToString()
    
    function Add-TestResult($testId, $name, $description, $method, $path, $expectedStatus, $actualResult, $body, $validationPassed, $validationMsg) {
        $script:TestCounter++
        $passed = ($actualResult.StatusCode -eq $expectedStatus) -and $validationPassed -and !$actualResult.Error
        
        $testResult = @{
            TestId = $testId
            Name = $name
            Description = $description
            Method = $method
            Path = $path
            ExpectedStatus = $expectedStatus
            ActualStatus = $actualResult.StatusCode
            ResponseTimeMs = [math]::Round($actualResult.ResponseTimeMs, 2)
            Body = $body
            ResponseBody = $actualResult.Body
            ResponseRaw = $actualResult.RawBody.Substring(0, [Math]::Min(500, $actualResult.RawBody.Length))
            Success = $passed
            ValidationPassed = $validationPassed
            ValidationMessage = $validationMsg
            Error = $actualResult.Error
            Timestamp = (Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff")
        }
        
        $script:TestResults += $testResult
        
        if ($passed) {
            Write-Success "[$($script:TestCounter.ToString().PadLeft(2, '0'))] $name ($([math]::Round($actualResult.ResponseTimeMs, 1))ms)"
        } else {
            Write-Failure "[$($script:TestCounter.ToString().PadLeft(2, '0'))] $name"
            
            if ($actualResult.Error) {
                Write-Host "$Red  Connection Error: $($actualResult.Error)$Reset"
            } elseif ($actualResult.StatusCode -ne $expectedStatus) {
                Write-Host "$Red  Status Mismatch: Expected $expectedStatus, got $($actualResult.StatusCode)$Reset"
            }
            
            if ($validationMsg) {
                Write-Host "$Yellow  Validation: $validationMsg$Reset"
            }
            
            if ($actualResult.Body) {
                Write-Host "  Response Body:"
                $actualResult.Body | ConvertTo-Json -Depth 3 | ForEach-Object { Write-Host "    $_" }
            }
        }
        
        return $testResult
    }
    
    # Test 1: Health Check
    $res = Invoke-ApiRequest "GET" "/health"
    $test = Add-TestResult 1 "Health Check" "Verify server health endpoint" "GET" "/health" 200 $res $null `
        ($res.Body -and $res.Body.status -eq "healthy") "status field should be 'healthy'"
    
    # Test 2: System Stats
    $res = Invoke-ApiRequest "GET" "/stats"
    $validation = ($res.Body -and $res.Body.version -and $res.Body.PSObject.Properties.Name -contains "running_instances")
    $test = Add-TestResult 2 "System Stats" "Verify system stats returns required fields" "GET" "/stats" 200 $res $null `
        $validation "Response should contain version and running_instances"
    
    # Test 3: Login - Invalid Credentials
    $res = Invoke-ApiRequest "POST" "/login" @{ username = "nonexistent"; password = "wrongpassword123" }
    $test = Add-TestResult 3 "Login - Invalid Credentials" "Verify 401 on bad credentials" "POST" "/login" 401 $res `
        @{ username = "nonexistent"; password = "wrongpassword123" } $true $null
    
    # Test 4: Login - Short Username
    $res = Invoke-ApiRequest "POST" "/login" @{ username = "ab"; password = "validpassword123" }
    $test = Add-TestResult 4 "Login - Validation (Short Username)" "Verify validation error for short username" "POST" "/login" 422 $res `
        @{ username = "ab"; password = "validpassword123" } $true $null
    
    # Test 5: Login - Short Password
    $res = Invoke-ApiRequest "POST" "/login" @{ username = "validuser"; password = "short" }
    $test = Add-TestResult 5 "Login - Validation (Short Password)" "Verify validation error for short password" "POST" "/login" 422 $res `
        @{ username = "validuser"; password = "short" } $true $null
    
    # Test 6: Setup - Short Key
    $res = Invoke-ApiRequest "POST" "/setup" @{ key = "short"; username = "admin"; password = "securepassword123" }
    $test = Add-TestResult 6 "Setup - Validation (Short Key)" "Verify validation error for short key" "POST" "/setup" 422 $res `
        @{ key = "short"; username = "admin"; password = "securepassword123" } $true $null
    
    # Test 7: Setup - Short Username
    $res = Invoke-ApiRequest "POST" "/setup" @{ key = "this-is-a-valid-setup-key"; username = "ab"; password = "securepassword123" }
    $test = Add-TestResult 7 "Setup - Validation (Short Username)" "Verify validation error for short username" "POST" "/setup" 422 $res `
        @{ key = "this-is-a-valid-setup-key"; username = "ab"; password = "securepassword123" } $true $null
    
    # Test 8: Setup - Short Password
    $res = Invoke-ApiRequest "POST" "/setup" @{ key = "this-is-a-valid-setup-key"; username = "admin"; password = "short" }
    $test = Add-TestResult 8 "Setup - Validation (Short Password)" "Verify validation error for short password" "POST" "/setup" 422 $res `
        @{ key = "this-is-a-valid-setup-key"; username = "admin"; password = "short" } $true $null
    
    # Test 9: Setup - Valid Request
    $res = Invoke-ApiRequest "POST" "/setup" @{ key = "this-is-a-valid-setup-key"; username = "testadmin"; password = "securepassword123" }
    $validation = ($res.Body -and $res.Body.token)
    $test = Add-TestResult 9 "Setup - Valid Request" "Verify successful setup returns token" "POST" "/setup" 200 $res `
        @{ key = "this-is-a-valid-setup-key"; username = "testadmin"; password = "securepassword123" } `
        $validation "Response should contain token field"
    
    # Test 10: Login - Valid Credentials
    $res = Invoke-ApiRequest "POST" "/login" @{ username = "testadmin"; password = "securepassword123" }
    $validation = ($res.Body -and $res.Body.token -and $res.Body.user_id)
    $test = Add-TestResult 10 "Login - Valid Credentials" "Verify successful login returns token and user_id" "POST" "/login" 200 $res `
        @{ username = "testadmin"; password = "securepassword123" } `
        $validation "Response should contain token and user_id"
    
    if ($test.Success) {
        $script:AuthToken = $test.ResponseBody.token
    }
    
    # Instance management tests with non-existent instance
    # Test 11: Start Instance - Not Found
    $res = Invoke-ApiRequest "POST" "/instances/$testUuid/start"
    $test = Add-TestResult 11 "Start Instance - Not Found" "Verify 404 for non-existent instance" "POST" "/instances/:id/start" 404 $res $null $true $null
    
    # Test 12: Stop Instance - Not Found
    $res = Invoke-ApiRequest "POST" "/instances/$testUuid/stop"
    $test = Add-TestResult 12 "Stop Instance - Not Found" "Verify 404 for non-existent instance" "POST" "/instances/:id/stop" 404 $res $null $true $null
    
    # Test 13: Kill Instance - Not Found
    $res = Invoke-ApiRequest "POST" "/instances/$testUuid/kill"
    $test = Add-TestResult 13 "Kill Instance - Not Found" "Verify 404 for non-existent instance" "POST" "/instances/:id/kill" 404 $res $null $true $null
    
    # Test 14: Send Command - Not Found
    $res = Invoke-ApiRequest "POST" "/instances/$testUuid/command" @{ command = "say Hello" }
    $test = Add-TestResult 14 "Send Command - Not Found" "Verify 404 for non-existent instance" "POST" "/instances/:id/command" 404 $res @{ command = "say Hello" } $true $null
    
    # Test 15: Get Modpack Metadata - Not Found
    $res = Invoke-ApiRequest "GET" "/instances/$testUuid/metadata"
    $test = Add-TestResult 15 "Get Modpack Metadata - Not Found" "Verify 404 for non-existent instance" "GET" "/instances/:id/metadata" 404 $res $null $true $null
    
    # Test 16: Download Modpack - Not Found
    $res = Invoke-ApiRequest "GET" "/instances/$testUuid/modpack"
    $test = Add-TestResult 16 "Download Modpack - Not Found" "Verify 404 for non-existent instance" "GET" "/instances/:id/modpack" 404 $res $null $true $null
    
    # Test 17: Console WebSocket Endpoint
    $res = Invoke-ApiRequest "GET" "/instances/$testUuid/console"
    $test = Add-TestResult 17 "Console WebSocket Endpoint" "Verify console endpoint accessible" "GET" "/instances/:id/console" 200 $res $null $true $null
    
    # Test 18: Upload Modpack - No File
    $res = Invoke-ApiRequest "POST" "/instances"
    $test = Add-TestResult 18 "Upload Modpack - No File" "Verify error without file" "POST" "/instances" 400 $res $null $true $null
    
    # Test 19: Invalid Path
    $res = Invoke-ApiRequest "GET" "/nonexistent/path"
    $test = Add-TestResult 19 "Invalid Path" "Verify 404 for invalid path" "GET" "/nonexistent/path" 404 $res $null $true $null
    
    # Test 20: Method Not Allowed (POST /health)
    $res = Invoke-ApiRequest "POST" "/health"
    $test = Add-TestResult 20 "Method Not Allowed (POST /health)" "Verify 405 for wrong method" "POST" "/health" 405 $res $null $true $null
    
    # Test 21: Method Not Allowed (GET /instances/:id/start)
    $res = Invoke-ApiRequest "GET" "/instances/$testUuid/start"
    $test = Add-TestResult 21 "Method Not Allowed (GET /start)" "Verify 405 for wrong method" "GET" "/instances/:id/start" 405 $res $null $true $null
}

# Phase 4: Generate Report
function Generate-Report() {
    Write-Section "PHASE 4: TEST REPORT"
    
    $passed = ($script:TestResults | Where-Object { $_.Success }).Count
    $failed = ($script:TestResults | Where-Object { -not $_.Success }).Count
    $total = $script:TestResults.Count
    
    Write-Host ""
    Write-Host "$Cyan=== SUMMARY ===$Reset"
    Write-Host "  Total Tests: $total"
    Write-Host "  $Green Passed: $passed$Reset"
    Write-Host "  $Red Failed: $failed$Reset"
    if ($total -gt 0) {
        Write-Host "  Success Rate: $([math]::Round(($passed / $total) * 100, 2))%"
    }
    
    # Failed tests detailed analysis
    $failedTests = $script:TestResults | Where-Object { -not $_.Success }
    
    if ($failedTests.Count -gt 0) {
        Write-Subsection "FAILED TESTS - DETAILED ANALYSIS"
        
        foreach ($test in $failedTests) {
            Write-Host ""
            Write-Host "$Red[$($test.TestId.ToString().PadLeft(2, '0'))] $($test.Name)$Reset"
            Write-Host "  Description: $($test.Description)"
            Write-Host "  $Yellow Expected: HTTP $($test.ExpectedStatus)$Reset"
            Write-Host "  $Red Actual: HTTP $($test.ActualStatus)$Reset"
            
            if ($test.ValidationMessage) {
                Write-Host "  Validation: $($test.ValidationMessage)"
            }
            
            if ($test.ResponseBody) {
                Write-Host "  Response Body:"
                $test.ResponseBody | ConvertTo-Json -Depth 3 | ForEach-Object { Write-Host "    $_" }
            } elseif ($test.ResponseRaw) {
                Write-Host "  Raw Response: $($test.ResponseRaw)"
            }
            
            # Generate recommendation
            $recommendation = $null
            if ($test.ActualStatus -eq 500) {
                $recommendation = "Server returned 500 Internal Server Error. Check handler implementation and server logs."
            } elseif ($test.ActualStatus -eq 404 -and $test.ExpectedStatus -eq 200) {
                $recommendation = "Endpoint returned 404. Verify route is registered in router.rs and handler exists."
            } elseif ($test.ActualStatus -eq 405) {
                $recommendation = "Method not allowed. Check router.rs for correct HTTP method binding."
            } elseif ($test.ExpectedStatus -eq 422 -and $test.ActualStatus -eq 200) {
                $recommendation = "Expected validation error (422) but got success. Check validation logic in handler."
            } elseif ($test.ActualStatus -eq 0) {
                $recommendation = "Connection failed. Server may have crashed or is not responding."
            }
            
            if ($recommendation) {
                Write-Host "  $Cyan→ $recommendation$Reset"
            }
        }
    }
    
    # Build final report
    $report = @{
        RunInfo = @{
            Timestamp = (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
            CorePath = (Resolve-Path $CorePath).Path
            Port = $Port
            TestDataPath = (Resolve-Path $LodestonePath).Path
        }
        Phases = @{
            Compilation = if ($script:CompilationErrors.Count -gt 0) { 
                @{ Success = $false; Errors = $script:CompilationErrors }
            } else {
                @{ Success = $true }
            }
            Startup = @{ 
                Success = $script:ServerProcess -ne $null
                Logs = $script:ServerLogs
            }
            Tests = @{
                Summary = @{
                    Total = $total
                    Passed = $passed
                    Failed = $failed
                    SuccessRate = if ($total -gt 0) { [math]::Round(($passed / $total) * 100, 2) } else { 0 }
                }
                Results = $script:TestResults
                FailedTests = $failedTests
            }
        }
        OverallSuccess = ($passed -eq $total -and $total -gt 0)
    }
    
    return $report
}

# Phase 5: Cleanup
function Stop-Server() {
    Write-Section "PHASE 5: CLEANUP"
    
    if ($script:ServerProcess) {
        Write-Info "Stopping server..."
        Stop-Process -Id $script:ServerProcess.Id -Force -ErrorAction SilentlyContinue
        Write-Success "Server stopped"
    }
    
    # Cleanup temp files
    if ($script:ServerStdoutPath -and (Test-Path $script:ServerStdoutPath)) {
        Remove-Item $script:ServerStdoutPath -ErrorAction SilentlyContinue
    }
    if ($script:ServerStderrPath -and (Test-Path $script:ServerStderrPath)) {
        Remove-Item $script:ServerStderrPath -ErrorAction SilentlyContinue
    }
    
    Write-Success "Cleanup complete"
}

# Main execution
function Main() {
    $compilationResult = $null
    $startupResult = $null
    $report = $null
    
    try {
        # Phase 1: Compile
        $compilationResult = Compile-Core
        if (-not $compilationResult.Success) {
            Write-Failure "Compilation failed. Cannot proceed with tests."
            exit 1
        }
        
        # Phase 2: Start Server
        $startupResult = Start-Server
        if (-not $startupResult.Success) {
            Write-Failure "Server startup failed. Cannot proceed with tests."
            exit 1
        }
        
        # Phase 3: Run Tests
        Run-Tests
        
        # Phase 4: Generate Report
        $report = Generate-Report
        
        # Save JSON report
        $reportPath = Join-Path (Get-Location) "test-report.json"
        $report | ConvertTo-Json -Depth 10 | Set-Content $reportPath
        Write-Info "Full report saved to: $reportPath"
        
        # Return appropriate exit code
        if ($report.OverallSuccess) {
            Write-Success "All tests passed!"
            exit 0
        } else {
            Write-Failure "Some tests failed!"
            exit 1
        }
    }
    finally {
        Stop-Server
    }
}

# Run main
Main
