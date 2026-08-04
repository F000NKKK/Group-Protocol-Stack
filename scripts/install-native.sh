#!/usr/bin/env bash
# Build gbp-stack-ffi and distribute the native shared library to all
# language-binding directories so local test runs pick it up without
# manual copying.
#
# Usage:
#   ./scripts/install-native.sh
#   ./scripts/install-native.sh --release
#   ./scripts/install-native.sh --release --target-dir /path/to/target

set -euo pipefail

RELEASE=0
TARGET_DIR=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release) RELEASE=1; shift ;;
        --target-dir) TARGET_DIR="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--release] [--target-dir <dir>]" >&2
            exit 1
            ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── 1. Resolve RID + artifact name for this host ────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        ARTIFACT="libgbp_stack.so"
        case "$ARCH" in
            x86_64) RID="linux-x64" ;;
            aarch64|arm64) RID="linux-arm64" ;;
            *) echo "Unsupported Linux architecture: $ARCH" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        ARTIFACT="libgbp_stack.dylib"
        case "$ARCH" in
            x86_64) RID="osx-x64" ;;
            arm64) RID="osx-arm64" ;;
            *) echo "Unsupported macOS architecture: $ARCH" >&2; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS (use install-native.ps1 on Windows)" >&2
        exit 1
        ;;
esac

# ── 2. Cargo build ───────────────────────────────────────────────────────────
PROFILE="debug"
CARGO_ARGS=(build -p gbp-stack-ffi)
if [[ "$RELEASE" -eq 1 ]]; then
    PROFILE="release"
    CARGO_ARGS+=(--release)
fi

echo "cargo ${CARGO_ARGS[*]}"
(cd "$ROOT" && cargo "${CARGO_ARGS[@]}")

# ── 3. Resolve shared-library path ──────────────────────────────────────────
TARGET_BASE="${TARGET_DIR:-$ROOT/target}"
SRC_LIB="$TARGET_BASE/$PROFILE/$ARTIFACT"

if [[ ! -f "$SRC_LIB" ]]; then
    echo "Library not found: $SRC_LIB" >&2
    exit 1
fi
echo "Built: $SRC_LIB"

# ── 4. Copy to binding directories ──────────────────────────────────────────
DESTINATIONS=(
    "$ROOT/python/gbp_stack/_native/$RID/$ARTIFACT"
    "$ROOT/csharp/GBPStack/runtimes/$RID/native/$ARTIFACT"
    "$ROOT/js/native/$RID/$ARTIFACT"
)

for dst in "${DESTINATIONS[@]}"; do
    mkdir -p "$(dirname "$dst")"
    cp -f "$SRC_LIB" "$dst"
    echo "  -> $dst"
done

echo ""
echo "Done. Native library installed for all bindings ($RID)."
