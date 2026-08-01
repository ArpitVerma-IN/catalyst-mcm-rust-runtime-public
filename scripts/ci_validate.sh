#!/usr/bin/env bash
# ci_validate.sh — Single-command CI validation for the MCM Runtime.
#
# Runs every quality gate in sequence:
#   1. cargo fmt --check (formatting)
#   2. cargo clippy (linting)
#   3. cargo test (Rust unit tests)
#   4. cargo doc --no-deps (documentation)
#   5. pytest (Python integration tests)
#   6. (optional) memory safety validation
#
# Exit code 0 = all checks passed.
# Any non-zero exit immediately stops the pipeline.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend_rust"
FRONTEND_DIR="$PROJECT_ROOT/frontend_python"

# Color output helpers
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

pass() { echo -e "  ${GREEN}✓${NC} $1"; }
fail() { echo -e "  ${RED}✗${NC} $1"; exit 1; }

echo "============================================================"
echo "  Catalyst MCM Runtime — CI Validation"
echo "============================================================"

source ~/.cargo/env

# ── Step 1: Rust Formatting ──
echo ""
echo "[Step 1/5] Checking Rust formatting..."
cd "$BACKEND_DIR"
if cargo fmt --check 2>&1; then
    pass "Rust formatting OK"
else
    fail "Rust formatting errors detected. Run 'cargo fmt' to fix."
fi

# ── Step 2: Rust Linting ──
echo ""
echo "[Step 2/5] Running Clippy lints..."
if cargo clippy -- -D warnings 2>&1; then
    pass "Clippy clean (zero warnings)"
else
    fail "Clippy found warnings (treated as errors)."
fi

# ── Step 3: Rust Unit Tests ──
echo ""
echo "[Step 3/5] Running Rust unit tests..."
if cargo test 2>&1; then
    pass "All Rust tests passed"
else
    fail "Rust tests failed."
fi

# ── Step 4: Rust Documentation ──
echo ""
echo "[Step 4/5] Verifying Rust documentation builds..."
if cargo doc --no-deps 2>&1; then
    pass "cargo doc built successfully"
else
    fail "cargo doc failed. Fix documentation errors."
fi

# ── Step 5: Python Integration Tests ──
echo ""
echo "[Step 5/5] Running Python integration tests..."
cd "$FRONTEND_DIR"
if [ -d "venv" ]; then
    source venv/bin/activate
fi
if python -m pytest -v 2>&1; then
    pass "All Python tests passed"
else
    fail "Python tests failed."
fi

echo ""
echo "============================================================"
echo -e "  ${GREEN}All CI checks passed.${NC}"
echo "============================================================"
