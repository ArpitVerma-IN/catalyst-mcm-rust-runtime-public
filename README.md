# Catalyst Quantum Mid-Circuit Measurement (MCM) Runtime Engine

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.11+-3670A0?style=for-the-badge&logo=python&logoColor=ffdd54)](https://www.python.org/)
[![C++ FFI](https://img.shields.io/badge/C++%20FFI-Interoperability-%2300599C.svg?style=for-the-badge&logo=c%2B%2B&logoColor=white)](https://gcc.gnu.org/)
[![Fedora 44](https://img.shields.io/badge/Fedora_44-WSL2%20Target-3C6EB4?style=for-the-badge&logo=fedora&logoColor=white)](https://getfedora.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](LICENSE)

High-performance, memory-safe, and asynchronous systems plugin designed to manage active qubits and process real-time classical feedforward control flow for the **PennyLane-Catalyst** compiler.

---

## 🏛️ Architecture

```mermaid
graph TD
    subgraph Python Layer [frontend_python/]
        mcm_ffi[mcm_ffi.py<br>ctypes wrapper]
        conftest[conftest.py<br>pytest fixtures]
        teleport[teleportation_circuit.py]
    end

    subgraph C API
        capi[include/catalyst_bindings/capi.h]
    end

    subgraph Rust Backend [backend_rust/src/]
        ffi[ffi.rs<br>extern "C"]
        runtime[runtime.rs<br>McmCore / Tokio]
        qubit[qubit.rs<br>DashMap Registry]
        bindgen[bindings.rs<br>bindgen]
        measure[measurement.rs]
        cond[conditional.rs<br>feedforward]
    end

    subgraph Validation [tests/]
        memory_safety[memory_safety/<br>harness.c, Valgrind, ASAN]
    end

    mcm_ffi -->|ctypes FFI| capi
    capi -->|#include / bindgen| ffi
    ffi --> runtime
    runtime --> qubit
    runtime --> measure
    runtime --> cond
```

## 🏗️ Repository Layout

- **`backend_rust/`**: The core Rust crate implementing the runtime engine (Tokio, DashMap).
- **`frontend_python/`**: Python `ctypes` integration test harness and verification test suite.
- **`include/`**: The C-API header declaring the FFI interface contract.
- **`tests/memory_safety/`**: C test harness, Valgrind, AddressSanitizer scripts.
- **`scripts/`**: CI validation entry point.

---

## 🛠️ Core Components

1. **`McmRuntimeCore` Orchestrator**: The central asynchronous orchestrator leveraging a native **Tokio** runtime to dispatch measurement callbacks to C++ without blocking circuit execution.
2. **`QubitRegistry`**: A high-performance, thread-safe sharded state map (powered by `DashMap`) for lock-free qubit wire tracking and measurement result storage.
3. **Mid-Circuit Measurement Execution**: Simulates state collapse and safely handles the asynchronous C-callback dispatch mechanism.
4. **Conditional Feedforward Engine**: Enables dynamic quantum control flow by providing real-time classical condition evaluation based on prior measurement outcomes.
5. **C-FFI Translation Bridge (`ffi.rs`)**: A robust, zero-cost `extern "C"` interface that seamlessly interoperates with the Catalyst C++ runtime. Fully null-pointer-safe and panic-proof across the FFI boundary.
6. **Python ctypes Integration Test Suite**: End-to-end integration tests validating symbol resolution, C ABI parameter passing, RAII wrapper safety, and callback dispatch across process boundaries.
7. **Quantum Teleportation Validation**: A comprehensive integration circuit testing dynamic feedforward and real-time corrections.
8. **Memory Safety Validation**: Exhaustive FFI safety checks using Valgrind (leak detection) and AddressSanitizer (memory corruption).

---

## 🧪 Test Matrix

| Layer | Suite | Count | Tool |
|---|---|---|---|
| Rust unit tests | `cargo test` | 25 | `cargo` |
| Python integration | `pytest -v` | 40 | `pytest` |
| C memory harness | `./harness` (5 scenarios) | 5 | `gcc` |
| Valgrind | `make valgrind` | 1 run | `valgrind` |
| ASAN | `cargo +nightly test` | 25 (re-run) | `rustup nightly` |
| **Total unique test points** | | **96** | |

---

## 💡 Target Objectives & Developer Workflows

### 🎯 Target Areas & Project Goals
This project targets key bottlenecks in modern hybrid quantum-classical compilation and runtime execution within the **PennyLane**, **Catalyst**, and **MCM** ecosystem:

1. **High-Performance Mid-Circuit Measurement (MCM)**: Replacing traditional blocking or lock-heavy runtime handlers with a lock-free, sharded `DashMap` registry and Tokio-powered async callback dispatcher.
2. **Real-Time Classical Feedforward Control Flow**: Providing sub-microsecond evaluation of dynamic conditions (`qml.cond`, repeat-until-success circuits, dynamic reset) without halting execution.
3. **Memory Safety at FFI Boundaries**: Eliminating undefined behavior, memory leaks, and unwinding panics across the Python ↔ C++ ↔ Rust boundary using strict C-ABI contracts (`capi.h`) and zero-panic Rust FFI implementations (`ffi.rs`).
4. **Structured Runtime Observability**: Delivering real-time telemetry and structured logging via `env_logger` to diagnose asynchronous execution flow and callback timing.

### 🛠️ Developer Workflows & Utilization Guide

> [!TIP]
> **Looking for exhaustive tutorials and code examples?**
> Check out the [INTEGRATIONS.md](INTEGRATIONS.md) guide for comprehensive, step-by-step instructions on integrating this project into C/C++, PennyLane, and custom FFI workflows.

#### 1. For Catalyst Compiler & C++ Runtime Developers
*   **Goal**: Integrating a robust, memory-safe MCM runtime into Catalyst compiler backends or custom LLVM-pass pipelines.
*   **Steps to Use**:
    1. **Link the C-ABI Contract**: Include `include/catalyst_bindings/capi.h` in your C++ runtime build environment.
    2. **Connect LLVM Dialects**: Map MLIR/LLVM mid-circuit measurement intrinsics (e.g., `catalyst.measure`, `catalyst.cond`) directly to `mcm_measure` and `mcm_conditional_check` entry points.
    3. **Benchmarking Qubit Tracking**: Utilize `mcm_qubit_allocate` and `mcm_qubit_release` to stress-test concurrent qubit reuse strategies under high-wire-count workloads without lock contention.

#### 2. For Quantum Algorithm Developers & Researchers (PennyLane Users)
*   **Goal**: Debugging dynamic quantum circuits (Teleportation, Error Correction, Repeat-Until-Success) with low-level runtime observability.
*   **Steps to Use**:
    1. **Pythonic Runtime Simulation**: Use the `McmRuntime` Python wrapper (`frontend_python/mcm_ffi.py`) to simulate low-level MCM behavior directly inside Python scripts before compiling with Catalyst.
    2. **Enable Real-Time Telemetry**: Set `RUST_LOG=mcm_runtime=debug` in your shell environment to inspect detailed trace logs of qubit allocations, measurement outcomes, and callback dispatches during execution.
    3. **Pre-Deployment Regression Testing**: Execute the integration harness (`pytest`) to verify custom dynamic circuits against the ABI contract.

#### 3. For Systems & FFI Engineers
*   **Goal**: Extending Rust-based runtime components or creating safe language bindings.
*   **Steps to Use**:
    1. **Reference Architecture**: Use `backend_rust/src/ffi.rs` as a template for zero-panic, FFI-safe Rust dynamic libraries interfacing with C/C++.
    2. **Automated Cross-Language Verification**: Leverage `conftest.py`'s session-scoped `ensure_lib_built` fixture to seamlessly rebuild and test Rust changes directly from Python test suites (`cargo test` + `pytest`).

---

## 🚀 Quick Start

### Prerequisites
- Fedora 44 (WSL2) or equivalent Linux
- Rust 2024 Edition (`rustc 1.85+`)
- Python 3.11+
- `gcc`, `make`, `valgrind`

### 1. Build the Rust Library
```bash
cd backend_rust && cargo build
```
This generates the dynamic shared library `backend_rust/target/debug/libmcm_runtime.so`.

### 2. Run Rust Unit Tests
```bash
cargo test
```

### 3. Run Python Integration Tests
```bash
cd frontend_python
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
pytest -v
```

### 4. Run Memory Safety Validation
```bash
tests/memory_safety/run_valgrind.sh
tests/memory_safety/run_asan.sh
```

### 5. Run Full CI Validation
```bash
scripts/ci_validate.sh
```

---

## 📈 Impact and Performance Benefits

This project serves as a robust reconstruction of the Mid-Circuit Measurement (MCM) execution layer, demonstrating advanced systems engineering techniques tailored for hybrid quantum-classical compilation.

In the current quantum computing landscape, executing dynamic circuits—such as quantum teleportation and real-time error correction—relies heavily on the host CPU. Legacy implementations frequently depend on blocking synchronous calls or global interpreter locks, which throttle execution throughput.

By migrating the orchestration layer to Rust utilizing an asynchronous runtime (`Tokio`) and a sharded, lock-free state map (`DashMap`), this architecture delivers:
- **Zero-Blocking Execution**: Classical host threads do not stall while waiting for quantum hardware or simulation responses. Asynchronous callbacks manage state collapse and feedforward instructions on background thread pools.
- **Concurrent Scalability**: The lock-free registry allows simultaneous allocation and measurement operations across thousands of active qubits without resource contention.
- **Deterministic Memory Safety**: Eliminating memory leaks and unwinding panics at the FFI boundary ensures the runtime remains robust during long-running, resource-intensive algorithms.

This architectural overhaul directly accelerates the execution of Repeat-Until-Success (RUS) loops and dynamic feedforward protocols, minimizing the classical computation overhead within the quantum execution loop.

---

## 📖 Documentation

Generate the Rust API documentation:
```bash
cd backend_rust && cargo doc --no-deps --open
```

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## 📄 License

MIT — see [LICENSE](LICENSE).
