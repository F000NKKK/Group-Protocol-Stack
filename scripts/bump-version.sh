#!/usr/bin/env bash
# Bumps the project version atomically across every package manifest.
#
# Updates the version in:
#   * Cargo.toml                      (workspace.package.version + workspace.dependencies.* path-versions)
#   * csharp/GBPStack/GBPStack.csproj (<Version>)
#   * python/pyproject.toml           (project.version)
#   * python/gbp_stack/__init__.py    (__version__)
#   * js/package.json                 ("version")
#   * every README.md install snippet (gbp-stack = "..", `--version ..`,
#     `pip install gbp-stack==..`, `npm install @voluntas-progressus/gbp-stack@..`,
#     `npm install @voluntas-progressus/gbp-stack-wasm@..`)
#
# After running, review the diff, commit, tag (e.g. `git tag v1.0.0`) and
# push the tag — the release workflow handles the rest.
#
# Usage:
#   ./scripts/bump-version.sh -v 1.0.0
#   ./scripts/bump-version.sh --version 1.0.0-rc.1

set -euo pipefail

usage() {
    echo "Usage: $0 -v|--version <semver>" >&2
    exit 1
}

VERSION=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--version) VERSION="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown argument: $1" >&2; usage ;;
    esac
done
[[ -n "$VERSION" ]] || usage

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
    echo "Version '$VERSION' is not a valid SemVer string" >&2
    exit 1
fi
if [[ "$VERSION" == *. ]]; then
    echo "Version '$VERSION' has a trailing dot" >&2
    exit 1
fi

# PyPI / pyproject.toml requires PEP 440. SemVer's `1.0.0-rc1` is not PEP 440;
# the canonical equivalent is `1.0.0rc1` (no hyphen). Stable versions are
# unchanged.
PY_VERSION="$(perl -pe 's/-(rc|a|b|alpha|beta)(\d+)$/$1$2/' <<<"$VERSION")"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

update_file() {
    local path="$1"
    local full="$ROOT/$path"
    [[ -f "$full" ]] || { echo "missing: $full" >&2; exit 1; }
    local before
    before="$(cat "$full")"
    shift
    "$@" "$full"
    if [[ "$(cat "$full")" == "$before" ]]; then
        echo "  no change: $path"
    else
        echo "  updated:   $path"
    fi
}

echo "Bumping to $VERSION"

# ---- Rust workspace ----------------------------------------------------------
update_file Cargo.toml perl -0777 -i -pe '
    s/^(version\s*=\s*")[^"]+(")/${1}'"$VERSION"'${2}/m;
    s/(path\s*=\s*"crates\/[^"]+",\s*version\s*=\s*")[^"]+(")/${1}'"$VERSION"'${2}/g;
'

# ---- C# (NuGet) --------------------------------------------------------------
update_file csharp/GBPStack/GBPStack.csproj perl -0777 -i -pe \
    's/<Version>[^<]*<\/Version>/<Version>'"$VERSION"'<\/Version>/s'

# ---- Python (PyPI) — PEP 440 form (no hyphen before rc/a/b) ------------------
update_file python/pyproject.toml perl -i -pe \
    's/^(version\s*=\s*")[^"]+(")/${1}'"$PY_VERSION"'${2}/'
update_file python/gbp_stack/__init__.py perl -i -pe \
    's/^(__version__\s*=\s*")[^"]+(")/${1}'"$PY_VERSION"'${2}/'

# ---- JS / TS (npm) -----------------------------------------------------------
update_file js/package.json perl -i -pe \
    's/^(\s*"version"\s*:\s*")[^"]+(")/${1}'"$VERSION"'${2}/'

# ---- README.md install snippets ---------------------------------------------
# Every README that documents an install command carries the version inline.
# Patterns we update (each uniquely identifies one registry):
#   * Cargo:  gbp-stack = "X.Y.Z"          (or any of the gbp-* / *-protocol crates)
#   * NuGet:  --version X.Y.Z              (after `dotnet add package GBPStack`)
#   * PyPI:   pip install gbp-stack==X.Y.Z
#   * npm:    @voluntas-progressus/gbp-stack@X.Y.Z
#   * npm:    @voluntas-progressus/gbp-stack-wasm@X.Y.Z
CRATE_NAMES=(gbp-stack gbp-core gbp-protocol gbp-mls gbp-transport gbp-node gbp-stack-ffi gbp-cli gtp-protocol gap-protocol gsp-protocol)
CRATE_ALT="$(IFS='|'; echo "${CRATE_NAMES[*]}")"

while IFS= read -r -d '' readme; do
    rel="${readme#"$ROOT"/}"
    update_file "$rel" perl -0777 -i -pe '
        s/^(\s*(?:'"$CRATE_ALT"')\s*=\s*")[^"]+(")/${1}'"$VERSION"'${2}/mg;
        s/(GBPStack\s+--version\s+)[0-9A-Za-z.+-]+/${1}'"$VERSION"'/g;
        s/(pip\s+install\s+gbp-stack==)[0-9A-Za-z.+-]+/${1}'"$PY_VERSION"'/g;
        s/(\@voluntas-progressus\/gbp-stack\@)[0-9A-Za-z.+-]+/${1}'"$VERSION"'/g;
        s/(\@voluntas-progressus\/gbp-stack-wasm\@)[0-9A-Za-z.+-]+/${1}'"$VERSION"'/g;
    '
done < <(find "$ROOT" -type d \( -name node_modules -o -name target -o -name .git \) -prune -o -type f -name 'README.md' -print0)

echo ""
echo "Next steps:"
echo "  git diff"
echo "  git add -A && git commit -m \"chore: bump to $VERSION\""
echo "  git tag v$VERSION && git push && git push origin v$VERSION"
