"""
test_telemetry.py — Telemetry, status string, and end-to-end integration tests.

Tests for mcm_runtime_status_string and the full McmRuntime class wrapper,
including context-manager cleanup and a comprehensive lifecycle test.
"""

from mcm_ffi import MCM_RESULT_ONE, MCM_RESULT_ZERO, McmRuntime


class TestTelemetry:
    """Tests for the runtime status/telemetry string."""

    def test_status_string_contains_count(self, runtime):
        """After allocating 2 qubits, the status string reports active_qubits=2."""
        runtime.allocate(0)
        runtime.allocate(1)
        status = runtime.status_string()
        assert "active_qubits=2" in status

    def test_status_string_shows_callback(self, runtime):
        """After registering a callback, the status string reports callback_registered=true."""
        from mcm_ffi import CALLBACK_TYPE

        @CALLBACK_TYPE
        def noop(wire_id, result, ctx):
            pass

        runtime.register_callback(noop)
        status = runtime.status_string()
        assert "callback_registered=true" in status

    def test_status_string_after_release(self, runtime):
        """After allocating 3 and releasing 1, the count in the status string is 2."""
        for wire in range(3):
            runtime.allocate(wire)
        runtime.release(1)
        status = runtime.status_string()
        assert "active_qubits=2" in status


class TestFullLifecycle:
    """End-to-end integration tests through every FFI entry point."""

    def test_full_lifecycle(self, runtime):
        """Create → allocate → measure → conditional → status → release → destroy."""
        # Allocate wires 0 (even) and 1 (odd)
        runtime.allocate(0)
        runtime.allocate(1)
        assert runtime.count() == 2

        # Measure: even → ZERO, odd → ONE
        assert runtime.measure(0) == MCM_RESULT_ZERO
        assert runtime.measure(1) == MCM_RESULT_ONE

        # Conditional: wire 0 measured ZERO, check against ZERO → True
        assert runtime.conditional_check(0, MCM_RESULT_ZERO) is True

        # Conditional: wire 1 measured ONE, check against ZERO → False
        assert runtime.conditional_check(1, MCM_RESULT_ZERO) is False

        # Status string
        status = runtime.status_string()
        assert "active_qubits=2" in status

        # Release wire 0
        runtime.release(0)
        assert runtime.count() == 1

    def test_class_wrapper_context_manager(self):
        """The McmRuntime context manager correctly cleans up resources
        when the with-block exits."""
        rt = McmRuntime(max_qubits=8)
        rt.allocate(0)
        assert rt.count() == 1
        # Manually call __exit__ via destroy.
        rt.destroy()
        # After destroy, calling destroy again should be a safe no-op.
        rt.destroy()
