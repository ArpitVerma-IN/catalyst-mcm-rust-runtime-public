"""
test_teleportation.py — Pytest integration tests for the quantum
teleportation protocol using the MCM Rust Runtime.
"""

import time
import pytest

from mcm_ffi import (
    McmRuntime,
    MCM_RESULT_ZERO,
    MCM_RESULT_ONE,
    CALLBACK_TYPE,
    McmStatusError,
)


class TestTeleportationProtocol:
    """
    Validates the complete teleportation circuit control flow through
    the MCM runtime's FFI endpoints.
    """

    # Wire assignments matching the teleportation protocol
    WIRE_PSI = 0      # State to teleport (even → 0)
    WIRE_ALICE = 1    # Alice's EPR half (odd → 1)
    WIRE_BOB = 2      # Bob's target (even, never measured by Alice)

    def test_allocate_teleportation_wires(self, runtime):
        """All three teleportation wires can be allocated."""
        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)
        runtime.allocate(self.WIRE_BOB)
        assert runtime.count() == 3

    def test_bell_measurement_outcomes(self, runtime):
        """Mid-circuit measurements return deterministic parity results."""
        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)

        m0 = runtime.measure(self.WIRE_PSI)   # even → 0
        m1 = runtime.measure(self.WIRE_ALICE)  # odd → 1

        assert m0 == MCM_RESULT_ZERO
        assert m1 == MCM_RESULT_ONE

    def test_x_correction_decision(self, runtime):
        """Conditional check correctly identifies X-correction needed (m1 == 1)."""
        runtime.allocate(self.WIRE_ALICE)
        runtime.measure(self.WIRE_ALICE)

        assert runtime.conditional_check(self.WIRE_ALICE, MCM_RESULT_ONE) is True
        assert runtime.conditional_check(self.WIRE_ALICE, MCM_RESULT_ZERO) is False

    def test_z_correction_decision(self, runtime):
        """Conditional check correctly identifies no Z-correction needed (m0 == 0)."""
        runtime.allocate(self.WIRE_PSI)
        runtime.measure(self.WIRE_PSI)

        assert runtime.conditional_check(self.WIRE_PSI, MCM_RESULT_ONE) is False
        assert runtime.conditional_check(self.WIRE_PSI, MCM_RESULT_ZERO) is True

    def test_bob_wire_survives_measurement_phase(self, runtime):
        """Bob's wire remains allocated and unmeasured after Alice's measurements."""
        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)
        runtime.allocate(self.WIRE_BOB)

        runtime.measure(self.WIRE_PSI)
        runtime.measure(self.WIRE_ALICE)

        assert runtime.count() == 3

    def test_measured_wires_released_bob_survives(self, runtime):
        """After releasing measured wires, only Bob's wire remains."""
        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)
        runtime.allocate(self.WIRE_BOB)

        runtime.measure(self.WIRE_PSI)
        runtime.measure(self.WIRE_ALICE)

        runtime.release(self.WIRE_PSI)
        runtime.release(self.WIRE_ALICE)

        assert runtime.count() == 1

    def test_callback_fires_for_both_measurements(self, runtime):
        """Async callback is invoked exactly twice (once per measurement)."""
        callback_log = []

        @CALLBACK_TYPE
        def cb(wire_id, result, ctx):
            callback_log.append((wire_id, result))

        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)
        runtime.register_callback(cb)

        runtime.measure(self.WIRE_PSI)
        runtime.measure(self.WIRE_ALICE)

        time.sleep(0.15)
        assert len(callback_log) == 2

    def test_callback_receives_correct_outcomes(self, runtime):
        """Callback arguments match the deterministic parity outcomes."""
        callback_log = []

        @CALLBACK_TYPE
        def cb(wire_id, result, ctx):
            callback_log.append((wire_id, result))

        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)
        runtime.register_callback(cb)

        runtime.measure(self.WIRE_PSI)
        runtime.measure(self.WIRE_ALICE)

        time.sleep(0.15)
        outcomes = {wire: res for wire, res in callback_log}
        assert outcomes[self.WIRE_PSI] == MCM_RESULT_ZERO
        assert outcomes[self.WIRE_ALICE] == MCM_RESULT_ONE

    def test_full_teleportation_lifecycle(self, runtime):
        """End-to-end teleportation: allocate → measure → condition → release."""
        callback_log = []

        @CALLBACK_TYPE
        def cb(wire_id, result, ctx):
            callback_log.append((wire_id, result))

        # Allocate
        runtime.allocate(self.WIRE_PSI)
        runtime.allocate(self.WIRE_ALICE)
        runtime.allocate(self.WIRE_BOB)
        runtime.register_callback(cb)
        assert runtime.count() == 3

        # Mid-circuit measurements
        m0 = runtime.measure(self.WIRE_PSI)
        m1 = runtime.measure(self.WIRE_ALICE)
        assert m0 == MCM_RESULT_ZERO
        assert m1 == MCM_RESULT_ONE

        # Classical feedforward decisions
        needs_x = runtime.conditional_check(self.WIRE_ALICE, MCM_RESULT_ONE)
        needs_z = runtime.conditional_check(self.WIRE_PSI, MCM_RESULT_ONE)
        assert needs_x is True
        assert needs_z is False

        # Verify callbacks fired
        time.sleep(0.15)
        assert len(callback_log) == 2

        # Release measured wires
        runtime.release(self.WIRE_PSI)
        runtime.release(self.WIRE_ALICE)
        assert runtime.count() == 1

        # Telemetry
        status = runtime.status_string()
        assert "active_qubits=1" in status

    def test_double_measure_prevented(self, runtime):
        """Re-measuring a wire raises McmStatusError (double-measurement guard)."""
        runtime.allocate(self.WIRE_PSI)
        runtime.measure(self.WIRE_PSI)

        with pytest.raises(McmStatusError):
            runtime.measure(self.WIRE_PSI)

    def test_conditional_on_unmeasured_bob_fails(self, runtime):
        """Attempting conditional_check on unmeasured Bob's wire raises error."""
        runtime.allocate(self.WIRE_BOB)

        with pytest.raises(McmStatusError):
            runtime.conditional_check(self.WIRE_BOB, MCM_RESULT_ZERO)


class TestAlternateWireAssignments:
    """
    Validates the protocol with different wire ID assignments to
    ensure the runtime is not hard-coded to specific wire numbers.
    """

    def test_swapped_parity_assignment(self, runtime):
        """Use wire 3 (odd→1) for ψ and wire 4 (even→0) for Alice."""
        WIRE_PSI = 3
        WIRE_ALICE = 4
        WIRE_BOB = 5

        runtime.allocate(WIRE_PSI)
        runtime.allocate(WIRE_ALICE)
        runtime.allocate(WIRE_BOB)

        m0 = runtime.measure(WIRE_PSI)   # odd → 1
        m1 = runtime.measure(WIRE_ALICE)  # even → 0

        assert m0 == MCM_RESULT_ONE
        assert m1 == MCM_RESULT_ZERO

        # Corrections are now inverted from the standard case
        needs_x = runtime.conditional_check(WIRE_ALICE, MCM_RESULT_ONE)
        needs_z = runtime.conditional_check(WIRE_PSI, MCM_RESULT_ONE)

        assert needs_x is False  # m1=0, no X needed
        assert needs_z is True   # m0=1, Z IS needed

    def test_high_wire_ids(self, runtime):
        """Protocol works with large wire IDs (stress boundary check)."""
        WIRE_PSI = 10
        WIRE_ALICE = 11
        WIRE_BOB = 12

        runtime.allocate(WIRE_PSI)
        runtime.allocate(WIRE_ALICE)
        runtime.allocate(WIRE_BOB)

        m0 = runtime.measure(WIRE_PSI)
        m1 = runtime.measure(WIRE_ALICE)

        assert m0 == MCM_RESULT_ZERO  # even
        assert m1 == MCM_RESULT_ONE   # odd
        assert runtime.count() == 3
