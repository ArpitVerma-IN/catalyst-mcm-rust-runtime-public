# Catalyst MCM Runtime: Exhaustive Integration Guide

This document provides comprehensive guides for integrating the Catalyst Mid-Circuit Measurement (MCM) Rust Runtime into various workflows, using pre-compiled release binaries, and running the development test suites.

---

## 📦 1. Using Pre-Compiled Release Binaries (`.tar.gz`)

The easiest way to integrate the MCM Runtime into your C/C++ projects is by using the official release packages hosted on GitHub Releases.

### Step 1: Download and Extract
Download the latest `mcm_runtime_vX.X.X_linux_x86_64.tar.gz` from the repository's Releases page.
```bash
tar -xzf mcm_runtime_v0.1.0_linux_x86_64.tar.gz
```
This extracts a structured directory containing exactly what you need to link against:
* `lib/libmcm_runtime.so` (The compiled runtime engine)
* `include/capi.h` (The C/C++ API header contract)

### Step 2: C++ CMake Integration
Link the extracted package into your CMake project:

```cmake
# CMakeLists.txt
cmake_minimum_required(VERSION 3.25)
project(CatalystMCMIntegration CXX)
set(CMAKE_CXX_STANDARD 20)

# 1. Point these to your extracted directory
set(MCM_PACKAGE_DIR "/path/to/extracted/mcm_runtime_v0.1.0_linux_x86_64")
set(MCM_INCLUDE "${MCM_PACKAGE_DIR}/include")
set(MCM_LIB "${MCM_PACKAGE_DIR}/lib/libmcm_runtime.so")

# 2. Add your executable
add_executable(quantum_runner main.cpp)

# 3. Include and Link
target_include_directories(quantum_runner PRIVATE ${MCM_INCLUDE})
target_link_libraries(quantum_runner PRIVATE ${MCM_LIB})

# 4. Set RPATH so the OS can find the library at runtime
set_target_properties(quantum_runner PROPERTIES
    INSTALL_RPATH "${MCM_PACKAGE_DIR}/lib"
    BUILD_WITH_INSTALL_RPATH TRUE
)
```

---

## 🛠️ 2. Building from Source & Developer Scripts

If you are modifying the runtime or running the validation pipelines locally, use the following developer workflows. *(Prerequisites: Fedora 44 / WSL2, Rust 1.85+, Python 3.11+).*

### Compiling the Engine
```bash
cd backend_rust
cargo build --release
```
*Outputs: `backend_rust/target/release/libmcm_runtime.so`*

### Running Python Integration Tests
```bash
cd frontend_python
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
pytest -v
```

### Memory Safety & CI Validation
The project includes strict memory testing scripts using Valgrind and ASAN:
```bash
# Valgrind Leak Check
tests/memory_safety/run_valgrind.sh

# AddressSanitizer Validation
tests/memory_safety/run_asan.sh

# Full CI Pipeline (Formats, Lints, Tests)
scripts/ci_validate.sh
```

---

## 🚀 3. C++ API Implementation Example

This code demonstrates a robust pattern for interacting with the FFI. It wraps the C API in modern C++ constructs (RAII and closures) to prevent memory leaks and handle asynchronous measurement callbacks.

```cpp
// main.cpp
#include <iostream>
#include <functional>
#include <stdexcept>
#include <thread>
#include <chrono>

// Include the standard C-API contract provided in the .tar.gz include/ folder
#include "capi.h"

class McmRuntimeCpp {
private:
    McmRuntime* handle;
    std::function<void(uint64_t, uint32_t)> cpp_callback;

    static void c_trampoline(uint64_t wire_id, uint32_t result, void* ctx) {
        if (!ctx) return;
        auto* func = static_cast<std::function<void(uint64_t, uint32_t)>*>(ctx);
        (*func)(wire_id, result);
    }

public:
    McmRuntimeCpp(uint64_t max_qubits) {
        handle = mcm_runtime_create(max_qubits);
        if (!handle) throw std::runtime_error("Init failed.");
    }

    ~McmRuntimeCpp() {
        if (handle) mcm_runtime_destroy(handle);
    }

    void allocate(uint64_t wire_id) {
        if (mcm_qubit_allocate(handle, wire_id) != MCM_STATUS_OK) {
            throw std::runtime_error("Allocation failed.");
        }
    }

    void register_callback(std::function<void(uint64_t, uint32_t)> cb) {
        cpp_callback = std::move(cb);
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

int main() {
    McmRuntimeCpp runtime(64);
    
    runtime.register_callback([](uint64_t wire, uint32_t res) {
        std::cout << "[Async Callback] Wire " << wire << " collapsed to " << res << std::endl;
    });

    runtime.allocate(0);
    uint32_t r0 = runtime.measure(0);
    std::cout << "[Main Thread] Sync return: " << r0 << std::endl;
    
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    return 0;
}
```

---

## 🐍 4. Python Developer Simulation

For quantum algorithm developers, the included Python `ctypes` wrapper allows you to simulate Repeat-Until-Success logic.

**File:** `frontend_python/rus_simulation.py`

```python
import time
from mcm_ffi import McmRuntime, MCM_RESULT_ZERO, MCM_RESULT_ONE

def run_repeat_until_success():
    # Context manager handles safe destruction
    with McmRuntime(max_qubits=16) as rt:
        
        rt.allocate(0) # Ancilla
        rt.allocate(1) # Target
        
        # Telemetry logger
        rt.register_callback(lambda wire, res, ctx: print(f"  [Callback] Qubit {wire} collapsed to |{res}⟩"))
        
        success = False
        for attempt in range(1, 6):
            print(f"\n--- Attempt {attempt} ---")
            
            res = rt.measure(0)
            
            # Simulate Catalyst `qml.cond` check
            if rt.conditional_check(0, expected=MCM_RESULT_ZERO):
                print("Condition Met: Success state achieved!")
                success = True
                break
            else:
                print("Condition Failed: Applying X gate correction to Target...")
                # Reset ancilla
                rt.release(0)
                rt.allocate(0)
                
        time.sleep(0.1)
        print(f"\nFinal Status: {rt.status_string()}")
        return success

if __name__ == "__main__":
    run_repeat_until_success()
```

To gain deep observability into the runtime's Tokio asynchronous mechanics, enable Rust telemetry:
```bash
cd frontend_python
source venv/bin/activate
RUST_LOG=mcm_runtime=debug python rus_simulation.py
```
