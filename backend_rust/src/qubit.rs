//! Qubit allocation, wire tracking, and state management.
//!
//! This module provides [`QubitRegistry`], a thread-safe concurrent map that
//! tracks which wire indices are actively allocated and stores per-qubit
//! metadata including measurement results.
//!
//! # Thread Safety
//!
//! `QubitRegistry` uses [`DashMap`] internally, which employs lock striping
//! (sharded locking) to allow truly parallel access to different wire IDs.
//! This is critical because multiple Tokio tasks may allocate, release, or
//! measure qubits concurrently during asynchronous circuit execution.
//!
//! # Wire Indices
//!
//! In quantum circuit compilers like Catalyst, qubits are identified by
//! sequential non-negative integers called "wire indices". A circuit with
//! `max_qubits = 64` uses wire IDs in the range `[0, 64)`. This module
//! enforces that range at allocation time.

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use log::{debug, warn};

use crate::bindings::{
    McmStatus_MCM_STATUS_ALLOCATION_FAIL, McmStatus_MCM_STATUS_ALREADY_MEASURED,
    McmStatus_MCM_STATUS_INVALID_QUBIT,
};

/// Per-qubit metadata stored in the registry.
///
/// Each allocated qubit has a `QubitState` entry tracking its wire identity
/// and whether it has been measured.
#[derive(Debug, Clone)]
pub(crate) struct QubitState {
    /// The classical measurement result, if the qubit has been measured.
    ///
    /// - `None` means the qubit has not been measured yet.
    /// - `Some(MCM_RESULT_ZERO)` means it collapsed to |0⟩.
    /// - `Some(MCM_RESULT_ONE)` means it collapsed to |1⟩.
    ///
    /// Once set, this value is immutable — measuring the same qubit twice
    /// is an error (the double-measure guard).
    pub measurement_result: Option<u32>,
}

/// Thread-safe registry for tracking allocated qubits and their state.
///
/// Wraps a [`DashMap<u64, QubitState>`] with a maximum capacity constraint.
/// All public methods are `&self` (shared reference), meaning multiple
/// threads can call them concurrently without external synchronization.
#[derive(Debug)]
pub(crate) struct QubitRegistry {
    /// The concurrent map from wire_id to qubit state.
    /// DashMap internally shards the map across ~CPU-count segments,
    /// so operations on different wire IDs never contend.
    active_qubits: DashMap<u64, QubitState>,

    /// The maximum number of qubits this registry can hold.
    /// Wire IDs must be in the range [0, max_qubits).
    max_qubits: u64,
}

impl QubitRegistry {
    /// Create a new empty registry with the given capacity limit.
    ///
    /// Pre-allocates internal storage for `max_qubits` entries to avoid
    /// expensive runtime resizing during circuit execution.
    pub fn new(max_qubits: u64) -> Self {
        Self {
            active_qubits: DashMap::with_capacity(max_qubits as usize),
            max_qubits,
        }
    }

    /// Allocate a qubit on the specified wire.
    ///
    /// # Errors
    /// - `MCM_STATUS_ALLOCATION_FAIL` if `wire_id >= max_qubits` (out of range).
    /// - `MCM_STATUS_ALLOCATION_FAIL` if the wire is already allocated.
    pub fn allocate(&self, wire_id: u64) -> Result<(), u32> {
        // Guard: wire_id must be within the declared capacity.
        if wire_id >= self.max_qubits {
            warn!(
                "Allocation rejected: wire_id={} exceeds max_qubits={}",
                wire_id, self.max_qubits
            );
            return Err(McmStatus_MCM_STATUS_ALLOCATION_FAIL);
        }

        // Atomic check-and-insert via the Entry API.
        // This holds the shard's write lock for the entire operation,
        // preventing the TOCTOU race that a separate contains_key+insert
        // would introduce under concurrent allocation.
        match self.active_qubits.entry(wire_id) {
            Entry::Occupied(_) => {
                warn!("Allocation rejected: wire_id={} already occupied", wire_id);
                Err(McmStatus_MCM_STATUS_ALLOCATION_FAIL)
            }
            Entry::Vacant(slot) => {
                slot.insert(QubitState {
                    measurement_result: None,
                });
                debug!("Qubit allocated on wire_id={}", wire_id);
                Ok(())
            }
        }
    }

    /// Release a qubit, returning its wire to the free pool.
    ///
    /// # Errors
    /// - `MCM_STATUS_INVALID_QUBIT` if the wire was not allocated.
    pub fn release(&self, wire_id: u64) -> Result<(), u32> {
        match self.active_qubits.remove(&wire_id) {
            Some(_) => {
                debug!("Qubit released from wire_id={}", wire_id);
                Ok(())
            }
            None => {
                warn!("Release rejected: wire_id={} not allocated", wire_id);
                Err(McmStatus_MCM_STATUS_INVALID_QUBIT)
            }
        }
    }

    /// Return the number of currently allocated (active) qubits.
    pub fn count(&self) -> u64 {
        self.active_qubits.len() as u64
    }

    /// Check whether a wire is currently allocated.
    pub fn is_allocated(&self, wire_id: u64) -> bool {
        self.active_qubits.contains_key(&wire_id)
    }

    /// Store a measurement result for an allocated qubit.
    ///
    /// # Errors
    /// - `MCM_STATUS_INVALID_QUBIT` if the wire is not allocated.
    /// - `MCM_STATUS_ALREADY_MEASURED` if the qubit was already measured
    ///   (double-measure guard).
    pub fn set_measurement(&self, wire_id: u64, result: u32) -> Result<(), u32> {
        // We need a mutable reference to the state to update the measurement result.
        // DashMap's `get_mut` provides a lock guard for the specific entry.
        let mut state = self.active_qubits.get_mut(&wire_id).ok_or_else(|| {
            warn!("Measurement rejected: wire_id={} not allocated", wire_id);
            McmStatus_MCM_STATUS_INVALID_QUBIT
        })?;

        // Guard: A qubit can only be measured once.
        if state.measurement_result.is_some() {
            warn!("Measurement rejected: wire_id={} already measured", wire_id);
            return Err(McmStatus_MCM_STATUS_ALREADY_MEASURED);
        }

        // Store the result.
        state.measurement_result = Some(result);
        debug!(
            "Measurement stored for wire_id={}: result={}",
            wire_id, result
        );
        Ok(())
    }

    /// Retrieve the measurement result for a given wire, if it has been measured.
    ///
    /// Returns `None` if the wire is not allocated or has not been measured.
    pub fn get_measurement(&self, wire_id: u64) -> Option<u32> {
        self.active_qubits
            .get(&wire_id)
            .and_then(|entry| entry.measurement_result)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::McmStatus_MCM_STATUS_OK;

    /// Helper: create a registry with capacity for 64 qubits.
    fn make_registry() -> QubitRegistry {
        QubitRegistry::new(64)
    }

    #[test]
    fn allocate_single_qubit() {
        let reg = make_registry();
        assert!(reg.allocate(0).is_ok());
        assert_eq!(reg.count(), 1);
        assert!(reg.is_allocated(0));
    }

    #[test]
    fn allocate_multiple_qubits() {
        let reg = make_registry();
        for wire in 0..10 {
            assert!(reg.allocate(wire).is_ok());
        }
        assert_eq!(reg.count(), 10);
    }

    #[test]
    fn allocate_duplicate_wire_fails() {
        let reg = make_registry();
        assert!(reg.allocate(5).is_ok());
        let err = reg.allocate(5).unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_ALLOCATION_FAIL);
    }

    #[test]
    fn allocate_beyond_max_fails() {
        let reg = make_registry(); // max = 64
        let err = reg.allocate(100).unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_ALLOCATION_FAIL);
    }

    #[test]
    fn release_allocated_qubit() {
        let reg = make_registry();
        reg.allocate(0).unwrap();
        assert_eq!(reg.count(), 1);
        assert!(reg.release(0).is_ok());
        assert_eq!(reg.count(), 0);
        assert!(!reg.is_allocated(0));
    }

    #[test]
    fn release_unallocated_fails() {
        let reg = make_registry();
        let err = reg.release(99).unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_INVALID_QUBIT);
    }

    #[test]
    fn measurement_lifecycle() {
        let reg = make_registry();
        reg.allocate(3).unwrap();

        // Before measurement: no result stored.
        assert_eq!(reg.get_measurement(3), None);

        // Perform measurement: store result ZERO (0).
        assert!(reg.set_measurement(3, McmStatus_MCM_STATUS_OK).is_ok());

        // After measurement: result is retrievable.
        assert_eq!(reg.get_measurement(3), Some(McmStatus_MCM_STATUS_OK));
    }

    #[test]
    fn double_measurement_fails() {
        let reg = make_registry();
        reg.allocate(7).unwrap();

        // First measurement succeeds.
        assert!(reg.set_measurement(7, 0).is_ok());

        // Second measurement on same qubit must fail.
        let err = reg.set_measurement(7, 1).unwrap_err();
        assert_eq!(err, McmStatus_MCM_STATUS_ALREADY_MEASURED);
    }
}
