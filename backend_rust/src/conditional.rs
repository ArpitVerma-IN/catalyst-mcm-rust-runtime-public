//! Conditional evaluation for dynamic circuit control flow.
//!
//! This module contains the `evaluate_condition` function that compares
//! a stored measurement result against an expected value, enabling
//! "if measured(q0) == 1, then apply X(q1)" style dynamic circuits.

use crate::bindings::{McmMeasurementResult, McmStatus_MCM_STATUS_INVALID_QUBIT};
use crate::runtime::McmRuntimeCore;
use log::{debug, warn};

impl McmRuntimeCore {
    /// Evaluate a classical condition based on a prior measurement result.
    ///
    /// Looks up the stored measurement result for `wire_id`. If the qubit has not
    /// been measured, returns an error. Otherwise, returns `true` if the stored
    /// result matches `expected`, and `false` otherwise.
    pub fn evaluate_condition(
        &self,
        wire_id: u64,
        expected: McmMeasurementResult,
    ) -> Result<bool, u32> {
        // get_measurement returns Option<u32>. If None, either the wire isn't
        // allocated or it hasn't been measured yet. Either way, we can't
        // evaluate a condition on it.
        let actual_result = self
            .qubit_registry
            .get_measurement(wire_id)
            .ok_or_else(|| {
                warn!(
                    "Condition evaluation failed: wire_id={} has no measurement",
                    wire_id
                );
                McmStatus_MCM_STATUS_INVALID_QUBIT
            })?;

        // Compare the actual stored result with the expected enum value.
        let met = actual_result == expected;
        debug!(
            "Condition evaluated for wire_id={}: expected={}, actual={}, met={}",
            wire_id, expected, actual_result, met
        );
        Ok(met)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::{
        McmMeasurementResult_MCM_RESULT_ONE, McmMeasurementResult_MCM_RESULT_ZERO,
    };

    fn make_runtime() -> McmRuntimeCore {
        McmRuntimeCore::new(64).unwrap()
    }

    #[test]
    fn evaluate_unmeasured_qubit_fails() {
        let rt = make_runtime();
        rt.qubit_registry.allocate(10).unwrap();

        let err = rt
            .evaluate_condition(10, McmMeasurementResult_MCM_RESULT_ZERO)
            .unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_INVALID_QUBIT);
    }

    #[test]
    fn evaluate_unallocated_qubit_fails() {
        let rt = make_runtime();

        let err = rt
            .evaluate_condition(99, McmMeasurementResult_MCM_RESULT_ONE)
            .unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_INVALID_QUBIT);
    }

    #[test]
    fn evaluate_matches_correctly() {
        let rt = make_runtime();
        rt.qubit_registry.allocate(2).unwrap(); // Even wire

        // Measure it to store ZERO
        rt.measure_qubit(2).unwrap();

        // Condition checking against ZERO should be true
        let matched = rt
            .evaluate_condition(2, McmMeasurementResult_MCM_RESULT_ZERO)
            .unwrap();
        assert!(matched);

        // Condition checking against ONE should be false
        let not_matched = rt
            .evaluate_condition(2, McmMeasurementResult_MCM_RESULT_ONE)
            .unwrap();
        assert!(!not_matched);
    }
}
