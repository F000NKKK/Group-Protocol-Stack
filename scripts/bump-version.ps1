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
    * every README.md install snippet (gbp-stack = "..", `--version ..`,
      `pip install gbp-stack==..`, `npm install @voluntas-progressus/gbp-stack@..`)

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
if ($Version.EndsWith('.')) {
    throw "Version '$Version' has a trailing dot"
}

# PyPI / pyproject.toml requires PEP 440. SemVer's `1.0.0-rc1` is not PEP 440;
# the canonical equivalent is `1.0.0rc1` (no hyphen). Stable versions are
# unchanged.
$PythonVersion = $Version -replace '-(rc|a|b|alpha|beta)(\d+)$', '$1$2'

$root = Split-Path $PSScriptRoot -Parent
function Update-File([string]$path, [scriptblock]$transform) {
    $full = Join-Path $root $path
    if (-not (Test-Path $full)) { throw "missing: $full" }
    # Read explicitly as UTF-8. PS 5.1's `Get-Content -Raw` falls back to the
    # system code page (e.g. cp1251 on RU Windows) and mangles em-dashes /
    # box-drawing characters in the README files.
    $orig = [System.IO.File]::ReadAllText($full, [System.Text.UTF8Encoding]::new($false))
    $new  = & $transform $orig
    if ($new -eq $orig) {
        Write-Host "  no change: $path" -ForegroundColor DarkGray
        return
    }
    # Write UTF-8 *without* BOM. PowerShell 5.1's `Set-Content -Encoding utf8`
    # emits a BOM that Python's tomllib (and other strict parsers) refuse.
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

# ---- Python (PyPI) — PEP 440 form (no hyphen before rc/a/b) ------------------
Update-File 'python/pyproject.toml' {
    param($s)
    return [regex]::Replace($s,
        '(?m)^(version\s*=\s*")[^"]+(")',
        "`${1}$PythonVersion`${2}")
}
Update-File 'python/gbp_stack/__init__.py' {
    param($s)
    return [regex]::Replace($s,
        '(?m)^(__version__\s*=\s*")[^"]+(")',
        "`${1}$PythonVersion`${2}")
}

# ---- JS / TS (npm) -----------------------------------------------------------
Update-File 'js/package.json' {
    param($s)
    return [regex]::Replace($s,
        '(?m)^(\s*"version"\s*:\s*")[^"]+(")',
        "`${1}$Version`${2}")
}

# ---- README.md install snippets ---------------------------------------------
# Every README that documents an install command carries the version inline.
# Patterns we update (each uniquely identifies one registry):
#   * Cargo:  gbp-stack = "X.Y.Z"          (or any of the gbp-* / *-protocol crates)
#   * NuGet:  --version X.Y.Z              (after `dotnet add package GBPStack`)
#   * PyPI:   pip install gbp-stack==X.Y.Z
#   * npm:    @voluntas-progressus/gbp-stack@X.Y.Z
$readmes = Get-ChildItem -Path $root -Recurse -Filter 'README.md' `
    -Exclude 'node_modules', 'target' |
    Where-Object {
        $_.FullName -notmatch '\\(node_modules|target|\.git)\\'
    }

$cratePackageNames = @(
    'gbp-stack', 'gbp-core', 'gbp-protocol', 'gbp-mls', 'gbp-transport',
    'gbp-node', 'gbp-stack-ffi', 'gbp-cli',
    'gtp-protocol', 'gap-protocol', 'gsp-protocol'
)

foreach ($r in $readmes) {
    # PS 5.1 has no [Path]::GetRelativePath — strip the root prefix manually.
    $rel = $r.FullName.Substring($root.Length).TrimStart('\', '/').Replace('\', '/')
    Update-File $rel {
        param($s)

        # Cargo: `<crate> = "X.Y.Z"` (TOML lines inside fenced code blocks).
        foreach ($cn in $cratePackageNames) {
            $escaped = [regex]::Escape($cn)
            $s = [regex]::Replace($s,
                "(?m)^(\s*$escaped\s*=\s*"")[^""]+("")",
                "`${1}$Version`${2}")
        }

        # NuGet: `dotnet add package GBPStack --version X.Y.Z`
        $s = [regex]::Replace($s,
            '(GBPStack\s+--version\s+)[0-9A-Za-z.+-]+',
            "`${1}$Version")

        # PyPI: `pip install gbp-stack==X.Y.Z` (PEP 440 form)
        $s = [regex]::Replace($s,
            '(pip\s+install\s+gbp-stack==)[0-9A-Za-z.+-]+',
            "`${1}$PythonVersion")

        # npm: `@voluntas-progressus/gbp-stack@X.Y.Z`
        $s = [regex]::Replace($s,
            '(@voluntas-progressus/gbp-stack@)[0-9A-Za-z.+-]+',
            "`${1}$Version")

        return $s
    }
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  git diff" -ForegroundColor Yellow
Write-Host "  git add -A && git commit -m `"chore: bump to $Version`"" -ForegroundColor Yellow
Write-Host "  git tag v$Version && git push && git push origin v$Version" -ForegroundColor Yellow
