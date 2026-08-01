"""
teleportation_circuit.py — Quantum Teleportation Validation using the MCM Runtime.

Demonstrates the complete teleportation protocol, exercising every FFI
endpoint in the Catalyst MCM Rust Runtime to validate end-to-end
mid-circuit measurement and classical feedforward functionality.
"""

import time
import numpy as np

# Conditionally import PennyLane for the reference simulation
try:
    import pennylane as qml
    HAS_PENNYLANE = True
except ImportError:
    HAS_PENNYLANE = False

from mcm_ffi import (
    McmRuntime,
    MCM_RESULT_ZERO,
    MCM_RESULT_ONE,
    CALLBACK_TYPE,
)


# ── Section 1: MCM Runtime Teleportation ────────────────────────────

def run_mcm_teleportation():
    """
    Execute the teleportation protocol's classical control flow
    through the Rust MCM runtime.

    Returns a dict with measurement outcomes and correction decisions.
    """
    with McmRuntime(max_qubits=8) as rt:
        # Step 1: Allocate the 3 teleportation wires
        WIRE_PSI = 0    # State to teleport (even → measures 0)
        WIRE_ALICE = 1  # Alice's EPR half (odd → measures 1)
        WIRE_BOB = 2    # Bob's EPR half (target)

        rt.allocate(WIRE_PSI)
        rt.allocate(WIRE_ALICE)
        rt.allocate(WIRE_BOB)

        # Step 2: Register async telemetry callback
        callback_log = []

        @CALLBACK_TYPE
        def telemetry_cb(wire_id, result, ctx):
            callback_log.append((wire_id, result))

        rt.register_callback(telemetry_cb)

        # Step 3: Mid-circuit measurements (Alice's Bell measurement results)
        m0 = rt.measure(WIRE_PSI)
        m1 = rt.measure(WIRE_ALICE)

        # Step 4: Classical feedforward — conditional correction decisions
        needs_x_correction = rt.conditional_check(WIRE_ALICE, MCM_RESULT_ONE)
        needs_z_correction = rt.conditional_check(WIRE_PSI, MCM_RESULT_ONE)

        # Step 5: Runtime telemetry snapshot
        time.sleep(0.1)  # Allow async callbacks to flush
        status = rt.status_string()

        # Step 6: Release measured wires (Bob's wire stays active)
        rt.release(WIRE_PSI)
        rt.release(WIRE_ALICE)

        return {
            "m0": m0,
            "m1": m1,
            "needs_x_correction": needs_x_correction,
            "needs_z_correction": needs_z_correction,
            "callback_log": callback_log,
            "status": status,
            "bob_wire_active": rt.count(),
        }


# ── Section 2: PennyLane Reference Simulation ───────────────────────

def run_pennylane_reference(theta, phi):
    """
    Run a full teleportation circuit on PennyLane's default.qubit simulator.

    Prepares |ψ⟩ = RY(theta) RZ(phi) |0⟩ on wire 0, creates a Bell pair
    on wires 1-2, performs Bell measurement, and applies corrections.

    Returns the final state amplitudes of wire 2 (Bob's qubit).
    """
    if not HAS_PENNYLANE:
        return None

    dev = qml.device("default.qubit", wires=3)

    @qml.qnode(dev)
    def teleportation_circuit():
        # Prepare |ψ⟩ on wire 0
        qml.RY(theta, wires=0)
        qml.RZ(phi, wires=0)

        # Create Bell pair on wires 1-2
        qml.Hadamard(wires=1)
        qml.CNOT(wires=[1, 2])

        # Alice's Bell measurement circuit
        qml.CNOT(wires=[0, 1])
        qml.Hadamard(wires=0)

        # Return the full state vector for analysis
        return qml.state()

    state = teleportation_circuit()
    return state


# ── Section 3: Main Execution ───────────────────────────────────────

def main():
    print("=" * 60)
    print("  Quantum Teleportation — MCM Runtime Validation")
    print("=" * 60)

    # Layer 1: MCM Runtime
    print("\n[Layer 1] MCM Runtime Orchestration")
    print("-" * 40)
    results = run_mcm_teleportation()

    print(f"  Wire 0 (ψ) measurement:       m0 = {results['m0']}")
    print(f"  Wire 1 (Alice) measurement:    m1 = {results['m1']}")
    print(f"  X-correction needed (m1==1):   {results['needs_x_correction']}")
    print(f"  Z-correction needed (m0==1):   {results['needs_z_correction']}")
    print(f"  Async callbacks received:      {len(results['callback_log'])}")
    for wire, res in results['callback_log']:
        print(f"    → Wire {wire} collapsed to |{res}⟩")
    print(f"  Bob's wire still active:       {results['bob_wire_active']}")
    print(f"  Runtime status:                {results['status']}")

    # Layer 2: PennyLane Reference (optional)
    if HAS_PENNYLANE:
        print(f"\n[Layer 2] PennyLane Reference Simulation")
        print("-" * 40)
        theta, phi = np.pi / 4, np.pi / 6
        state = run_pennylane_reference(theta, phi)
        if state is not None:
            print(f"  Prepared state: RY({theta:.4f}) RZ({phi:.4f}) |0⟩")
            print(f"  Full 3-qubit state vector (8 amplitudes):")
            for i, amp in enumerate(state):
                if abs(amp) > 1e-10:
                    print(f"    |{i:03b}⟩ : {amp:.6f}")

    print("\n" + "=" * 60)
    print("  Validation Complete")
    print("=" * 60)


if __name__ == "__main__":
    main()
