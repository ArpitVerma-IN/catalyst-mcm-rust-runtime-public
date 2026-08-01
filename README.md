# Catalyst Quantum Mid-Circuit Measurement (MCM) Runtime Engine

[![Rust](https://img.shields.io/badge/rust-2024%20Edition-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.11+-3670A0?style=for-the-badge&logo=python&logoColor=ffdd54)](https://www.python.org/)
[![C++ FFI](https://img.shields.io/badge/C++%20FFI-Interoperability-%2300599C.svg?style=for-the-badge&logo=c%2B%2B&logoColor=white)](https://gcc.gnu.org/)
[![Fedora 44](https://img.shields.io/badge/Fedora_44-WSL2%20Target-3C6EB4?style=for-the-badge&logo=fedora&logoColor=white)](https://getfedora.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](LICENSE)

High-performance, memory-safe, and asynchronous systems plugin designed to manage active qubits and process real-time classical feedforward control flow for the **PennyLane-Catalyst** compiler.

> [!IMPORTANT]
> **For extensive setup instructions, usage guides, and details on how to use the compiled `.tar.gz` release binaries, please view the [INTEGRATIONS](INTEGRATIONS.md) guide.**

## 🏛️ Architecture

```mermaid
graph TD
    subgraph Python Layer [frontend_python/]
        mcm_ffi[mcm_ffi.py<br>ctypes wrapper]
        conftest[conftest.py<br>pytest fixtures]
    end

    subgraph C API
        capi[include/catalyst_bindings/capi.h]
    end

    subgraph Rust Backend [backend_rust/src/]
        ffi[ffi.rs<br>extern "C"]
        runtime[runtime.rs<br>McmCore / Tokio]
        qubit[qubit.rs<br>DashMap Registry]
    end

    mcm_ffi -->|ctypes FFI| capi
    capi -->|#include / bindgen| ffi
    ffi --> runtime
    runtime --> qubit
```

## 🏗️ Repository Layout

- **`backend_rust/`**: The core Rust crate implementing the runtime engine.
- **`frontend_python/`**: Python `ctypes` integration test harness.
- **`include/`**: The C-API header declaring the FFI interface contract.
- **`tests/memory_safety/`**: C test harness, Valgrind, AddressSanitizer scripts.
- **`scripts/`**: CI validation entry point.

## 🤝 Contributing
See [CONTRIBUTING.md](CONTRIBUTING.md).

## 📄 License
MIT — see [LICENSE](LICENSE).
