<#
.SYNOPSIS
    Build gbp-stack-ffi and distribute the native DLL to all language-binding
    directories so local test runs pick it up without manual copying.

.PARAMETER Release
    Build in release mode instead of debug.

.PARAMETER TargetDir
    Override the Cargo target directory (default: <repo-root>/target).

.EXAMPLE
    .\scripts\install-native.ps1
    .\scripts\install-native.ps1 -Release
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [string]$TargetDir = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot

# ── 1. Cargo build ──────────────────────────────────────────────────────────
$profile  = if ($Release) { "release" } else { "debug" }
$cargoArgs = @("build", "-p", "gbp-stack-ffi")
if ($Release) { $cargoArgs += "--release" }

Write-Host "cargo $($cargoArgs -join ' ')" -ForegroundColor Cyan
Push-Location $repoRoot
try {
    # Cargo writes progress to stderr; lower the action preference so PowerShell 5.1
    # does not treat those stderr lines as terminating errors.
    $savedPref = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    & cargo @cargoArgs
    $cargoExit = $LASTEXITCODE
    $ErrorActionPreference = $savedPref
    if ($cargoExit -ne 0) { throw "cargo build failed (exit $cargoExit)" }
} finally {
    Pop-Location
}

# ── 2. Resolve DLL path ─────────────────────────────────────────────────────
$targetBase = if ($TargetDir) { $TargetDir } else { Join-Path $repoRoot "target" }
$dllName    = "gbp_stack.dll"
$srcDll     = Join-Path (Join-Path $targetBase $profile) $dllName

if (-not (Test-Path $srcDll)) {
    throw "DLL not found: $srcDll"
}
Write-Host "Built: $srcDll" -ForegroundColor Green

# ── 3. Copy to binding directories ──────────────────────────────────────────
function Join-Paths([string]$base, [string[]]$parts) {
    $result = $base
    foreach ($p in $parts) { $result = Join-Path $result $p }
    $result
}

$destinations = @(
    # Python — platform-specific native dir
    (Join-Paths $repoRoot @("python", "gbp_stack", "_native", "win-x64", $dllName)),
    # C# — runtimes/<rid>/native/ (matches StageHostRuntime target in GBPStack.csproj)
    (Join-Paths $repoRoot @("csharp", "GBPStack", "runtimes", "win-x64", "native", $dllName)),
    # JS — native folder next to package.json
    (Join-Paths $repoRoot @("js", "native", "win-x64", $dllName))
)

foreach ($dst in $destinations) {
    $dir = Split-Path -Parent $dst
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    Copy-Item $srcDll $dst -Force
    Write-Host "  -> $dst" -ForegroundColor Gray
}

Write-Host "`nDone. Native library installed for all bindings." -ForegroundColor Green
