#!/bin/bash
set -euo pipefail

# Default SPRT parameters
ELO0=0
ELO1=5
ALPHA=0.05
BETA=0.1

# Usage function
usage() {
    cat <<EOF
Usage: $0 <commit-hash> [--elo0=X] [--elo1=Y] [--alpha=A] [--beta=B]

Runs SPRT test between HEAD (Latest) and a given commit (Prev).

Parameters:
  <commit-hash>   Git commit hash to test against (Prev)
  --elo0=X        SPRT elo0 (default: 0)
  --elo1=Y        SPRT elo1 (default: 5)
  --alpha=A       SPRT alpha (default: 0.05)
  --beta=B        SPRT beta (default: 0.1)

Example:
  $0 abc1234 --elo0=0 --elo1=5 --alpha=0.05 --beta=0.1
EOF
    exit 1
}

# Parse arguments
if [ $# -lt 1 ]; then
    usage
fi

COMMIT_HASH="$1"
shift

while [ $# -gt 0 ]; do
    case "$1" in
        --elo0=*) ELO0="${1#*=}" ;;
        --elo1=*) ELO1="${1#*=}" ;;
        --alpha=*) ALPHA="${1#*=}" ;;
        --beta=*) BETA="${1#*=}" ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
    shift
done

# Validate commit exists
if ! git cat-file -e "$COMMIT_HASH" 2>/dev/null; then
    echo "Error: Commit $COMMIT_HASH not found"
    exit 1
fi

echo "=== SPRT Test Configuration ==="
echo "Prev commit: $COMMIT_HASH"
echo "SPRT params: elo0=$ELO0 elo1=$ELO1 alpha=$ALPHA beta=$BETA"
echo "==============================="

# Store current commit (HEAD) for later
HEAD_COMMIT=$(git rev-parse HEAD)
echo "HEAD commit: $HEAD_COMMIT"

# Build directory for Prev
PREV_BIN="./target/release/HydraChessPrev"

# Step 1: Checkout the old commit and build Prev
echo "=== Building Prev ($COMMIT_HASH) ==="
git checkout "$COMMIT_HASH" --quiet
cargo build --release
cp "./target/release/HydraChess" "$PREV_BIN"
echo "Prev binary copied to $PREV_BIN"

# Step 2: Return to HEAD and build Latest
echo "=== Building Latest ($HEAD_COMMIT) ==="
git checkout - --quiet
cargo build --release
echo "Latest binary at ./target/release/HydraChess"

# Step 3: Run fastchess
OPENINGS_FILE="./versions/UHO_2024_6mvs_+085_+094.pgn"
if [ ! -f "$OPENINGS_FILE" ]; then
    echo "Error: Openings file not found: $OPENINGS_FILE"
    exit 1
fi

echo "=== Running fastchess SPRT ==="
fastchess \
    -engine cmd=./target/release/HydraChess name=HydraChessLatest \
    -engine cmd="$PREV_BIN" name=HydraChessPrev \
    -each tc=10+0.1 \
    -openings file="$OPENINGS_FILE" format=pgn order=random \
    -rounds 40000 -repeat \
    -ratinginterval 5 \
    -sprt elo0=$ELO0 elo1=$ELO1 alpha=$ALPHA beta=$BETA \
    -report penta=true \
    -concurrency 20

echo "=== SPRT test complete ==="