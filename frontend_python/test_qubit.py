"""
test_qubit.py — Qubit allocation and release integration tests.

Tests for mcm_qubit_allocate, mcm_qubit_release, and mcm_qubit_count,
covering normal operation, capacity limits, duplicates, and reallocation.
"""

import pytest

from mcm_ffi import (
    MCM_STATUS_ALLOCATION_FAIL,
    MCM_STATUS_INVALID_QUBIT,
    McmRuntime,
    McmStatusError,
)


class TestQubitManagement:
    """Tests for qubit allocation, release, and counting."""

    def test_allocate_single(self, runtime):
        """Allocate a single qubit and verify count is 1."""
        runtime.allocate(0)
        assert runtime.count() == 1

    def test_allocate_and_count(self, runtime):
        """Allocate 5 qubits on wires 0-4, verify count is 5."""
        for wire in range(5):
            runtime.allocate(wire)
        assert runtime.count() == 5

    def test_allocate_beyond_capacity(self):
        """Allocating wire_id >= max_qubits raises ALLOCATION_FAIL."""
        with McmRuntime(max_qubits=16) as rt:
            with pytest.raises(McmStatusError) as exc_info:
                rt.allocate(100)
            assert exc_info.value.status_code == MCM_STATUS_ALLOCATION_FAIL

    def test_allocate_duplicate(self, runtime):
        """Allocating the same wire twice raises ALLOCATION_FAIL."""
        runtime.allocate(0)
        with pytest.raises(McmStatusError) as exc_info:
            runtime.allocate(0)
        assert exc_info.value.status_code == MCM_STATUS_ALLOCATION_FAIL

    def test_release_and_recount(self, runtime):
        """Allocate 3 qubits, release 1, verify count decrements to 2."""
        for wire in range(3):
            runtime.allocate(wire)
        assert runtime.count() == 3
        runtime.release(1)
        assert runtime.count() == 2

    def test_release_unallocated(self, runtime):
        """Releasing a wire that was never allocated raises INVALID_QUBIT."""
        with pytest.raises(McmStatusError) as exc_info:
            runtime.release(99)
        assert exc_info.value.status_code == MCM_STATUS_INVALID_QUBIT

    def test_allocate_release_reallocate(self, runtime):
        """A released wire can be reallocated and used again."""
        runtime.allocate(5)
        assert runtime.count() == 1
        runtime.release(5)
        assert runtime.count() == 0
        # Reallocate the same wire — should succeed.
        runtime.allocate(5)
        assert runtime.count() == 1
