//! FFI boundary — all `#[no_mangle] extern "C"` entry points.
//!
//! This module translates between the raw C world and the safe Rust world.
//! It upholds strict safety guarantees across the FFI boundary.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use log::{error, info, warn};
use std::ffi::{c_char, c_void};

use crate::bindings::{
    McmMeasurementCallback, McmMeasurementResult, McmRuntime, McmStatus, McmStatus_MCM_STATUS_OK,
    McmStatus_MCM_STATUS_RUNTIME_ERROR,
};
use crate::runtime::McmRuntimeCore;

/// Create a new MCM runtime instance.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_runtime_create(max_qubits: u64) -> *mut McmRuntime {
    // Initialize env_logger on first call. Silently succeeds if already initialized.
    let _ = env_logger::try_init();

    info!("Creating MCM runtime with max_qubits={}", max_qubits);
    match McmRuntimeCore::new(max_qubits) {
        Ok(core) => {
            info!("MCM runtime created successfully");
            Box::into_raw(Box::new(core)) as *mut McmRuntime
        }
        Err(_) => {
            error!("Failed to create MCM runtime");
            std::ptr::null_mut()
        }
    }
}

/// Destroy the runtime and free all associated resources.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_runtime_destroy(runtime: *mut McmRuntime) {
    if runtime.is_null() {
        warn!("mcm_runtime_destroy called with null pointer");
        return;
    }
    info!("Destroying MCM runtime");
    // SAFETY: Ownership was given to C via into_raw; this reclaims it.
    unsafe {
        drop(Box::from_raw(runtime as *mut McmRuntimeCore));
    }
}

/// Allocate a new qubit on the specified wire.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_qubit_allocate(runtime: *mut McmRuntime, wire_id: u64) -> McmStatus {
    if runtime.is_null() {
        warn!("mcm_qubit_allocate called with null runtime pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    match core.qubit_registry.allocate(wire_id) {
        Ok(()) => McmStatus_MCM_STATUS_OK,
        Err(status) => status,
    }
}

/// Release a qubit, returning its wire to the free pool.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_qubit_release(runtime: *mut McmRuntime, wire_id: u64) -> McmStatus {
    if runtime.is_null() {
        warn!("mcm_qubit_release called with null runtime pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    match core.qubit_registry.release(wire_id) {
        Ok(()) => McmStatus_MCM_STATUS_OK,
        Err(status) => status,
    }
}

/// Query the total number of currently allocated (active) qubits.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_qubit_count(runtime: *const McmRuntime) -> u64 {
    if runtime.is_null() {
        warn!("mcm_qubit_count called with null runtime pointer");
        return 0;
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    core.qubit_registry.count()
}

/// Perform a mid-circuit measurement on the specified qubit.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_measure(
    runtime: *mut McmRuntime,
    wire_id: u64,
    result: *mut McmMeasurementResult,
) -> McmStatus {
    if runtime.is_null() {
        warn!("mcm_measure called with null runtime pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    if result.is_null() {
        warn!("mcm_measure called with null result pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    match core.measure_qubit(wire_id) {
        Ok(outcome) => {
            unsafe {
                *result = outcome;
            }
            McmStatus_MCM_STATUS_OK
        }
        Err(status) => status,
    }
}

/// Register a callback that fires whenever any qubit is measured.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_register_measurement_callback(
    runtime: *mut McmRuntime,
    callback: McmMeasurementCallback,
    ctx: *mut c_void,
) -> McmStatus {
    if runtime.is_null() {
        warn!("mcm_register_measurement_callback called with null runtime pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    core.register_callback(callback, ctx);
    McmStatus_MCM_STATUS_OK
}

/// Evaluate a classical condition based on a prior measurement result.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_conditional_check(
    runtime: *const McmRuntime,
    wire_id: u64,
    expected: McmMeasurementResult,
    condition_met: *mut bool,
) -> McmStatus {
    if runtime.is_null() {
        warn!("mcm_conditional_check called with null runtime pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    if condition_met.is_null() {
        warn!("mcm_conditional_check called with null condition_met pointer");
        return McmStatus_MCM_STATUS_RUNTIME_ERROR;
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    match core.evaluate_condition(wire_id, expected) {
        Ok(met) => {
            unsafe {
                *condition_met = met;
            }
            McmStatus_MCM_STATUS_OK
        }
        Err(status) => status,
    }
}

/// Return a human-readable string describing the runtime's current state.
#[unsafe(no_mangle)]
pub extern "C" fn mcm_runtime_status_string(runtime: *const McmRuntime) -> *const c_char {
    if runtime.is_null() {
        warn!("mcm_runtime_status_string called with null runtime pointer");
        return std::ptr::null();
    }
    let core = unsafe { &*(runtime as *const McmRuntimeCore) };
    core.status_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::McmMeasurementResult_MCM_RESULT_ZERO;
    use std::ptr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_create_and_destroy() {
        let runtime = mcm_runtime_create(10);
        assert!(!runtime.is_null());

        // This should not crash or leak memory.
        mcm_runtime_destroy(runtime);

        // Null destroy should be a safe no-op.
        mcm_runtime_destroy(ptr::null_mut());
    }

    #[test]
    fn test_full_lifecycle() {
        let runtime = mcm_runtime_create(10);
        assert!(!runtime.is_null());

        // Allocate qubit 0
        assert_eq!(mcm_qubit_allocate(runtime, 0), McmStatus_MCM_STATUS_OK);
        assert_eq!(mcm_qubit_count(runtime), 1);

        // Measure qubit 0
        let mut measure_result: McmMeasurementResult = 999;
        assert_eq!(
            mcm_measure(runtime, 0, &mut measure_result),
            McmStatus_MCM_STATUS_OK
        );
        assert!(measure_result == 0 || measure_result == 1);

        // Conditional check
        let mut condition_met: bool = false;
        assert_eq!(
            mcm_conditional_check(runtime, 0, measure_result, &mut condition_met),
            McmStatus_MCM_STATUS_OK
        );
        assert!(condition_met);

        // Status string
        let status_ptr = mcm_runtime_status_string(runtime);
        assert!(!status_ptr.is_null());
        let c_str = unsafe { std::ffi::CStr::from_ptr(status_ptr) };
        let s = c_str.to_str().unwrap();
        assert!(s.contains("active_qubits=1"));

        // Release qubit 0
        assert_eq!(mcm_qubit_release(runtime, 0), McmStatus_MCM_STATUS_OK);
        assert_eq!(mcm_qubit_count(runtime), 0);

        mcm_runtime_destroy(runtime);
    }

    #[test]
    fn test_null_runtime_safety() {
        let null_rt: *mut McmRuntime = ptr::null_mut();

        assert_eq!(
            mcm_qubit_allocate(null_rt, 0),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );
        assert_eq!(
            mcm_qubit_release(null_rt, 0),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );
        assert_eq!(mcm_qubit_count(null_rt), 0);

        let mut res = McmMeasurementResult_MCM_RESULT_ZERO;
        assert_eq!(
            mcm_measure(null_rt, 0, &mut res),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );

        assert_eq!(
            mcm_register_measurement_callback(null_rt, None, ptr::null_mut()),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );

        let mut cond = false;
        assert_eq!(
            mcm_conditional_check(null_rt, 0, McmMeasurementResult_MCM_RESULT_ZERO, &mut cond),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );

        assert_eq!(mcm_runtime_status_string(null_rt), ptr::null());
    }

    #[test]
    fn test_null_out_pointer_safety() {
        let runtime = mcm_runtime_create(10);
        assert_eq!(mcm_qubit_allocate(runtime, 0), McmStatus_MCM_STATUS_OK);

        // Test null result pointer for measurement
        assert_eq!(
            mcm_measure(runtime, 0, ptr::null_mut()),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );

        // Measure properly for next test
        let mut res = McmMeasurementResult_MCM_RESULT_ZERO;
        mcm_measure(runtime, 0, &mut res);

        // Test null condition_met pointer for conditional check
        assert_eq!(
            mcm_conditional_check(
                runtime,
                0,
                McmMeasurementResult_MCM_RESULT_ZERO,
                ptr::null_mut()
            ),
            McmStatus_MCM_STATUS_RUNTIME_ERROR
        );

        mcm_runtime_destroy(runtime);
    }

    #[test]
    fn test_callback_through_ffi() {
        let runtime = mcm_runtime_create(10);
        assert_eq!(mcm_qubit_allocate(runtime, 0), McmStatus_MCM_STATUS_OK);

        // Shared atomic counter
        let counter = Arc::new(AtomicU64::new(0));
        let counter_ptr = Arc::into_raw(counter.clone());

        unsafe extern "C" fn test_callback(
            _wire_id: u64,
            _result: McmMeasurementResult,
            ctx: *mut c_void,
        ) {
            let counter = unsafe { &*(ctx as *const AtomicU64) };
            counter.fetch_add(1, Ordering::SeqCst);
        }

        // Register callback via FFI
        assert_eq!(
            mcm_register_measurement_callback(
                runtime,
                Some(test_callback),
                counter_ptr as *mut c_void
            ),
            McmStatus_MCM_STATUS_OK
        );

        // Measure qubit to trigger callback
        let mut res = McmMeasurementResult_MCM_RESULT_ZERO;
        assert_eq!(mcm_measure(runtime, 0, &mut res), McmStatus_MCM_STATUS_OK);

        // Wait for tokio to run the callback
        std::thread::sleep(std::time::Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Clean up
        unsafe {
            Arc::from_raw(counter_ptr);
        }
        mcm_runtime_destroy(runtime);
    }
}
