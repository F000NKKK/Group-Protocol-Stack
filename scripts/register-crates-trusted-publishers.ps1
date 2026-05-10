<#
.SYNOPSIS
  Manages GitHub Actions Trusted Publishers on crates.io for every crate
  in this workspace, using the crates.io REST API.

.DESCRIPTION
  Trusted Publishing on crates.io has been generally available since June
  2024. The web UI requires you to configure each crate one-by-one; this
  script does the same calls via the public API in a loop.

  By default the script is idempotent: it lists existing publishers per
  crate and only creates one if no match (same owner/repo/workflow/env)
  exists. Pass -DeduplicateExisting to delete duplicate entries that were
  created by earlier runs of this script.

.PARAMETER Token
  crates.io API token. If not provided, the script reads
  $env:CRATES_IO_API_TOKEN.

.PARAMETER Owner
  GitHub repository owner. Default: F000NKKK.

.PARAMETER Repository
  GitHub repository name. Default: Group-Protocol-Stack.

.PARAMETER Workflow
  Workflow filename under .github/workflows/. Default: release.yml.

.PARAMETER Environment
  GitHub Actions environment used for publishing. Default: crates-io.

.PARAMETER Crates
  List of crate names to configure. Defaults to every member of this
  workspace.

.PARAMETER DeduplicateExisting
  Before creating, delete every duplicate that already matches the
  configuration, leaving exactly one.

.EXAMPLE
  pwsh ./scripts/register-crates-trusted-publishers.ps1
  pwsh ./scripts/register-crates-trusted-publishers.ps1 -DeduplicateExisting
#>

[CmdletBinding()]
param(
    [string]$Token       = $env:CRATES_IO_API_TOKEN,
    [string]$Owner       = "F000NKKK",
    [string]$Repository  = "Group-Protocol-Stack",
    [string]$Workflow    = "release.yml",
    [string]$Environment = "crates-io",
    [string[]]$Crates    = @(
        "gbp-core",
        "gbp-protocol",
        "gbp-mls",
        "gbp-transport",
        "gbp-node",
        "gtp-protocol",
        "gap-protocol",
        "gsp-protocol",
        "gbp-stack",
        "gbp-stack-ffi",
        "gbp-cli"
    ),
    [switch]$DeduplicateExisting
)

if (-not $Token) {
    throw "crates.io token not found. Pass -Token or set CRATES_IO_API_TOKEN."
}

$base = "https://crates.io/api/v1"
$headers = @{
    "Authorization" = $Token
    "User-Agent"    = "$Owner/$Repository (registration script)"
    "Content-Type"  = "application/json; charset=utf-8"
    "Accept"        = "application/json"
}

function Get-Publishers([string]$crate) {
    try {
        $resp = Invoke-RestMethod `
            -Uri "$base/trusted_publishing/github_configs?crate=$crate" `
            -Headers $headers `
            -Method Get `
            -ErrorAction Stop
        # The API returns either {github_configs:[...]} or a flat array.
        if ($resp.github_configs) { return ,$resp.github_configs }
        if ($resp -is [System.Collections.IEnumerable]) { return ,@($resp) }
        return ,@($resp)
    } catch {
        Write-Host "    list FAILED: $($_.Exception.Message)" -ForegroundColor Red
        return ,@()
    }
}

function Test-Match($cfg, $crate) {
    return ($cfg.crate -eq $crate -or $null -eq $cfg.crate) -and `
           ($cfg.repository_owner -eq $Owner) -and `
           ($cfg.repository_name -eq $Repository) -and `
           ($cfg.workflow_filename -eq $Workflow) -and `
           (($cfg.environment -eq $Environment) -or `
            ($null -eq $cfg.environment -and [string]::IsNullOrEmpty($Environment)))
}

function Remove-Publisher([int]$id) {
    try {
        Invoke-RestMethod `
            -Uri "$base/trusted_publishing/github_configs/$id" `
            -Headers $headers `
            -Method Delete `
            -ErrorAction Stop | Out-Null
        return $true
    } catch {
        Write-Host "    delete FAILED: $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
}

function Add-Publisher([string]$crate) {
    $body = @{
        github_config = @{
            crate             = $crate
            repository_owner  = $Owner
            repository_name   = $Repository
            workflow_filename = $Workflow
            environment       = $Environment
        }
    } | ConvertTo-Json -Depth 4 -Compress
    try {
        Invoke-RestMethod `
            -Uri "$base/trusted_publishing/github_configs" `
            -Method Post `
            -Headers $headers `
            -Body $body `
            -ErrorAction Stop | Out-Null
        Write-Host "    CREATED" -ForegroundColor Green
        return $true
    } catch {
        $msg = $_.Exception.Message
        if ($_.ErrorDetails) { $msg = "$msg | $($_.ErrorDetails.Message)" }
        Write-Host "    create FAILED: $msg" -ForegroundColor Red
        return $false
    }
}

foreach ($crate in $Crates) {
    Write-Host "==> $crate" -ForegroundColor Cyan

    $existing = Get-Publishers $crate
    $matches  = @($existing | Where-Object { Test-Match $_ $crate })

    Write-Host "    existing matches: $($matches.Count)"

    if ($matches.Count -eq 0) {
        Add-Publisher $crate | Out-Null
        continue
    }

    if ($DeduplicateExisting -and $matches.Count -gt 1) {
        $keep = $matches | Select-Object -First 1
        $kill = $matches | Select-Object -Skip 1
        Write-Host "    keeping id=$($keep.id); removing $($kill.Count) duplicate(s)" -ForegroundColor Yellow
        foreach ($d in $kill) {
            if (Remove-Publisher $d.id) {
                Write-Host "      removed id=$($d.id)"
            }
        }
        continue
    }

    if ($matches.Count -eq 1) {
        Write-Host "    skip (already configured, id=$($matches[0].id))" -ForegroundColor Yellow
    } else {
        Write-Host "    skip ($($matches.Count) duplicates exist; rerun with -DeduplicateExisting to clean up)" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Done. Verify at https://crates.io/me/pending-publishers" -ForegroundColor Green
