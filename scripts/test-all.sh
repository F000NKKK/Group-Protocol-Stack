#!/usr/bin/env bash
# Build the native library, then run integration tests for all three bindings
# (Python, C#, JavaScript).
#
# Usage:
#   ./scripts/test-all.sh
#   ./scripts/test-all.sh --release
#   ./scripts/test-all.sh --skip csharp,js

set -euo pipefail

RELEASE=0
SKIP=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) RELEASE=1; shift ;;
        --skip) SKIP="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--release] [--skip python,csharp,js]" >&2
            exit 1
            ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

is_skipped() {
    local name="$1"
    IFS=',' read -ra parts <<<"$(tr '[:upper:]' '[:lower:]' <<<"$SKIP")"
    for p in "${parts[@]}"; do
        [[ "$(echo "$p" | xargs)" == "$name" ]] && return 0
    done
    return 1
}

FAILURES=()

# ── Step 1: Build and distribute the native library ─────────────────────────
echo "=== Building native library ==="
INSTALL_ARGS=()
[[ "$RELEASE" -eq 1 ]] && INSTALL_ARGS+=(--release)
"$ROOT/scripts/install-native.sh" "${INSTALL_ARGS[@]}"

# ── Step 2: Python tests ─────────────────────────────────────────────────────
if ! is_skipped python; then
    echo ""
    echo "=== Python tests ==="
    (cd "$ROOT/python" && python -m pytest tests/test_integration.py -v --tb=short) \
        || FAILURES+=("Python")
fi

# ── Step 3: C# tests ─────────────────────────────────────────────────────────
if ! is_skipped csharp; then
    echo ""
    echo "=== C# tests ==="
    (cd "$ROOT" && dotnet test csharp/GBPStack.Tests/GBPStack.Tests.csproj --logger "console;verbosity=normal") \
        || FAILURES+=("C#")
fi

# ── Step 4: JavaScript tests ──────────────────────────────────────────────────
if ! is_skipped js; then
    echo ""
    echo "=== JavaScript tests ==="
    (cd "$ROOT/js" && npm test) \
        || FAILURES+=("JavaScript")
fi

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
if [[ ${#FAILURES[@]} -eq 0 ]]; then
    echo "All tests passed."
else
    echo "FAILED: $(IFS=', '; echo "${FAILURES[*]}")"
    exit 1
fi
