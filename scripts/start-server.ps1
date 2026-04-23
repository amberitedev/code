#!/usr/bin/env pwsh
# Start Amberite Core server

$env:LIBCLANG_PATH = "C:\Users\ilai\scoop\apps\llvm\current\bin"
$env:Path = "C:\Users\ilai\scoop\shims;$env:Path"

$dataDir = "C:\temp\amberite-test-data"
if (!(Test-Path $dataDir)) {
    New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
}
if (!(Test-Path "$dataDir\.lodestone")) {
    New-Item -ItemType Directory -Path "$dataDir\.lodestone" -Force | Out-Null
}

$env:LODESTONE_PATH = $dataDir

Start-Process -FilePath "cargo" `
    -ArgumentList "run", "--", "--lodestone-path", $dataDir `
    -WorkingDirectory "C:\Users\ilai\amberite\apps\core" `
    -WindowStyle Hidden

Write-Host "Server starting..."
Start-Sleep -Seconds 5

# Wait for health endpoint
$attempts = 0
$maxAttempts = 30
while ($attempts -lt $maxAttempts) {
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:16662/health" -Method GET -TimeoutSec 2 -ErrorAction Stop
        if ($response.status -eq "healthy") {
            Write-Host "Server is running on port 16662"
            exit 0
        }
    }
    catch {
        $attempts++
        Start-Sleep -Seconds 1
    }
}

Write-Host "Server failed to start"
exit 1
