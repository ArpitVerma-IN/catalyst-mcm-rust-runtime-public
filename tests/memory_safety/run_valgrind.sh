#!/usr/bin/env bash
# run_valgrind.sh — Build the Rust library, compile the C harness,
# and run it under Valgrind with full leak checking.
#
# Exit code 0 = clean (no leaks, no errors)
# Exit code 1 = Valgrind detected issues

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend_rust"
HARNESS_DIR="$SCRIPT_DIR"

echo "============================================================"
echo "  Phase 2.5: Valgrind Memory Safety Validation"
echo "============================================================"

# Step 1: Build the Rust shared library (debug mode for symbols)
echo ""
echo "[Step 1] Building libmcm_runtime.so (debug)..."
cd "$BACKEND_DIR"
source ~/.cargo/env
cargo build 2>&1
echo "  ✓ Library built"

# Step 2: Compile the C test harness
echo ""
echo "[Step 2] Compiling C test harness..."
cd "$HARNESS_DIR"
make clean
make build
echo "  ✓ Harness compiled"

# Step 3: Sanity check — run without Valgrind first
echo ""
echo "[Step 3] Sanity check (no Valgrind)..."
make run
echo "  ✓ Harness runs correctly"

# Step 4: Run under Valgrind
echo ""
echo "[Step 4] Running under Valgrind..."
echo "---"
make valgrind
VALGRIND_EXIT=$?
echo "---"

if [ $VALGRIND_EXIT -eq 0 ]; then
    echo ""
    echo "  ✓ VALGRIND CLEAN: No memory errors or leaks detected."
else
    echo ""
    echo "  ✗ VALGRIND FAILED: Memory errors or leaks detected."
    exit 1
fi

echo ""
echo "============================================================"
echo "  Valgrind validation complete."
echo "============================================================"
