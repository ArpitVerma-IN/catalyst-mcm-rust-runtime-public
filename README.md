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

The runtime operates across three distinct boundary layers:

1. **Frontend (Python / C++)**: Acts as the simulation or execution host. It links against the compiled dynamic library (`libmcm_runtime.so`) and triggers quantum instructions.
2. **C-API Bridge (`capi.h` & `ffi.rs`)**: A strict, zero-panic `extern "C"` interface that safely translates memory pointers and runtime commands between the host language and the Rust backend.
3. **Rust Core Engine (`backend_rust`)**: 
   - Utilizes an asynchronous **Tokio** runtime to dispatch measurement callbacks to background threads, ensuring the host execution thread never blocks.
   - Utilizes a highly concurrent, lock-free **DashMap** registry to track active qubits and store collapsed measurement states, enabling sub-microsecond evaluation of dynamic classical feedforward logic.

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
