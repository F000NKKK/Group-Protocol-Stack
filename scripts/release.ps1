<#
.SYNOPSIS
  Cuts a release: bumps the version across every package manifest, updates
  CHANGELOG.md from conventional commits, regenerates the SECURITY.md
  supported-versions table, commits, tags and pushes.

.DESCRIPTION
  After this script returns, the GitHub Actions release workflow will see
  the new tag and publish to crates.io, NuGet, PyPI, npm and GitHub
  Releases.

  The "current version" is read from Cargo.toml (workspace.package.version);
  every other manifest is updated to match via scripts/bump-version.ps1.

  SECURITY.md is regenerated automatically:
    * The two most recent minor series (latest patch only) are marked supported.
    * Any annotated tag whose message contains "deprecated" or "eol" is
      explicitly forced to unsupported, even if policy would support it.
    * All other stable tags are marked unsupported.

  To mark a released patch as deprecated without releasing a new version:
    git tag -a -f v1.2.1 -m "deprecated: superseded by v1.2.2"
    git push origin v1.2.1 --force-with-lease
    pwsh ./scripts/release.ps1 -Bump patch -NoPush   # just regenerate SECURITY.md
    # or run Update-SecurityPolicy manually and commit

.PARAMETER Bump
  patch | minor | major | rc — relative bump (cannot be combined with -Version).
  rc creates / increments a -rcN pre-release on the *next* patch.

.PARAMETER Version
  Exact SemVer to release (e.g. 1.0.0, 1.0.0-rc1). Wins over -Bump.

.PARAMETER NoPush
  Build the commit and tag locally but do not push.

.EXAMPLE
  pwsh ./scripts/release.ps1 -Bump patch
  pwsh ./scripts/release.ps1 -Bump minor
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
    # CHANGELOG.md uses UTF-8 with BOM for maximum Windows tooling compatibility.
    # Code manifests use no-BOM (via bump-version.ps1) to avoid breaking parsers.
    Set-Content -Path $path -Value $new -Encoding utf8
}

# Rebuilds the | Version | Supported | table in SECURITY.md from git tags.
#
# Policy:
#   - Latest patch of the two most-recent minor series → supported.
#   - Any annotated tag whose message contains "deprecated" or "eol" → unsupported
#     (overrides policy even for the latest patch).
#   - Everything else → unsupported.
#
# $newVersion — the version being released (tag doesn't exist in git yet at
#               call time, so we inject it into the list manually).
function Update-SecurityPolicy([string]$newVersion) {
    Write-Host "Updating SECURITY.md supported-versions table..." -ForegroundColor Cyan

    # Collect all stable release tags (vMAJOR.MINOR.PATCH, no pre-release suffix).
    $allTags = @(git tag -l | Where-Object { $_ -match '^v\d+\.\d+\.\d+$' })

    # Inject the version being released before the tag exists in git.
    if ($newVersion -and "v$newVersion" -notin $allTags) {
        $allTags = @("v$newVersion") + $allTags
    }

    # Sort descending so the highest version comes first.
    $sorted = $allTags | Sort-Object {
        $v = ($_ -replace '^v', '').Split('.')
        [long]$v[0] * 1000000L + [long]$v[1] * 1000L + [long]$v[2]
    } -Descending

    # Detect deprecated/eol tags via annotation message.
    $deprecatedSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    foreach ($tag in $sorted) {
        $objType = git for-each-ref "refs/tags/$tag" --format='%(objecttype)' 2>$null
        if ($objType -eq 'tag') {
            $msg = git for-each-ref "refs/tags/$tag" --format='%(contents)' 2>$null
            if (($msg -join ' ') -match '\b(deprecated|eol|end.of.life)\b') {
                [void]$deprecatedSet.Add($tag)
            }
        }
    }

    # Group by major.minor — latest NON-deprecated patch per minor is the supported candidate.
    # If the absolute latest patch is deprecated, fall back to the next non-deprecated patch.
    $latestByMinor = [ordered]@{}
    foreach ($tag in $sorted) {
        if ($tag -match '^v(\d+)\.(\d+)\.') {
            $key = "$($Matches[1]).$($Matches[2])"
            if (-not $latestByMinor.Contains($key) -and -not $deprecatedSet.Contains($tag)) {
                $latestByMinor[$key] = $tag
            }
        }
    }

    # Top 2 minors (latest non-deprecated patch each) = supported.
    $supportedMinors = @($latestByMinor.Keys | Select-Object -First 2)

    # Build compact semver-range table.
    # For each supported minor:
    #   - If no deprecated patch exists above the supported one → show "X.Y.x" (whole minor)
    #   - Otherwise show the exact supported patch "X.Y.Z" (some patches are deprecated)
    # Then a single catch-all "< oldest_supported_version" → unsupported.
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add('| Version      | Supported          |')
    $lines.Add('| ------------ | ------------------ |')

    $oldestSupportedVer = $null
    foreach ($minorKey in $supportedMinors) {
        $supportedTag = $latestByMinor[$minorKey]
        $supportedVer = $supportedTag -replace '^v', ''
        $parts = $supportedVer.Split('.')
        $maj = $parts[0]; $min = $parts[1]; $pat = [int]$parts[2]

        # Check if any higher-patched tag in this minor is deprecated.
        $hasDeprecatedAbove = $sorted | Where-Object {
            $_ -match "^v$maj\.$min\.(\d+)$" -and
            [int]$Matches[1] -gt $pat -and
            $deprecatedSet.Contains($_)
        }

        $display = if ($hasDeprecatedAbove) { $supportedVer } else { "$maj.$min.x" }
        $lines.Add("| $($display.PadRight(12)) | :white_check_mark: |")
        $oldestSupportedVer = $supportedVer
    }

    # Catch-all for everything older.
    if ($oldestSupportedVer) {
        $lines.Add("| < $($oldestSupportedVer.PadRight(10)) | :x:                |")
    }

    # Replace the existing table block in SECURITY.md.
    # Matches: "| Version |..." header line + divider line + all "|...|" rows.
    $path    = Join-Path $root 'SECURITY.md'
    $md      = [System.IO.File]::ReadAllText($path, [System.Text.UTF8Encoding]::new($false))
    $newTable = ($lines -join "`n") + "`n"
    $before = $md
    $md = [regex]::Replace($md,
        '(?m)^\| Version\s*\|[^\n]*\r?\n(?:\|[^\n]*\r?\n)+',
        $newTable)
    [System.IO.File]::WriteAllText($path, $md, [System.Text.UTF8Encoding]::new($false))
    if ($md -eq $before) { Write-Host "  WARNING: SECURITY.md table pattern not found — file unchanged" -ForegroundColor Yellow }
    else { Write-Host "  updated: SECURITY.md" -ForegroundColor Green }
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

    # 1. Bump every manifest.
    & (Join-Path $PSScriptRoot 'bump-version.ps1') -Version $next

    # 2. Changelog.
    $lastTag = git describe --tags --abbrev=0 2>$null
    $range = if ($LASTEXITCODE -eq 0 -and $lastTag) { "$lastTag..HEAD" } else { 'HEAD' }
    $LASTEXITCODE = 0
    $entry = New-ChangelogEntry $next $range
    Update-Changelog $entry
    Write-Host "CHANGELOG.md updated"

    # 3. Regenerate SECURITY.md supported-versions table.
    #    Called before `git add -A` so the updated file is included in the commit.
    Update-SecurityPolicy $next

    # 4. Commit (manifests + READMEs + CHANGELOG + SECURITY.md) + annotated tag.
    git add -A
    git commit -m "chore(release): $next"
    git tag -a "v$next" -m "Release v$next"
    Write-Host "Committed and tagged v$next"

    # 5. Push commit then tag (tag last so CI sees the final commit under v$next).
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
