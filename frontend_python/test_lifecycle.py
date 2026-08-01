"""
test_lifecycle.py — Runtime lifecycle integration tests.

Tests for mcm_runtime_create and mcm_runtime_destroy, validating
handle creation, null-pointer safety, and edge-case capacities.
"""

from ctypes import c_uint64

from mcm_ffi import McmRuntime


class TestLifecycle:
    """Tests for runtime creation and destruction."""

    def test_create_and_destroy(self, mcm_lib):
        """Create a runtime with capacity 16, verify handle is non-null, destroy it."""
        runtime = mcm_lib.mcm_runtime_create(c_uint64(16))
        assert runtime is not None and runtime != 0
        mcm_lib.mcm_runtime_destroy(runtime)

    def test_null_destroy_is_safe(self, mcm_lib):
        """Calling destroy with None must not crash (no-op)."""
        mcm_lib.mcm_runtime_destroy(None)

    def test_create_with_zero_capacity(self, mcm_lib):
        """A runtime with max_qubits=0 should create successfully.
        No qubits can be allocated, but the runtime handle itself is valid."""
        runtime = mcm_lib.mcm_runtime_create(c_uint64(0))
        assert runtime is not None and runtime != 0
        # Count should be 0 since nothing can be allocated.
        assert mcm_lib.mcm_qubit_count(runtime) == 0
        mcm_lib.mcm_runtime_destroy(runtime)

    def test_create_multiple_runtimes(self):
        """Multiple independent McmRuntime instances can coexist."""
        with McmRuntime(max_qubits=8) as rt1, McmRuntime(max_qubits=16) as rt2:
            rt1.allocate(0)
            rt2.allocate(0)
            # Each runtime has its own independent state.
            assert rt1.count() == 1
            assert rt2.count() == 1
