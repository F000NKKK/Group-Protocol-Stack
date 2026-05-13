<#
.SYNOPSIS
  Cuts a release: bumps the version across every package manifest, updates
  CHANGELOG.md from conventional commits, commits, tags and pushes.

.DESCRIPTION
  After this script returns, the GitHub Actions release workflow will see
  the new tag and publish to crates.io, NuGet, PyPI, npm and GitHub
  Releases.

  The "current version" is read from Cargo.toml (workspace.package.version);
  every other manifest is updated to match via scripts/bump-version.ps1.

.PARAMETER Bump
  patch | minor | major | rc — relative bump (cannot be combined with -Version).
  rc creates / increments a -rcN pre-release on the *next* patch.

.PARAMETER Version
  Exact SemVer to release (e.g. 1.0.0, 1.0.0-rc1). Wins over -Bump.

.PARAMETER NoPush
  Build the commit and tag locally but do not push.

.EXAMPLE
  pwsh ./scripts/release.ps1 -Bump patch
  pwsh ./scripts/release.ps1 -Bump rc
  pwsh ./scripts/release.ps1 -Version 1.0.0
#>

[CmdletBinding(DefaultParameterSetName = 'Bump')]
param(
    [Parameter(ParameterSetName = 'Bump')]
    [ValidateSet('patch', 'minor', 'major', 'rc')]
    [string]$Bump = 'patch',

    [Parameter(ParameterSetName = 'Version', Mandatory)]
    [string]$Version,

    [switch]$NoPush
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

function Get-CurrentVersion {
    $cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
    if ($cargo -match '(?m)^version\s*=\s*"([^"]+)"') { return $Matches[1] }
    throw "could not read workspace version from Cargo.toml"
}

function Step-Version([string]$current, [string]$bump) {
    if ($current -notmatch '^(\d+)\.(\d+)\.(\d+)(?:-(.+))?$') {
        throw "current version '$current' is not SemVer"
    }
    $maj = [int]$Matches[1]; $min = [int]$Matches[2]; $pat = [int]$Matches[3]; $pre = $Matches[4]
    switch ($bump) {
        'major' { return "$($maj + 1).0.0" }
        'minor' { return "$maj.$($min + 1).0" }
        'patch' {
            if ($pre) { return "$maj.$min.$pat" }
            return "$maj.$min.$($pat + 1)"
        }
        'rc' {
            if ($pre -and $pre -match '^rc(\d+)$') {
                return "$maj.$min.$pat-rc$([int]$Matches[1] + 1)"
            }
            return "$maj.$min.$($pat + 1)-rc1"
        }
    }
}

function Test-Clean {
    $st = git status --porcelain
    if ($st) {
        Write-Host "Working tree is not clean:" -ForegroundColor Red
        $st | Write-Host
        throw "commit or stash changes first"
    }
}

function New-ChangelogEntry([string]$version, [string]$range) {
    $sections = [ordered]@{
        feat     = '### Features'
        fix      = '### Bug Fixes'
        perf     = '### Performance'
        refactor = '### Refactoring'
        test     = '### Tests'
        docs     = '### Documentation'
        build    = '### Build'
        ci       = '### CI'
        chore    = '### Chores'
        style    = '### Style'
    }
    $today = Get-Date -Format 'yyyy-MM-dd'
    $sb = [System.Text.StringBuilder]::new()
    [void]$sb.AppendLine("## $version ($today)").AppendLine()
    $any = $false
    foreach ($type in $sections.Keys) {
        $commits = git log $range --no-merges --pretty=format:"- %s (%h)" --grep "^$type" 2>$null
        if ($commits) {
            [void]$sb.AppendLine($sections[$type]).AppendLine()
            foreach ($line in $commits) { [void]$sb.AppendLine($line) }
            [void]$sb.AppendLine()
            $any = $true
        }
    }
    if (-not $any) {
        [void]$sb.AppendLine("_No conventional commits found in this range._").AppendLine()
    }
    return $sb.ToString()
}

function Update-Changelog([string]$entry) {
    $path = Join-Path $root 'CHANGELOG.md'
    if (Test-Path $path) {
        $existing = Get-Content $path -Raw
        $new = $entry + "---`r`n`r`n" + $existing
    } else {
        $new = "# Changelog`r`n`r`n" + $entry
    }
    # CHANGELOG.md uses UTF-8 with BOM for maximum Windows tooling compatibility
    # (Notepad, legacy editors). Code manifests (Cargo.toml, .csproj, etc.) use
    # no-BOM via bump-version.ps1 to avoid breaking parsers.
    Set-Content -Path $path -Value $new -Encoding utf8
}

# --- main ---------------------------------------------------------------------

Push-Location $root
try {
    Test-Clean

    $branch = (git rev-parse --abbrev-ref HEAD).Trim()
    if ($branch -ne 'main' -and $branch -ne 'master') {
        Write-Host "WARNING: releasing from branch '$branch'" -ForegroundColor Yellow
    }

    $current = Get-CurrentVersion
    if ($PSCmdlet.ParameterSetName -eq 'Version') {
        $next = $Version.TrimStart('v').Trim().TrimEnd('.')
    } else {
        $next = Step-Version $current $Bump
    }
    if ($next -notmatch '^\d+\.\d+\.\d+(-rc\d+)?$') {
        throw "next version '$next' is not a clean SemVer (expected MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-rcN)"
    }
    if ($next -eq $current) { throw "next version equals current ($current); nothing to do" }

    Write-Host "Releasing $current -> $next" -ForegroundColor Cyan

    # 1. Bump every manifest
    & (Join-Path $PSScriptRoot 'bump-version.ps1') -Version $next

    # 2. Changelog
    $lastTag = git describe --tags --abbrev=0 2>$null
    $range = if ($LASTEXITCODE -eq 0 -and $lastTag) { "$lastTag..HEAD" } else { 'HEAD' }
    $LASTEXITCODE = 0
    $entry = New-ChangelogEntry $next $range
    Update-Changelog $entry
    Write-Host "CHANGELOG.md updated"

    # 3. Commit (manifests + every README touched by bump-version) + tag.
    #    Test-Clean above guarantees the only dirty paths are ours, so -A is safe
    #    and picks up README files that bump-version.ps1 rewrites.
    git add -A
    git commit -m "chore(release): $next"
    git tag -a "v$next" -m "Release v$next"
    Write-Host "Committed and tagged v$next"

    # 4. Push commit then tag (tag last so CI sees the final commit under v$next).
    if ($NoPush) {
        Write-Host "Skipping push (-NoPush). When ready: git push && git push origin v$next" -ForegroundColor Yellow
    } else {
        git push
        git push origin "v$next"
        Write-Host "Pushed to origin with tag v$next"
    }

    Write-Host ""
    Write-Host "Release v$next complete." -ForegroundColor Green
    Write-Host "Track CI: https://github.com/F000NKKK/Group-Protocol-Stack/actions"
}
finally {
    Pop-Location
}
