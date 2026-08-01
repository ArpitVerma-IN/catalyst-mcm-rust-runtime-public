# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] — 2026-08-01

### Added

#### Runtime Core (Rust)
- `McmRuntimeCore` async orchestrator with Tokio multi-threaded runtime
- `QubitRegistry` lock-free sharded state map (DashMap) for wire tracking
- Mid-circuit measurement execution with deterministic parity-based simulation
- Asynchronous C-callback dispatch via `tokio::spawn_blocking`
- Conditional feedforward engine for dynamic circuit control flow
- `extern "C"` FFI bridge with null-pointer safety and zero-panic guarantees
- Structured logging via `env_logger` for runtime observability
- 25 Rust unit tests covering all core modules

#### C-API Contract
- `capi.h` header with opaque handle types, status codes, and Doxygen documentation
- `bindgen` integration for automatic Rust type generation from C header

#### Python Integration
- `mcm_ffi.py` ctypes wrapper with RAII context manager and error translation
- `conftest.py` auto-build fixture for seamless Rust → Python testing
- 40 integration tests across 6 test files (lifecycle, qubit, measurement, conditional, telemetry, teleportation)
- Quantum teleportation validation circuit with optional PennyLane reference simulation

#### Memory Safety Validation
- C test harness exercising 5 FFI scenarios (lifecycle, protocol, callback race, null safety, stress)
- Valgrind integration with suppression rules for Tokio/glibc false positives
- AddressSanitizer integration via `cargo +nightly test` and ASAN-instrumented `.so`

#### Documentation & Project
- README with architecture diagram, test matrix, and quick-start guide
- INTEGRATIONS.md with exhaustive C++, Python, and FFI extension tutorials
- CONTRIBUTING.md with commit conventions and development workflow
- SECURITY.md with vulnerability reporting process
- CI validation script (`scripts/ci_validate.sh`)
- This CHANGELOG
