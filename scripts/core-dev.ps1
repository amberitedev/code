#Requires -Version 5.1
<#
.SYNOPSIS
    Small temporary process manager for Amberite Core in dev.

.DESCRIPTION
    Start, stop, restart, or check status of the Core backend.
    Designed for agent loops: restart blocks until the HTTP port is open,
    prints startup errors, and enforces a startup timeout so the caller never hangs.

.EXAMPLE
    .\scripts\core-dev.ps1 restart
#>
param(
	[Parameter(Mandatory = $true, Position = 0)]
	[ValidateSet("start", "stop", "restart", "status", "logs", "wait")]
	[string]$Command
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# 1. Config & paths
# ---------------------------------------------------------------------------
$repoRoot = Split-Path -Parent $PSScriptRoot
$coreDir = Join-Path $repoRoot "apps\core"
$logDir = Join-Path $coreDir "logs"
$logFile = Join-Path $logDir "core-dev.log"
$pidFile = Join-Path $coreDir ".core-dev.pid"

$hostAddr = if ($env:CORE_HOST) { $env:CORE_HOST } else { "127.0.0.1" }
$port = if ($env:CORE_PORT) { [int]$env:CORE_PORT } else { 16662 }
$timeout = if ($env:CORE_DEV_TIMEOUT) { [int]$env:CORE_DEV_TIMEOUT } else { 180 }
$pollInterval = 1

# ---------------------------------------------------------------------------
# 2. Helpers
# ---------------------------------------------------------------------------
function Test-PortOpen {
	try {
		$client = New-Object System.Net.Sockets.TcpClient
		$client.Connect($hostAddr, $port)
		$client.Close()
		return $true
	}
	catch {
		return $false
	}
}

function Get-PidFromFile {
	if (Test-Path $pidFile) {
		$content = Get-Content $pidFile -Raw
		$int = 0
		if ([int]::TryParse($content, [ref]$int)) {
			return $int
		}
	}
	return $null
}

function Show-Logs {
	if (Test-Path $logFile) {
		Write-Host "--- last 50 lines of $logFile ---"
		Get-Content $logFile -Tail 50
		Write-Host "------------------------------------"
	}
	else {
		Write-Host "No log file found."
	}
}

# ---------------------------------------------------------------------------
# 3. Commands
# ---------------------------------------------------------------------------
function Stop-Core {
	Write-Host "[core-dev] Stopping core..."

	# Kill the wrapper process we recorded.
	$corePid = Get-PidFromFile
	if ($corePid) {
		$proc = Get-Process -Id $corePid -ErrorAction SilentlyContinue
		if ($proc) {
			Stop-Process -Id $corePid -Force -ErrorAction SilentlyContinue
			# Kill children recursively.
			Get-CimInstance Win32_Process -Filter "ParentProcessId=$corePid" -ErrorAction SilentlyContinue | ForEach-Object {
				Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
			}
		}
	}

	# Fallback: kill whoever is listening on the port.
	$conn = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue | Select-Object -First 1
	if ($conn -and $conn.OwningProcess) {
		Stop-Process -Id $conn.OwningProcess -Force -ErrorAction SilentlyContinue
	}

	if (Test-Path $pidFile) {
		Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
	}

	Write-Host "[core-dev] Core stopped."
}

function Start-Core {
	$existingPid = Get-PidFromFile
	if ($existingPid) {
		$proc = Get-Process -Id $existingPid -ErrorAction SilentlyContinue
		if ($proc) {
			if (Test-PortOpen) {
				Write-Host "[core-dev] Core already online at http://${hostAddr}:$port (PID: $existingPid)"
				return
			}
			Write-Host "[core-dev] Process exists (PID: $existingPid) but port not open yet -- waiting..."
		}
		else {
			Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
		}
	}

	# Handle orphan server (port open but PID file missing).
	if (Test-PortOpen) {
		Write-Host "[core-dev] Core already listening on port $port. Use 'restart' for a fresh instance."
		return
	}

	New-Item -ItemType Directory -Force -Path $logDir | Out-Null
	# Truncate logs for a clean start.
	"" | Set-Content $logFile -NoNewline

	Write-Host "[core-dev] Starting core... timeout: ${timeout}s | log: $logFile"

	# Use cmd as a wrapper so we can redirect stdout + stderr to the same log file easily.
	$proc = Start-Process -FilePath "cmd.exe" `
		-ArgumentList "/c cargo run > `"$logFile`" 2>&1" `
		-WorkingDirectory $coreDir `
		-PassThru `
		-WindowStyle Hidden

	$proc.Id | Set-Content $pidFile

	Write-Host "[core-dev] Build/startup in progress (PID: $($proc.Id))..."

	$elapsed = 0
	while ($elapsed -lt $timeout) {
		if (Test-PortOpen) {
			Write-Host "[core-dev] Core is ONLINE at http://${hostAddr}:$port (PID: $($proc.Id))"
			return
		}
		if ($proc.HasExited) {
			Write-Host "[core-dev] ERROR: Core process exited before the port opened."
			Show-Logs
			Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
			throw "Core failed to start"
		}
		Start-Sleep -Seconds $pollInterval
		$elapsed += $pollInterval
	}

	Write-Host "[core-dev] ERROR: Timed out after ${timeout}s waiting for core to come online."
	Show-Logs
	Stop-Core
	throw "Timeout waiting for core startup"
}

function Status-Core {
	$corePid = Get-PidFromFile
	if ($corePid) {
		$proc = Get-Process -Id $corePid -ErrorAction SilentlyContinue
		if ($proc) {
			Write-Host "[core-dev] Process running (PID: $corePid)"
		}
		else {
			Write-Host "[core-dev] Process not running"
		}
	}
	else {
		Write-Host "[core-dev] Process not running"
	}

	if (Test-PortOpen) {
		Write-Host "[core-dev] Port $port is OPEN"
	}
	else {
		Write-Host "[core-dev] Port $port is CLOSED"
	}
}

function Wait-Core {
	Write-Host "[core-dev] Waiting for core at ${hostAddr}:$port (timeout: ${timeout}s)..."
	$elapsed = 0
	while ($elapsed -lt $timeout) {
		if (Test-PortOpen) {
			Write-Host "[core-dev] Core is online."
			return
		}
		Start-Sleep -Seconds $pollInterval
		$elapsed += $pollInterval
	}
	Write-Host "[core-dev] ERROR: Timed out waiting for core."
	throw "Timeout waiting for core"
}

# ---------------------------------------------------------------------------
# 4. Dispatch
# ---------------------------------------------------------------------------
switch ($Command) {
	"start" { Start-Core }
	"stop" { Stop-Core }
	"restart" { Stop-Core; Start-Core }
	"status" { Status-Core }
	"logs" { Show-Logs }
	"wait" { Wait-Core }
}
