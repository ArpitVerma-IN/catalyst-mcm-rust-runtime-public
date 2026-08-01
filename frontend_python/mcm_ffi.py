"""
mcm_ffi.py — ctypes wrapper for libmcm_runtime.so

This module provides two layers of access to the MCM runtime shared library:

1. **Low-level**: The ``load_mcm_runtime(path)`` function returns a raw
   ``ctypes.CDLL`` handle with all ``argtypes``/``restype`` declarations set.
   Use this when you need direct control over marshalling.

2. **High-level**: The ``McmRuntime`` class wraps the raw handle in a Pythonic
   RAII (Resource Acquisition Is Initialization) interface with context-manager
   support, automatic resource cleanup, and typed Python methods.
"""

import ctypes
import pathlib
from ctypes import (
    CDLL,
    CFUNCTYPE,
    POINTER,
    c_bool,
    c_char_p,
    c_uint32,
    c_uint64,
    c_void_p,
)

# ---------------------------------------------------------------------------
# Constants (matching capi.h enums)
# ---------------------------------------------------------------------------

# McmStatus
MCM_STATUS_OK = 0
MCM_STATUS_INVALID_QUBIT = 1
MCM_STATUS_RUNTIME_ERROR = 2
MCM_STATUS_ALREADY_MEASURED = 3
MCM_STATUS_ALLOCATION_FAIL = 4

# McmMeasurementResult
MCM_RESULT_ZERO = 0
MCM_RESULT_ONE = 1

# Human-readable status names for error messages.
_STATUS_NAMES = {
    MCM_STATUS_OK: "OK",
    MCM_STATUS_INVALID_QUBIT: "INVALID_QUBIT",
    MCM_STATUS_RUNTIME_ERROR: "RUNTIME_ERROR",
    MCM_STATUS_ALREADY_MEASURED: "ALREADY_MEASURED",
    MCM_STATUS_ALLOCATION_FAIL: "ALLOCATION_FAIL",
}

# Callback function type: void (*)(uint64_t wire_id, uint32_t result, void* ctx)
CALLBACK_TYPE = CFUNCTYPE(None, c_uint64, c_uint32, c_void_p)


# ---------------------------------------------------------------------------
# McmStatusError — Python exception for FFI error codes
# ---------------------------------------------------------------------------


class McmStatusError(Exception):
    """Raised when an FFI call returns a non-OK McmStatus code."""

    def __init__(self, status_code: int, function_name: str):
        self.status_code = status_code
        self.status_name = _STATUS_NAMES.get(status_code, f"UNKNOWN({status_code})")
        self.function_name = function_name
        super().__init__(
            f"{function_name} failed with status {self.status_name} ({status_code})"
        )


# ---------------------------------------------------------------------------
# Low-level: load_mcm_runtime()
# ---------------------------------------------------------------------------


def load_mcm_runtime(library_path: str) -> CDLL:
    """
    Load libmcm_runtime.so and declare all function signatures.

    Args:
        library_path: Absolute path to the compiled .so file.

    Returns:
        A ctypes.CDLL handle with all argtypes/restype set.
    """
    lib = CDLL(library_path)

    # -- Section 5: Lifecycle Management --

    lib.mcm_runtime_create.argtypes = [c_uint64]
    lib.mcm_runtime_create.restype = c_void_p

    lib.mcm_runtime_destroy.argtypes = [c_void_p]
    lib.mcm_runtime_destroy.restype = None

    # -- Section 6: Qubit Management --

    lib.mcm_qubit_allocate.argtypes = [c_void_p, c_uint64]
    lib.mcm_qubit_allocate.restype = c_uint32

    lib.mcm_qubit_release.argtypes = [c_void_p, c_uint64]
    lib.mcm_qubit_release.restype = c_uint32

    lib.mcm_qubit_count.argtypes = [c_void_p]
    lib.mcm_qubit_count.restype = c_uint64

    # -- Section 7: Measurement --

    lib.mcm_measure.argtypes = [c_void_p, c_uint64, POINTER(c_uint32)]
    lib.mcm_measure.restype = c_uint32

    lib.mcm_register_measurement_callback.argtypes = [
        c_void_p,
        CALLBACK_TYPE,
        c_void_p,
    ]
    lib.mcm_register_measurement_callback.restype = c_uint32

    # -- Section 8: Conditional --

    lib.mcm_conditional_check.argtypes = [
        c_void_p,
        c_uint64,
        c_uint32,
        POINTER(c_bool),
    ]
    lib.mcm_conditional_check.restype = c_uint32

    # -- Section 9: Telemetry --

    lib.mcm_runtime_status_string.argtypes = [c_void_p]
    lib.mcm_runtime_status_string.restype = c_char_p

    return lib


# ---------------------------------------------------------------------------
# High-level: McmRuntime class
# ---------------------------------------------------------------------------

# Default library path, resolved relative to this file.
_DEFAULT_LIB_PATH = (
    pathlib.Path(__file__).resolve().parent.parent
    / "backend_rust"
    / "target"
    / "debug"
    / "libmcm_runtime.so"
)


class McmRuntime:
    """
    Pythonic wrapper around the MCM runtime shared library.

    Implements the RAII pattern: the runtime handle is created in ``__init__``
    and destroyed in ``destroy()`` (or automatically when used as a
    context manager).

    Example::

        with McmRuntime(max_qubits=16) as rt:
            rt.allocate(0)
            result = rt.measure(0)
            print(f"Measured: {result}")
    """

    def __init__(self, max_qubits: int, library_path: str | None = None):
        """
        Create a new MCM runtime instance.

        Args:
            max_qubits: Maximum number of qubits (wire IDs must be < this).
            library_path: Path to libmcm_runtime.so. Uses the default debug
                          build path if not specified.

        Raises:
            FileNotFoundError: If the .so file does not exist.
            RuntimeError: If the Rust runtime fails to initialize.
        """
        path = pathlib.Path(library_path) if library_path else _DEFAULT_LIB_PATH
        if not path.exists():
            raise FileNotFoundError(
                f"Shared library not found at {path}. "
                f"Run 'cargo build' in backend_rust/ first."
            )

        self._lib = load_mcm_runtime(str(path))
        self._handle = self._lib.mcm_runtime_create(c_uint64(max_qubits))
        if not self._handle:
            raise RuntimeError(
                f"Failed to create MCM runtime with max_qubits={max_qubits}"
            )
        self._destroyed = False

        # Store any registered callback reference to prevent garbage collection.
        self._callback_ref = None

    # -- Context manager protocol --

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.destroy()
        return False  # Do not suppress exceptions.

    # -- Lifecycle --

    def destroy(self):
        """Destroy the runtime and free all Rust-side resources."""
        if not self._destroyed and self._handle:
            self._lib.mcm_runtime_destroy(self._handle)
            self._destroyed = True
            self._handle = None

    # -- Qubit Management --

    def allocate(self, wire_id: int) -> None:
        """
        Allocate a qubit on the specified wire.

        Raises:
            McmStatusError: If allocation fails (out of range or duplicate).
        """
        status = self._lib.mcm_qubit_allocate(self._handle, c_uint64(wire_id))
        if status != MCM_STATUS_OK:
            raise McmStatusError(status, "mcm_qubit_allocate")

    def release(self, wire_id: int) -> None:
        """
        Release a qubit, returning its wire to the free pool.

        Raises:
            McmStatusError: If the wire was not allocated.
        """
        status = self._lib.mcm_qubit_release(self._handle, c_uint64(wire_id))
        if status != MCM_STATUS_OK:
            raise McmStatusError(status, "mcm_qubit_release")

    def count(self) -> int:
        """Return the number of currently allocated (active) qubits."""
        return self._lib.mcm_qubit_count(self._handle)

    # -- Measurement --

    def measure(self, wire_id: int) -> int:
        """
        Perform a mid-circuit measurement on the specified qubit.

        Returns:
            The measurement result (MCM_RESULT_ZERO or MCM_RESULT_ONE).

        Raises:
            McmStatusError: If the wire is not allocated or already measured.
        """
        result = c_uint32()
        status = self._lib.mcm_measure(
            self._handle, c_uint64(wire_id), ctypes.byref(result)
        )
        if status != MCM_STATUS_OK:
            raise McmStatusError(status, "mcm_measure")
        return result.value

    def register_callback(self, callback, ctx=None) -> None:
        """
        Register a measurement callback.

        Args:
            callback: A CALLBACK_TYPE-compatible callable.
            ctx: Opaque context pointer (passed through to callback).

        Raises:
            McmStatusError: If registration fails.
        """
        # Keep a reference to prevent Python from garbage-collecting
        # the ctypes callback wrapper while Rust still holds a pointer to it.
        self._callback_ref = callback
        status = self._lib.mcm_register_measurement_callback(
            self._handle, callback, ctx
        )
        if status != MCM_STATUS_OK:
            raise McmStatusError(status, "mcm_register_measurement_callback")

    # -- Conditional --

    def conditional_check(self, wire_id: int, expected: int) -> bool:
        """
        Evaluate a classical condition based on a prior measurement result.

        Returns:
            True if the stored result matches ``expected``, False otherwise.

        Raises:
            McmStatusError: If the wire has no stored measurement.
        """
        condition_met = c_bool()
        status = self._lib.mcm_conditional_check(
            self._handle, c_uint64(wire_id), expected, ctypes.byref(condition_met)
        )
        if status != MCM_STATUS_OK:
            raise McmStatusError(status, "mcm_conditional_check")
        return condition_met.value

    # -- Telemetry --

    def status_string(self) -> str:
        """Return a human-readable status string from the runtime."""
        raw = self._lib.mcm_runtime_status_string(self._handle)
        if raw is None:
            return ""
        return raw.decode("utf-8")
