#!/usr/bin/env bash
# Cuts a release: bumps the version across every package manifest, updates
# CHANGELOG.md from conventional commits, regenerates the SECURITY.md
# supported-versions table, commits, tags and pushes.
#
# After this script returns, the GitHub Actions release workflow will see
# the new tag and publish to crates.io, NuGet, PyPI, npm and GitHub Releases.
#
# The "current version" is read from Cargo.toml (workspace.package.version);
# every other manifest is updated to match via scripts/bump-version.sh.
#
# SECURITY.md is regenerated automatically:
#   * Only the latest minor series (latest patch only) is marked supported.
#   * Any annotated tag whose message contains "deprecated" or "eol" is
#     explicitly forced to unsupported, even if policy would support it.
#   * All other stable tags are marked unsupported.
#
# To mark a released patch as deprecated without releasing a new version:
#   git tag -a -f v1.2.1 -m "deprecated: superseded by v1.2.2"
#   git push origin v1.2.1 --force-with-lease
#   ./scripts/release.sh --bump patch --no-push   # just regenerate SECURITY.md
#
# Usage:
#   ./scripts/release.sh --bump patch
#   ./scripts/release.sh --bump minor
#   ./scripts/release.sh --version 1.0.0
#   ./scripts/release.sh --bump patch --no-push

set -euo pipefail

BUMP="patch"
VERSION=""
NO_PUSH=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bump) BUMP="$2"; shift 2 ;;
        --version) VERSION="$2"; shift 2 ;;
        --no-push) NO_PUSH=1; shift ;;
        -h|--help)
            echo "Usage: $0 [--bump patch|minor|major|rc] [--version X.Y.Z] [--no-push]" >&2
            exit 1
            ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

get_current_version() {
    perl -ne 'if (/^version\s*=\s*"([^"]+)"/) { print $1; exit }' Cargo.toml
}

step_version() {
    local current="$1" bump="$2"
    if [[ ! "$current" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-(.+))?$ ]]; then
        echo "current version '$current' is not SemVer" >&2
        exit 1
    fi
    local maj="${BASH_REMATCH[1]}" min="${BASH_REMATCH[2]}" pat="${BASH_REMATCH[3]}" pre="${BASH_REMATCH[5]:-}"
    case "$bump" in
        major) echo "$((maj + 1)).0.0" ;;
        minor) echo "$maj.$((min + 1)).0" ;;
        patch)
            if [[ -n "$pre" ]]; then echo "$maj.$min.$pat"; else echo "$maj.$min.$((pat + 1))"; fi
            ;;
        rc)
            if [[ "$pre" =~ ^rc([0-9]+)$ ]]; then
                echo "$maj.$min.$pat-rc$((BASH_REMATCH[1] + 1))"
            else
                echo "$maj.$min.$((pat + 1))-rc1"
            fi
            ;;
        *) echo "unknown bump '$bump'" >&2; exit 1 ;;
    esac
}

test_clean() {
    local st
    st="$(git status --porcelain)"
    if [[ -n "$st" ]]; then
        echo "Working tree is not clean:" >&2
        echo "$st" >&2
        echo "commit or stash changes first" >&2
        exit 1
    fi
}

new_changelog_entry() {
    local version="$1" range="$2"
    local today
    today="$(date +%Y-%m-%d)"
    local out
    out="## $version ($today)"$'\n\n'
    local any=0
    declare -A titles=(
        [feat]="### Features" [fix]="### Bug Fixes" [perf]="### Performance"
        [refactor]="### Refactoring" [test]="### Tests" [docs]="### Documentation"
        [build]="### Build" [ci]="### CI" [chore]="### Chores" [style]="### Style"
    )
    local order=(feat fix perf refactor test docs build ci chore style)
    for type in "${order[@]}"; do
        local commits
        commits="$(git log "$range" --no-merges --pretty=format:"- %s (%h)" --grep "^$type" 2>/dev/null || true)"
        if [[ -n "$commits" ]]; then
            out+="${titles[$type]}"$'\n\n'
            out+="$commits"$'\n\n'
            any=1
        fi
    done
    if [[ "$any" -eq 0 ]]; then
        out+="_No conventional commits found in this range._"$'\n\n'
    fi
    printf '%s' "$out"
}

update_changelog() {
    local entry="$1"
    local path="$ROOT/CHANGELOG.md"
    local tmp
    tmp="$(mktemp)"
    if [[ -f "$path" ]]; then
        { printf '%s' "$entry"; printf -- '---\n\n'; cat "$path"; } > "$tmp"
    else
        { printf '# Changelog\n\n'; printf '%s' "$entry"; } > "$tmp"
    fi
    mv "$tmp" "$path"
}

# Rebuilds the | Version | Supported | table in SECURITY.md from git tags.
#
# Policy:
#   - Latest patch of the most-recent minor series -> supported.
#   - Any annotated tag whose message contains "deprecated"/"eol"/"end of life"
#     -> unsupported (overrides policy even for the latest patch).
#   - Everything else -> unsupported.
update_security_policy() {
    local new_version="$1"
    echo "Updating SECURITY.md supported-versions table..."

    local all_tags=()
    while IFS= read -r t; do [[ -n "$t" ]] && all_tags+=("$t"); done \
        < <(git tag -l | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' || true)

    local injected="v$new_version"
    local found=0
    for t in "${all_tags[@]}"; do [[ "$t" == "$injected" ]] && found=1; done
    [[ "$found" -eq 0 ]] && all_tags=("$injected" "${all_tags[@]}")

    # Sort descending by maj*1e6 + min*1e3 + patch.
    local sorted=()
    while IFS= read -r t; do sorted+=("$t"); done < <(
        for t in "${all_tags[@]}"; do
            IFS='.' read -r maj min pat <<<"${t#v}"
            printf '%d %s\n' "$((10#$maj * 1000000 + 10#$min * 1000 + 10#$pat))" "$t"
        done | sort -k1,1nr | awk '{print $2}'
    )

    # Determine which tags are deprecated/eol via annotated-tag message.
    declare -A deprecated=()
    for t in "${sorted[@]}"; do
        [[ "$t" == "$injected" ]] && continue  # not created in git yet
        local obj_type
        obj_type="$(git for-each-ref "refs/tags/$t" --format='%(objecttype)' 2>/dev/null || true)"
        [[ "$obj_type" == "tag" ]] || continue
        local msg
        msg="$(git for-each-ref "refs/tags/$t" --format='%(contents)' 2>/dev/null || true)"
        if grep -qiE '\b(deprecated|eol|end.of.life)\b' <<<"$msg"; then
            local tag_minor="${t#v}"; tag_minor="${tag_minor%.*}"
            local msg_minor
            msg_minor="$(grep -oE '[0-9]+\.[0-9]+[.x]' <<<"$msg" | head -1 | grep -oE '^[0-9]+\.[0-9]+' || true)"
            if [[ -z "$msg_minor" || "$msg_minor" == "$tag_minor" ]]; then
                deprecated["$t"]=1
            fi
        fi
    done

    # Group by major.minor — first non-deprecated tag per minor, in descending
    # order, is the supported candidate. Only the single latest minor is kept.
    declare -A latest_by_minor=()
    local minor_order=()
    for t in "${sorted[@]}"; do
        local key="${t#v}"; key="${key%.*}"
        if [[ -z "${latest_by_minor[$key]:-}" && -z "${deprecated[$t]:-}" ]]; then
            latest_by_minor["$key"]="$t"
            minor_order+=("$key")
        fi
    done

    local lines=()
    lines+=("| Version      | Supported          |")
    lines+=("| ------------ | ------------------ |")

    local oldest_supported=""
    if [[ ${#minor_order[@]} -gt 0 ]]; then
        local minor_key="${minor_order[0]}"
        local supported_tag="${latest_by_minor[$minor_key]}"
        local supported_ver="${supported_tag#v}"
        IFS='.' read -r maj min pat <<<"$supported_ver"

        local has_deprecated_above=0
        for t in "${sorted[@]}"; do
            if [[ "$t" =~ ^v${maj}\.${min}\.([0-9]+)$ ]]; then
                local p="${BASH_REMATCH[1]}"
                if [[ "$p" -gt "$pat" && -n "${deprecated[$t]:-}" ]]; then
                    has_deprecated_above=1
                fi
            fi
        done

        local display
        if [[ "$has_deprecated_above" -eq 1 ]]; then
            display="$supported_ver"
        else
            display="$maj.$min.x"
        fi
        lines+=("$(printf '| %-12s | :white_check_mark: |' "$display")")
        oldest_supported="$supported_ver"
    fi

    if [[ -n "$oldest_supported" ]]; then
        lines+=("$(printf '| < %-10s | :x:                |' "$oldest_supported")")
    fi

    local path="$ROOT/SECURITY.md"
    local table
    table="$(printf '%s\n' "${lines[@]}")"
    local before after
    before="$(cat "$path")"

    local table_file
    table_file="$(mktemp)"
    printf '%s\n' "$table" > "$table_file"
    after="$(TABLE_FILE="$table_file" perl -0777 -pe '
        BEGIN { local $/; open(my $fh, "<", $ENV{TABLE_FILE}) or die $!; $t = <$fh>; }
        s/^\| Version\s*\|[^\n]*\r?\n(?:\|[^\n]*\r?\n)+/$t/m;
    ' "$path")"
    rm -f "$table_file"

    if [[ "$after" == "$before" ]]; then
        echo "  WARNING: SECURITY.md table pattern not found - file unchanged"
    else
        printf '%s' "$after" > "$path"
        echo "  updated: SECURITY.md"
    fi
}

# --- main ---------------------------------------------------------------------

test_clean

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" && "$BRANCH" != "master" ]]; then
    echo "WARNING: releasing from branch '$BRANCH'"
fi

CURRENT="$(get_current_version)"
if [[ -n "$VERSION" ]]; then
    NEXT="${VERSION#v}"
    NEXT="${NEXT%.}"
else
    NEXT="$(step_version "$CURRENT" "$BUMP")"
fi
if [[ ! "$NEXT" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-rc[0-9]+)?$ ]]; then
    echo "next version '$NEXT' is not a clean SemVer (expected MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-rcN)" >&2
    exit 1
fi
if [[ "$NEXT" == "$CURRENT" ]]; then
    echo "next version equals current ($CURRENT); nothing to do" >&2
    exit 1
fi

echo "Releasing $CURRENT -> $NEXT"

# 1. Bump every manifest.
"$ROOT/scripts/bump-version.sh" --version "$NEXT"

# 2. Changelog.
LAST_TAG="$(git describe --tags --abbrev=0 2>/dev/null || true)"
RANGE="HEAD"
[[ -n "$LAST_TAG" ]] && RANGE="$LAST_TAG..HEAD"
ENTRY="$(new_changelog_entry "$NEXT" "$RANGE")"
update_changelog "$ENTRY"
echo "CHANGELOG.md updated"

# 3. Regenerate lock files so they reflect the new version in package.json / Cargo.toml.
echo "Regenerating lock files..."
(cd "$ROOT/js" && npm install --prefer-offline >/dev/null 2>&1) \
    && echo "  updated: js/package-lock.json" \
    || echo "  WARNING: npm install failed"
(cd "$ROOT" && cargo update --workspace --quiet)
echo "  updated: Cargo.lock"

# 4. Regenerate SECURITY.md supported-versions table.
#    Called before `git add -A` so the updated file is included in the commit.
update_security_policy "$NEXT"

# 5. Commit (manifests + READMEs + CHANGELOG + SECURITY.md + lock files) + annotated tag.
git add -A
git commit -m "chore(release): $NEXT"
git tag -a "v$NEXT" -m "Release v$NEXT"
echo "Committed and tagged v$NEXT"

# 6. Push commit then tag (tag last so CI sees the final commit under v$NEXT).
if [[ "$NO_PUSH" -eq 1 ]]; then
    echo "Skipping push (--no-push). When ready: git push && git push origin v$NEXT"
else
    git push
    git push origin "v$NEXT"
    echo "Pushed to origin with tag v$NEXT"
fi

echo ""
echo "Release v$NEXT complete."
echo "Track CI: https://github.com/F000NKKK/Group-Protocol-Stack/actions"
