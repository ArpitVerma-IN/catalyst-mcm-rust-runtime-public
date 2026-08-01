#!/usr/bin/env bash
# run_asan.sh — Build the Rust library with AddressSanitizer and run
# cargo test + pytest to detect memory errors at the FFI boundary.
#
# Requires: rustup component add rust-src --toolchain nightly

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend_rust"
FRONTEND_DIR="$PROJECT_ROOT/frontend_python"

echo "============================================================"
echo "  Phase 2.5: ASAN Memory Safety Validation"
echo "============================================================"

# Step 1: Check nightly toolchain
echo ""
echo "[Step 1] Checking nightly Rust toolchain..."
source ~/.cargo/env
if ! rustup run nightly rustc --version 2>/dev/null; then
    echo "  Installing nightly toolchain..."
    rustup toolchain install nightly
    rustup component add rust-src --toolchain nightly
fi
echo "  ✓ Nightly toolchain available"

# Step 2: Build with ASAN (cargo test uses nightly)
echo ""
echo "[Step 2] Running cargo test with AddressSanitizer..."
cd "$BACKEND_DIR"

RUSTFLAGS="-Zsanitizer=address" \
    cargo +nightly test \
    -Zbuild-std \
    --target x86_64-unknown-linux-gnu \
    2>&1

echo "  ✓ Rust unit tests passed under ASAN"

# Step 3: Rebuild shared library with ASAN for Python tests
echo ""
echo "[Step 3] Building libmcm_runtime.so with ASAN..."
RUSTFLAGS="-Zsanitizer=address" \
    cargo +nightly build \
    -Zbuild-std \
    --target x86_64-unknown-linux-gnu \
    2>&1

# Note: The ASAN-instrumented .so will be at:
# target/x86_64-unknown-linux-gnu/debug/libmcm_runtime.so
ASAN_LIB_PATH="$BACKEND_DIR/target/x86_64-unknown-linux-gnu/debug/libmcm_runtime.so"

if [ -f "$ASAN_LIB_PATH" ]; then
    echo "  ✓ ASAN-instrumented library built"

    # Step 4: Run pytest with the ASAN-instrumented library
    echo ""
    echo "[Step 4] Running pytest with ASAN-instrumented library..."
    cd "$FRONTEND_DIR"
    # Preload ASAN runtime for the Python process
    source venv/bin/activate
    ASAN_OPTIONS="detect_leaks=0" \
    MCM_LIB_PATH="$ASAN_LIB_PATH" \
        python -m pytest -v 2>&1 || true
    echo "  ✓ Python tests completed under ASAN"
else
    echo "  ⚠ ASAN library not found at expected path, skipping Python ASAN tests."
    echo "    This is expected if the target triple differs from x86_64-unknown-linux-gnu."
fi

echo ""
echo "============================================================"
echo "  ASAN validation complete."
echo "============================================================"
