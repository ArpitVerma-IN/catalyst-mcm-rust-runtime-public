# Catalyst MCM Runtime: Exhaustive Integration Guide

This document provides exhaustive, code-based tutorials and configuration snippets for integrating the Catalyst Mid-Circuit Measurement (MCM) Rust Runtime into various developer workflows.

---

## 🛠️ Tech Stack & Environment Requirements

Before integrating this project, ensure your environment meets the following baseline requirements:

### Target Operating System
- **Fedora Linux 44** (Native or WSL2) — Primary tested environment. (Ubuntu/Debian also supported, provided glibc requirements are met).

### Core Tech Stack
- **Rust Toolchain**: `rustc 1.85.0` (2024 Edition) or higher.
- **C/C++ Toolchain**: `gcc`/`g++` 13+ or `clang`/`clang++` 16+, `cmake` 3.25+.
- **Python**: `3.11` or higher.

### Key Library Dependencies
- **Rust (Backend)**: `tokio` (Async runtime), `dashmap` (Lock-free concurrency), `log` & `env_logger` (Telemetry).
- **Python (Frontend)**: `pytest` (Testing framework), `pennylane` (≥ 0.45.1), `pennylane_catalyst` (≥ 0.15.0).

---

## 1. Catalyst Compiler & C++ Runtime Integration

This section guides C++ engineers on hooking the Rust MCM runtime (`libmcm_runtime.so`) into a C++ execution pipeline (e.g., the Catalyst MLIR runtime).

### Step 1.1: CMake Configuration

To consume the Rust shared library in a CMake-based C++ project, you need to tell CMake where to find the header and the `.so` file.

```cmake
# CMakeLists.txt

cmake_minimum_required(VERSION 3.25)
project(CatalystMCMIntegration CXX)

set(CMAKE_CXX_STANDARD 20)

# 1. Define paths to the Rust runtime (adjust path as necessary)
set(RUST_MCM_ROOT "${CMAKE_SOURCE_DIR}/catalyst_mcm_core")
set(RUST_MCM_INCLUDE "${RUST_MCM_ROOT}/include")
set(RUST_MCM_LIB "${RUST_MCM_ROOT}/backend_rust/target/release/libmcm_runtime.so")

# 2. Add your C++ executable or shared library
add_executable(quantum_runner main.cpp)

# 3. Include the capi.h header directory
target_include_directories(quantum_runner PRIVATE ${RUST_MCM_INCLUDE})

# 4. Link the Rust dynamic library
target_link_libraries(quantum_runner PRIVATE ${RUST_MCM_LIB})

# 5. Set RPATH so the OS can find libmcm_runtime.so at runtime
set_target_properties(quantum_runner PROPERTIES
    INSTALL_RPATH "${RUST_MCM_ROOT}/backend_rust/target/release"
    BUILD_WITH_INSTALL_RPATH TRUE
)
```

### Step 1.2: Complete C++ Implementation Example

This code demonstrates a robust pattern for interacting with the FFI. It wraps the C API in modern C++ constructs (RAII and closures) to prevent memory leaks and handle asynchronous measurement callbacks.

```cpp
// main.cpp
#include <iostream>
#include <functional>
#include <stdexcept>
#include <thread>
#include <chrono>

// Include the standard C-API contract generated for the Rust runtime
#include "catalyst_bindings/capi.h"

// -------------------------------------------------------------------
// C++ RAII Wrapper for McmRuntime
// -------------------------------------------------------------------
class McmRuntimeCpp {
private:
    McmRuntime* handle;
    std::function<void(uint64_t, uint32_t)> cpp_callback;

    // Static trampoline function to bridge C to C++ closures
    static void c_trampoline(uint64_t wire_id, uint32_t result, void* ctx) {
        if (!ctx) return;
        // Cast the opaque context pointer back to a std::function
        auto* func = static_cast<std::function<void(uint64_t, uint32_t)>*>(ctx);
        (*func)(wire_id, result);
    }

public:
    McmRuntimeCpp(uint64_t max_qubits) {
        handle = mcm_runtime_create(max_qubits);
        if (!handle) {
            throw std::runtime_error("Failed to initialize MCM Rust runtime.");
        }
    }

    ~McmRuntimeCpp() {
        if (handle) {
            mcm_runtime_destroy(handle);
        }
    }

    void allocate(uint64_t wire_id) {
        if (mcm_qubit_allocate(handle, wire_id) != MCM_STATUS_OK) {
            throw std::runtime_error("Failed to allocate qubit.");
        }
    }

    void register_callback(std::function<void(uint64_t, uint32_t)> cb) {
        cpp_callback = std::move(cb);
        // Pass the std::function pointer as the opaque context to the trampoline
        mcm_register_measurement_callback(handle, c_trampoline, &cpp_callback);
    }

    uint32_t measure(uint64_t wire_id) {
        uint32_t result = 0;
        if (mcm_measure(handle, wire_id, &result) != MCM_STATUS_OK) {
            throw std::runtime_error("Measurement failed.");
        }
        return result;
    }
};

// -------------------------------------------------------------------
// Example Execution
// -------------------------------------------------------------------
int main() {
    try {
        std::cout << "[C++] Starting MCM Runtime Integration..." << std::endl;
        
        McmRuntimeCpp runtime(64);
        
        // Register a C++ lambda as the asynchronous callback
        runtime.register_callback([](uint64_t wire, uint32_t res) {
            std::cout << "[C++ Callback] Wire " << wire << " collapsed to " << res << std::endl;
        });

        // Allocate some wires
        runtime.allocate(0);
        runtime.allocate(1);

        // Simulate Catalyst MLIR `catalyst.measure` instruction
        uint32_t r0 = runtime.measure(0);
        std::cout << "[C++] Synchronous return for Wire 0: " << r0 << std::endl;
        
        // Sleep briefly to allow the Rust Tokio thread pool to fire the callback
        std::this_thread::sleep_for(std::chrono::milliseconds(100));

    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }
    return 0;
}
```

---

## 2. PennyLane Quantum Algorithm Developer Integration

This section guides Python developers on using the provided Python `ctypes` wrapper (`mcm_ffi.py`) to simulate dynamic quantum circuits locally, and how to use telemetry for debugging.

### Step 2.1: Python Simulator Script

The following script simulates a "Repeat-Until-Success" (RUS) control flow entirely in Python, mimicking what a Catalyst compiler pass would generate under the hood for a `qml.cond` block.

**File:** `frontend_python/rus_simulation.py`

```python
import time
from mcm_ffi import McmRuntime, MCM_RESULT_ZERO, MCM_RESULT_ONE

def run_repeat_until_success():
    """
    Simulates a Repeat-Until-Success circuit:
    We allocate an ancilla (wire 0) and a target (wire 1).
    We repeatedly measure the ancilla. If it yields 1, we 'apply' a correction.
    If it yields 0, we have succeeded and exit the loop.
    """
    # Use context manager to ensure libmcm_runtime.so is safely destroyed on exit
    with McmRuntime(max_qubits=16) as rt:
        
        # 1. Setup
        print("Allocating qubits...")
        rt.allocate(0) # Ancilla
        rt.allocate(1) # Target
        
        # 2. Register telemetry callback
        def telemetry_logger(wire_id, result, ctx):
            print(f"  [Async Telemetry] Qubit {wire_id} collapsed to |{result}⟩")
            
        rt.register_callback(telemetry_logger)
        
        # 3. Simulate execution loop
        max_attempts = 5
        success = False
        
        for attempt in range(1, max_attempts + 1):
            print(f"\n--- Attempt {attempt} ---")
            
            # Simulate a Catalyst `qml.measure(wires=[0])`
            res = rt.measure(0)
            print(f"Main thread: Measurement returned {res}")
            
            # Simulate Catalyst `qml.cond` check for success (measuring 0 means success)
            if rt.conditional_check(0, expected=MCM_RESULT_ZERO):
                print("Condition Met: Success state achieved!")
                success = True
                break
            else:
                print("Condition Failed: Applying X gate correction to Target (wire 1)...")
                # (Normally you would trigger a quantum gate here)
                
                # Release and reallocate ancilla to "reset" it for the next loop
                rt.release(0)
                rt.allocate(0)
                
        # 4. Status Check
        time.sleep(0.1) # Wait for final async callbacks to flush
        print(f"\nFinal Runtime Status: {rt.status_string()}")
        
        return success

if __name__ == "__main__":
    run_repeat_until_success()
```

### Step 2.2: Utilizing Telemetry Logging

To gain deep observability into the runtime's lock-free mechanics (e.g., verifying that callbacks are dispatched asynchronously by Tokio), run your scripts with `RUST_LOG` enabled.

```bash
# Navigate to the Python environment
cd frontend_python
source venv/bin/activate

# Execute with debug logging enabled for the mcm_runtime crate
RUST_LOG=mcm_runtime=debug python rus_simulation.py
```

**Expected Log Output:**
```text
[INFO  mcm_runtime::ffi] Creating MCM runtime with max_qubits=16
[DEBUG mcm_runtime::runtime] Initializing Tokio async runtime
[INFO  mcm_runtime::ffi] MCM runtime created successfully
[DEBUG mcm_runtime::qubit] Qubit allocated on wire_id=0
[DEBUG mcm_runtime::qubit] Qubit allocated on wire_id=1
[INFO  mcm_runtime::runtime] Measurement callback registered
[DEBUG mcm_runtime::measurement] Measuring qubit wire_id=0
[DEBUG mcm_runtime::qubit] Measurement stored for wire_id=0: result=0
[DEBUG mcm_runtime::runtime] Dispatching callback for wire_id=0, result=0
[DEBUG mcm_runtime::measurement] Measurement outcome for wire_id=0: 0
```

---

## 3. Systems & FFI Extensions Integration

If you need to extend the Rust runtime (e.g., adding a `mcm_qubit_reset` function), you must follow strict memory safety patterns across the FFI boundary.

### Extension Checklist

1. **Update `capi.h`**: Always define the C-contract first.
2. **Implement in `ffi.rs`**: Use `#![allow(clippy::not_unsafe_ptr_arg_deref)]` for the module, but enforce safety inside the function.
3. **No Panics**: Never call `.unwrap()` or `.expect()` inside an `extern "C"` function. C++ cannot safely catch Rust panics, which will lead to immediate process abortion.
4. **Null Pointer Guards**: Always validate incoming pointers.

### Example: Adding `mcm_qubit_reset`

**1. `include/catalyst_bindings/capi.h`**
```c
// Add to Section 6: Qubit Management
uint32_t mcm_qubit_reset(void* runtime, uint64_t wire_id);
```

**2. `backend_rust/src/ffi.rs`**
```rust
#[unsafe(no_mangle)]
pub extern "C" fn mcm_qubit_reset(runtime: *mut McmRuntime, wire_id: u64) -> McmStatus {
    // RULE 1: Null Pointer Guard
    if runtime.is_null() {
        log::warn!("mcm_qubit_reset called with null runtime pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }

    // RULE 2: Safe Pointer Casting
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };

    // RULE 3: No unwrapping errors; match and translate to C-Enums
    match core.reset_qubit(wire_id) {
        Ok(_) => McmStatus_MCM_STATUS_OK,
        Err(e) => {
            log::error!("Failed to reset qubit {}: {:?}", wire_id, e);
            // Return appropriate error code mapped in capi.h
            McmStatus_MCM_STATUS_INVALID_QUBIT
        }
    }
}
```

**3. Test Integration**: Rebuild the backend (`cargo build`) and extend `frontend_python/mcm_ffi.py` to add the new signature and write a corresponding `pytest` integration test before merging.

---

## 4. Advanced Circuit Validation: Quantum Teleportation

For integration scenarios that demand complex conditional logic, the runtime provides a reference quantum teleportation protocol implementation (`test_teleportation.py`). This demonstrates how to construct nested classical feedforward rules based on sequential mid-circuit measurements.

The teleportation protocol requires evaluating two independent measurement outcomes (the Bell state measurements of Alice's qubits) to determine the corrective X and Z gates applied to Bob's qubit. The runtime handles these dependencies via sequential calls to `mcm_conditional_check`, ensuring that the host thread properly routes the dynamic circuit flow without latency overhead.

When integrating similar algorithms, ensure that the callback context pointers remain valid throughout the entire logical lifetime of the qubit allocations.

---

## 5. Memory Safety & CI Pipeline Validation

When deploying this runtime in a production compiler backend, strict validation procedures must be maintained to prevent regressions at the C-FFI boundary.

### Continuous Integration (CI)
A unified validation script is provided at `scripts/ci_validate.sh`. It enforces formatting, linting, unit tests, and Python integration tests. Run this script locally prior to committing changes to the C++ runtime bridges.

### Valgrind and AddressSanitizer
The `tests/memory_safety/` directory contains a dedicated C harness (`harness.c`) that exercises extreme edge cases (null pointer injections, double-releases, and concurrent callback spam). 
- Use the provided `run_valgrind.sh` script to verify that the Tokio thread pool and DashMap shards release memory deterministically.
- Use the `run_asan.sh` script to recompile the shared library with AddressSanitizer instrumentations for deep pointer boundary validation.
