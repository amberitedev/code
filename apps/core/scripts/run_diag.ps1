# Amberite Core — Diagnostic Runner
# Usage: .\run_diag.ps1 -Token <jwt> [-Url http://localhost:16662]

param(
    [string]$Url   = "http://localhost:16662",
    [string]$Token = "",
    [string]$PairingCode = "",
    [string]$SupabaseUrl = "",
    [string]$OwnerId = ""
)

$ErrorActionPreference = "Stop"
$script = Join-Path $PSScriptRoot "diagnostic.py"

if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    Write-Error "python not found in PATH. Install Python 3.11+."
    exit 1
}

$req = python -c "import requests" 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing requests …"
    python -m pip install requests --quiet
}

$args = @("--url", $Url)
if ($Token)       { $args += @("--token",        $Token) }
if ($PairingCode) { $args += @("--pairing-code", $PairingCode) }
if ($SupabaseUrl) { $args += @("--supabase-url", $SupabaseUrl) }
if ($OwnerId)     { $args += @("--owner-id",     $OwnerId) }

python $script @args
exit $LASTEXITCODE
