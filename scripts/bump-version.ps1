<#
.SYNOPSIS
  Bumps the project version atomically across every package manifest.

.DESCRIPTION
  Updates the version in:
    * Cargo.toml                  (workspace.package.version + workspace.dependencies.* path-versions)
    * csharp/GBPStack/GBPStack.csproj  (<Version>)
    * python/pyproject.toml       (project.version)
    * python/gbp_stack/__init__.py (__version__)
    * js/package.json             ("version")

  After running, review the diff, commit, tag (e.g. `git tag v1.0.0`) and
  push the tag — the release workflow handles the rest.

.PARAMETER Version
  The new SemVer version (e.g. ``1.0.0`` or ``1.0.0-rc.1``).

.EXAMPLE
  pwsh ./scripts/bump-version.ps1 -Version 1.0.0
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw "Version '$Version' is not a valid SemVer string"
}

$root = Split-Path $PSScriptRoot -Parent
function Update-File([string]$path, [scriptblock]$transform) {
    $full = Join-Path $root $path
    if (-not (Test-Path $full)) { throw "missing: $full" }
    $orig = Get-Content $full -Raw
    $new  = & $transform $orig
    if ($new -eq $orig) {
        Write-Host "  no change: $path" -ForegroundColor DarkGray
        return
    }
    # Write UTF-8 *without* BOM. PowerShell 5.1's `Set-Content -Encoding utf8`
    # silently emits a BOM that Python's tomllib (and other strict parsers)
    # refuse to read.
    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($full, $new, $utf8NoBom)
    Write-Host "  updated:   $path" -ForegroundColor Green
}

Write-Host "Bumping to $Version" -ForegroundColor Cyan

# ---- Rust workspace ----------------------------------------------------------
Update-File 'Cargo.toml' {
    param($s)
    $s = [regex]::Replace($s,
        '(?m)^(version\s*=\s*")[^"]+(")',
        "`${1}$Version`${2}",
        1)
    # Update every internal workspace dep's `version = "..."` field.
    $s = [regex]::Replace($s,
        '(path\s*=\s*"crates/[^"]+",\s*version\s*=\s*")[^"]+(")',
        "`${1}$Version`${2}")
    return $s
}

# ---- C# (NuGet) --------------------------------------------------------------
Update-File 'csharp/GBPStack/GBPStack.csproj' {
    param($s)
    return [regex]::Replace($s,
        '(?s)<Version>[^<]*</Version>',
        "<Version>$Version</Version>")
}

# ---- Python (PyPI) -----------------------------------------------------------
Update-File 'python/pyproject.toml' {
    param($s)
    return [regex]::Replace($s,
        '(?m)^(version\s*=\s*")[^"]+(")',
        "`${1}$Version`${2}")
}
Update-File 'python/gbp_stack/__init__.py' {
    param($s)
    return [regex]::Replace($s,
        '(?m)^(__version__\s*=\s*")[^"]+(")',
        "`${1}$Version`${2}")
}

# ---- JS / TS (npm) -----------------------------------------------------------
Update-File 'js/package.json' {
    param($s)
    return [regex]::Replace($s,
        '(?m)^(\s*"version"\s*:\s*")[^"]+(")',
        "`${1}$Version`${2}")
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  git diff" -ForegroundColor Yellow
Write-Host "  git add -A && git commit -m `"chore: bump to $Version`"" -ForegroundColor Yellow
Write-Host "  git tag v$Version && git push && git push origin v$Version" -ForegroundColor Yellow
