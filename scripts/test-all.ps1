<#
.SYNOPSIS
    Build the native library, then run integration tests for all three bindings
    (Python, C#, JavaScript).

.PARAMETER Release
    Use the release build of the native library.

.PARAMETER Skip
    Comma-separated list of bindings to skip: python, csharp, js
    Example: -Skip python,js

.EXAMPLE
    .\scripts\test-all.ps1
    .\scripts\test-all.ps1 -Release
    .\scripts\test-all.ps1 -Skip csharp
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [string]$Skip = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot  = Split-Path -Parent $PSScriptRoot
$skipped   = $Skip.ToLower() -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
$failures  = @()

# ── Step 1: Build and distribute the native library ─────────────────────────
Write-Host "=== Building native library ===" -ForegroundColor Yellow
$installArgs = if ($Release) { @("-Release") } else { @() }
& "$PSScriptRoot\install-native.ps1" @installArgs

# ── Step 2: Python tests ─────────────────────────────────────────────────────
if ("python" -notin $skipped) {
    Write-Host "`n=== Python tests ===" -ForegroundColor Yellow
    Push-Location (Join-Path $repoRoot "python")
    try {
        python -m pytest tests/test_integration.py -v --tb=short
        if ($LASTEXITCODE -ne 0) { $failures += "Python" }
    } finally { Pop-Location }
}

# ── Step 3: C# tests ─────────────────────────────────────────────────────────
if ("csharp" -notin $skipped) {
    Write-Host "`n=== C# tests ===" -ForegroundColor Yellow
    Push-Location $repoRoot
    try {
        dotnet test csharp/GBPStack.Tests/GBPStack.Tests.csproj --logger "console;verbosity=normal"
        if ($LASTEXITCODE -ne 0) { $failures += "C#" }
    } finally { Pop-Location }
}

# ── Step 4: JavaScript tests ──────────────────────────────────────────────────
if ("js" -notin $skipped) {
    Write-Host "`n=== JavaScript tests ===" -ForegroundColor Yellow
    Push-Location (Join-Path $repoRoot "js")
    try {
        npm test
        if ($LASTEXITCODE -ne 0) { $failures += "JavaScript" }
    } finally { Pop-Location }
}

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host ""
if ($failures.Count -eq 0) {
    Write-Host "All tests passed." -ForegroundColor Green
} else {
    Write-Host "FAILED: $($failures -join ', ')" -ForegroundColor Red
    exit 1
}
