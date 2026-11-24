#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE_DIR="$ROOT/target/pgo-profile"
OPENINGS_FILE="$ROOT/versions/UHO_2024_6mvs_+085_+094.pgn"
ROUNDS_PER_TC="5"
CONCURRENCY="5"
TIME_CONTROLS="30+0.4 10+0.1"
INSTRUMENTED_BIN="$ROOT/target/release/HydraChess"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Builds HydraChess with LLVM instrumentation, profiles it with fastchess,
and rebuilds it with profile-guided optimization.

Options:
  --profile-dir PATH       Profile data directory (default: target/pgo-profile)
  --openings PATH          PGN openings book
  --rounds N               Games per time control (default: 20)
  --concurrency N          fastchess concurrency (default: 4)
  --time-controls LIST     Controls separated by spaces (default: "30+0.4 10+0.1")

Example:
  $0 --rounds 10 --concurrency 8
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile-dir)
            [[ $# -ge 2 ]] || { echo "Error: --profile-dir requires a value" >&2; exit 1; }
            PROFILE_DIR="$2"
            shift 2
            ;;
        --openings)
            [[ $# -ge 2 ]] || { echo "Error: --openings requires a value" >&2; exit 1; }
            OPENINGS_FILE="$2"
            shift 2
            ;;
        --rounds)
            [[ $# -ge 2 ]] || { echo "Error: --rounds requires a value" >&2; exit 1; }
            ROUNDS_PER_TC="$2"
            shift 2
            ;;
        --concurrency)
            [[ $# -ge 2 ]] || { echo "Error: --concurrency requires a value" >&2; exit 1; }
            CONCURRENCY="$2"
            shift 2
            ;;
        --time-controls)
            [[ $# -ge 2 ]] || { echo "Error: --time-controls requires a value" >&2; exit 1; }
            TIME_CONTROLS="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

for tool in cargo fastchess llvm-profdata; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Error: required tool '$tool' is not installed or not in PATH" >&2
        exit 1
    fi
done

if [[ ! -f "$OPENINGS_FILE" ]]; then
    echo "Error: openings file not found: $OPENINGS_FILE" >&2
    exit 1
fi

rm -rf "$PROFILE_DIR"
mkdir -p "$PROFILE_DIR"

echo "=== Building instrumented release binary ==="
CARGO_INCREMENTAL=0 RUSTFLAGS="-Cprofile-generate=$PROFILE_DIR" \
    cargo build --release

if [[ ! -x "$INSTRUMENTED_BIN" ]]; then
    echo "Error: instrumented binary was not created: $INSTRUMENTED_BIN" >&2
    exit 1
fi

echo "=== Profiling with fastchess ==="
for tc in $TIME_CONTROLS; do
    echo "Running $ROUNDS_PER_TC rounds at $tc"
    LLVM_PROFILE_FILE="$PROFILE_DIR/%p-%m.profraw" \
        fastchess \
            -engine cmd="$INSTRUMENTED_BIN" name=HydraChess-Profile-A \
            -engine cmd="$INSTRUMENTED_BIN" name=HydraChess-Profile-B \
            -each tc="$tc" option.Threads=4 \
            -openings file="$OPENINGS_FILE" format=pgn order=random \
            -rounds "$ROUNDS_PER_TC" -repeat \
            -concurrency "$CONCURRENCY"
done

mapfile -t PROFRAW_FILES < <(find "$PROFILE_DIR" -type f -name '*.profraw' -print)
if [[ ${#PROFRAW_FILES[@]} -eq 0 ]]; then
    echo "Error: fastchess did not produce any LLVM profile data" >&2
    exit 1
fi

echo "=== Merging ${#PROFRAW_FILES[@]} profile data files ==="
MERGED_PROFILE="$PROFILE_DIR/merged.profdata"
llvm-profdata merge -output="$MERGED_PROFILE" "${PROFRAW_FILES[@]}"

echo "=== Rebuilding with PGO ==="
CARGO_INCREMENTAL=0 RUSTFLAGS="-Cprofile-use=$MERGED_PROFILE" \
    cargo build --release

if [[ ! -x "$INSTRUMENTED_BIN" ]]; then
    echo "Error: PGO binary was not created: $INSTRUMENTED_BIN" >&2
    exit 1
fi

echo "=== PGO build complete ==="
echo "Profile data: $MERGED_PROFILE"
echo "PGO binary:   $INSTRUMENTED_BIN"
