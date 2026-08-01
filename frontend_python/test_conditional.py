"""
test_conditional.py — Conditional evaluation integration tests.

Tests for mcm_conditional_check, validating condition matching,
mismatching, and error handling for unmeasured/unallocated wires.
"""

import pytest

from mcm_ffi import (
    MCM_RESULT_ONE,
    MCM_RESULT_ZERO,
    MCM_STATUS_INVALID_QUBIT,
    McmStatusError,
)


class TestConditional:
    """Tests for classical condition evaluation after measurement."""

    def test_conditional_match(self, runtime):
        """When the stored result matches the expected value, return True."""
        runtime.allocate(0)
        result = runtime.measure(0)  # Even wire → ZERO
        assert runtime.conditional_check(0, result) is True

    def test_conditional_mismatch(self, runtime):
        """When the stored result differs from expected, return False."""
        runtime.allocate(0)
        result = runtime.measure(0)
        opposite = MCM_RESULT_ONE if result == MCM_RESULT_ZERO else MCM_RESULT_ZERO
        assert runtime.conditional_check(0, opposite) is False

    def test_conditional_unmeasured(self, runtime):
        """Checking a condition on a wire that has not been measured raises INVALID_QUBIT."""
        runtime.allocate(0)
        with pytest.raises(McmStatusError) as exc_info:
            runtime.conditional_check(0, MCM_RESULT_ZERO)
        assert exc_info.value.status_code == MCM_STATUS_INVALID_QUBIT

    def test_conditional_unallocated(self, runtime):
        """Checking a condition on a wire that was never allocated raises INVALID_QUBIT."""
        with pytest.raises(McmStatusError) as exc_info:
            runtime.conditional_check(99, MCM_RESULT_ONE)
        assert exc_info.value.status_code == MCM_STATUS_INVALID_QUBIT

    def test_conditional_after_release(self, runtime):
        """After measuring and then releasing a wire, conditional check
        should fail because the qubit state was removed."""
        runtime.allocate(2)
        runtime.measure(2)
        runtime.release(2)
        with pytest.raises(McmStatusError) as exc_info:
            runtime.conditional_check(2, MCM_RESULT_ZERO)
        assert exc_info.value.status_code == MCM_STATUS_INVALID_QUBIT
