//! Measurement execution, result storage, and callback dispatch.
//!
//! This module contains the `measure_qubit` function that validates qubit state,
//! simulates a measurement, stores the result, and fires the registered callback asynchronously.

use crate::bindings::{
    McmMeasurementResult, McmMeasurementResult_MCM_RESULT_ONE,
    McmMeasurementResult_MCM_RESULT_ZERO, McmStatus_MCM_STATUS_INVALID_QUBIT,
};
use crate::runtime::McmRuntimeCore;
use log::{debug, warn};

impl McmRuntimeCore {
    /// Perform a mid-circuit measurement on the specified qubit.
    ///
    /// Validates the qubit is allocated and not already measured.
    /// Simulates a measurement outcome (deterministic parity based on wire_id).
    /// Stores the result in the QubitRegistry and fires the registered callback.
    ///
    /// Returns the simulated measurement result on success, or an MCM_STATUS error code on failure.
    pub fn measure_qubit(&self, wire_id: u64) -> Result<McmMeasurementResult, u32> {
        debug!("Measuring qubit wire_id={}", wire_id);
        // Validate the wire is allocated. The QubitRegistry handles the
        // "already measured" check internally during `set_measurement`,
        // but we need to know it's allocated first if we want to fail fast,
        // although set_measurement also returns INVALID_QUBIT if not found.
        if !self.qubit_registry.is_allocated(wire_id) {
            warn!("Measurement failed: wire_id={} not allocated", wire_id);
            return Err(McmStatus_MCM_STATUS_INVALID_QUBIT);
        }

        // Simulate a measurement outcome based on wire parity for deterministic testing.
        // Even wires -> 0, Odd wires -> 1.
        let outcome = if wire_id.is_multiple_of(2) {
            McmMeasurementResult_MCM_RESULT_ZERO
        } else {
            McmMeasurementResult_MCM_RESULT_ONE
        };

        // Store the result. This will fail with MCM_STATUS_ALREADY_MEASURED
        // if the qubit has already been measured.
        self.qubit_registry.set_measurement(wire_id, outcome)?;

        // Fire the callback asynchronously.
        // fire_callback handles snapshotting and thread-safe dispatch internally.
        self.fire_callback(wire_id, outcome);

        debug!("Measurement outcome for wire_id={}: {}", wire_id, outcome);
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::{
        McmStatus_MCM_STATUS_ALREADY_MEASURED, McmStatus_MCM_STATUS_INVALID_QUBIT,
    };
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn make_runtime() -> McmRuntimeCore {
        McmRuntimeCore::new(64).unwrap()
    }

    #[test]
    fn measure_unallocated_fails() {
        let rt = make_runtime();
        let err = rt.measure_qubit(0).unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_INVALID_QUBIT);
    }

    #[test]
    fn measure_success_stores_result() {
        let rt = make_runtime();
        rt.qubit_registry.allocate(2).unwrap(); // Even -> ZERO

        let res = rt.measure_qubit(2).unwrap();
        assert_eq!(res, McmMeasurementResult_MCM_RESULT_ZERO);
        assert_eq!(
            rt.qubit_registry.get_measurement(2),
            Some(McmMeasurementResult_MCM_RESULT_ZERO)
        );

        rt.qubit_registry.allocate(3).unwrap(); // Odd -> ONE
        let res2 = rt.measure_qubit(3).unwrap();
        assert_eq!(res2, McmMeasurementResult_MCM_RESULT_ONE);
        assert_eq!(
            rt.qubit_registry.get_measurement(3),
            Some(McmMeasurementResult_MCM_RESULT_ONE)
        );
    }

    #[test]
    fn measure_twice_fails() {
        let rt = make_runtime();
        rt.qubit_registry.allocate(5).unwrap();
        rt.measure_qubit(5).unwrap();

        let err = rt.measure_qubit(5).unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_ALREADY_MEASURED);
    }

    // Shared state for the callback test
    struct CallbackState {
        wire_id: AtomicU64,
        result: AtomicU64,
        called: AtomicU64,
    }

    #[test]
    fn measure_fires_callback_with_correct_args() {
        let rt = make_runtime();
        rt.qubit_registry.allocate(7).unwrap(); // Odd wire -> ONE

        let state = Arc::new(CallbackState {
            wire_id: AtomicU64::new(999), // dummy initial
            result: AtomicU64::new(999),
            called: AtomicU64::new(0),
        });

        let state_ptr = Arc::into_raw(state.clone());

        unsafe extern "C" fn test_callback(
            wire_id: u64,
            result: McmMeasurementResult,
            ctx: *mut c_void,
        ) {
            let state = unsafe { &*(ctx as *const CallbackState) };
            state.wire_id.store(wire_id, Ordering::SeqCst);
            state.result.store(result as u64, Ordering::SeqCst);
            state.called.fetch_add(1, Ordering::SeqCst);
        }

        rt.register_callback(Some(test_callback), state_ptr as *mut c_void);

        // Measure wire 7
        rt.measure_qubit(7).unwrap();

        // Give Tokio's thread pool a moment to execute
        std::thread::sleep(std::time::Duration::from_millis(100));

        assert_eq!(state.called.load(Ordering::SeqCst), 1);
        assert_eq!(state.wire_id.load(Ordering::SeqCst), 7);
        assert_eq!(
            state.result.load(Ordering::SeqCst),
            McmMeasurementResult_MCM_RESULT_ONE as u64
        );

        unsafe {
            Arc::from_raw(state_ptr);
        }
    }
}
