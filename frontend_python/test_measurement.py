"""
test_measurement.py — Measurement and callback integration tests.

Tests for mcm_measure and mcm_register_measurement_callback, covering
basic measurement, error conditions, deterministic parity, and async
callback dispatch verification.
"""

import time

import pytest

from mcm_ffi import (
    CALLBACK_TYPE,
    MCM_RESULT_ONE,
    MCM_RESULT_ZERO,
    MCM_STATUS_ALREADY_MEASURED,
    MCM_STATUS_INVALID_QUBIT,
    McmStatusError,
)


class TestMeasurement:
    """Tests for mid-circuit measurement."""

    def test_measure_qubit(self, runtime):
        """Allocate wire 0, measure it, verify result is 0 or 1."""
        runtime.allocate(0)
        result = runtime.measure(0)
        assert result in (MCM_RESULT_ZERO, MCM_RESULT_ONE)

    def test_measure_unallocated(self, runtime):
        """Measuring a wire that was never allocated raises INVALID_QUBIT."""
        with pytest.raises(McmStatusError) as exc_info:
            runtime.measure(42)
        assert exc_info.value.status_code == MCM_STATUS_INVALID_QUBIT

    def test_measure_twice(self, runtime):
        """Measuring the same qubit twice raises ALREADY_MEASURED."""
        runtime.allocate(5)
        runtime.measure(5)  # First measurement succeeds.
        with pytest.raises(McmStatusError) as exc_info:
            runtime.measure(5)
        assert exc_info.value.status_code == MCM_STATUS_ALREADY_MEASURED

    def test_measure_deterministic_parity(self, runtime):
        """Even wire IDs produce ZERO, odd wire IDs produce ONE.
        This validates the deterministic simulation logic."""
        runtime.allocate(0)
        runtime.allocate(1)
        runtime.allocate(2)
        runtime.allocate(3)

        assert runtime.measure(0) == MCM_RESULT_ZERO   # even
        assert runtime.measure(1) == MCM_RESULT_ONE     # odd
        assert runtime.measure(2) == MCM_RESULT_ZERO   # even
        assert runtime.measure(3) == MCM_RESULT_ONE     # odd

    def test_callback_fires(self, runtime):
        """Register a callback, measure a qubit, verify the callback
        was invoked with the correct wire_id and result."""
        runtime.allocate(3)

        captured = []

        @CALLBACK_TYPE
        def on_measure(wire_id, result, ctx):
            captured.append((wire_id, result))

        runtime.register_callback(on_measure)

        runtime.measure(3)

        # Give Tokio's thread pool time to dispatch the callback.
        time.sleep(0.15)

        assert len(captured) == 1
        assert captured[0][0] == 3  # wire_id
        assert captured[0][1] in (MCM_RESULT_ZERO, MCM_RESULT_ONE)

    def test_callback_receives_correct_context(self, runtime):
        """The callback receives the context pointer passed during registration.
        We pass a known integer address and verify it arrives intact."""
        import ctypes

        runtime.allocate(7)

        captured_ctx = []

        @CALLBACK_TYPE
        def on_measure(wire_id, result, ctx):
            captured_ctx.append(ctx)

        # Pass a known non-null context value (address 0xDEAD).
        sentinel_ctx = ctypes.c_void_p(0xDEAD)
        runtime._lib.mcm_register_measurement_callback(
            runtime._handle, on_measure, sentinel_ctx
        )
        # Keep callback reference alive.
        runtime._callback_ref = on_measure

        runtime.measure(7)
        time.sleep(0.15)

        assert len(captured_ctx) == 1
        # The context pointer should arrive as an integer.
        assert captured_ctx[0] == 0xDEAD
