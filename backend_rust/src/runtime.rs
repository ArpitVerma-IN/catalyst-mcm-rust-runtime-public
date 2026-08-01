//! Core MCM runtime struct and lifecycle management.
//!
//! [`McmRuntimeCore`] is the central orchestrator that owns every subsystem:
//!
//! - A [`QubitRegistry`] for thread-safe qubit allocation and wire tracking.
//! - A [`tokio::runtime::Runtime`] for asynchronous callback dispatch.
//! - A [`CallbackRegistration`] slot for the measurement result callback.
//! - A cached [`CString`] for the FFI-safe status string.
//!
//! All FFI entry points in `ffi.rs` will receive an opaque `*mut McmRuntime`
//! pointer, cast it to `&McmRuntimeCore`, and delegate to methods here.
//!
//! # Interior Mutability
//!
//! Because every FFI function receives a shared `&self` reference (the opaque
//! pointer is never exclusively owned during a call), mutable state is wrapped
//! in [`std::sync::Mutex`]. This moves the borrow-checking from compile time
//! to runtime — a necessary trade-off for FFI-safe concurrency.

use std::ffi::{CString, c_void};
use std::os::raw::c_char;
use std::sync::Mutex;

use log::{debug, info, trace};

use tokio::runtime::Runtime;

use crate::bindings::McmMeasurementResult;
use crate::qubit::QubitRegistry;

// =============================================================================
// Callback Registration
// =============================================================================

/// Stores a registered measurement callback and its opaque context pointer.
///
/// # Safety Contract (upheld by the C++ caller)
///
/// The `ctx` pointer is never dereferenced by Rust. It is passed through
/// verbatim to the callback function. The C++ side guarantees that:
///   1. The pointed-to data remains valid until the callback is deregistered.
///   2. The pointed-to data is safe to access from any thread (because Tokio
///      may fire the callback on a background worker thread).
pub(crate) struct CallbackRegistration {
    /// The C function pointer: `void (*)(uint64_t, McmMeasurementResult, void*)`.
    /// This is the *unwrapped* form — we store it after confirming it's `Some`.
    pub func: unsafe extern "C" fn(u64, McmMeasurementResult, *mut c_void),

    /// Opaque context pointer, passed through to `func` on every invocation.
    pub ctx: *mut c_void,
}

// SAFETY: Raw pointers are not Send/Sync by default. We manually assert
// thread-safety because:
//   - `func` is a plain C function pointer (no captured state, always safe to call
//     from any thread — function pointers are inherently thread-safe).
//   - `ctx` is never dereferenced by Rust; the safety burden is on the C++ caller
//     who promises the target data is valid and thread-safe.
unsafe impl Send for CallbackRegistration {}
unsafe impl Sync for CallbackRegistration {}

// =============================================================================
// McmRuntimeCore
// =============================================================================

/// The central runtime orchestrator.
///
/// Created once via [`McmRuntimeCore::new`], held behind an opaque FFI pointer,
/// and destroyed when [`ffi::mcm_runtime_destroy`] calls [`Box::from_raw`].
///
/// All methods take `&self` (shared reference) and use interior mutability
/// where mutation is needed, making the struct safe to share across threads.
pub(crate) struct McmRuntimeCore {
    /// Thread-safe qubit allocation and state management.
    pub(crate) qubit_registry: QubitRegistry,

    /// The Tokio multi-threaded async runtime.
    ///
    /// Used to `spawn` callback invocations on a background thread pool so
    /// that `mcm_measure()` returns immediately without blocking on the
    /// callback.
    pub(crate) tokio_runtime: Runtime,

    /// The currently registered measurement callback, if any.
    ///
    /// Protected by a `Mutex` because:
    ///   - Registration (`mcm_register_measurement_callback`) writes to this.
    ///   - Measurement (`mcm_measure` → `fire_callback`) reads from this.
    ///   - Both can happen concurrently from different threads.
    pub(crate) callback: Mutex<Option<CallbackRegistration>>,

    /// Cached status string for FFI return.
    ///
    /// The `mcm_runtime_status_string` function returns a `*const c_char`.
    /// That pointer must remain valid until the next call or until the runtime
    /// is destroyed. Storing the `CString` here guarantees the allocation lives
    /// long enough.
    status_cache: Mutex<CString>,
}

impl McmRuntimeCore {
    /// Create a new runtime with the given qubit capacity.
    ///
    /// Initializes the Tokio multi-threaded runtime and pre-allocates the
    /// qubit registry. Returns `Err(())` if Tokio initialization fails
    /// (extremely rare — typically only in environments where threads
    /// cannot be spawned).
    pub fn new(max_qubits: u64) -> Result<Self, ()> {
        debug!("Initializing Tokio async runtime");
        let tokio_runtime = Runtime::new().map_err(|_| ())?;

        Ok(Self {
            qubit_registry: QubitRegistry::new(max_qubits),
            tokio_runtime,
            callback: Mutex::new(None),
            // Initialize with a placeholder; overwritten on first status_string() call.
            status_cache: Mutex::new(CString::new("MCM Runtime: initializing").unwrap()),
        })
    }

    /// Register (or deregister) the measurement result callback.
    ///
    /// - Pass `Some(func)` + `ctx` to register a new callback.
    /// - Pass `None` to deregister the current callback.
    ///
    /// Only one callback can be active at a time. Registering a new callback
    /// silently replaces the previous one.
    pub fn register_callback(
        &self,
        func: Option<unsafe extern "C" fn(u64, McmMeasurementResult, *mut c_void)>,
        ctx: *mut c_void,
    ) {
        let mut guard = self
            .callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if func.is_some() {
            info!("Measurement callback registered");
        } else {
            info!("Measurement callback deregistered");
        }
        *guard = func.map(|f| CallbackRegistration { func: f, ctx });
    }

    /// Fire the registered callback asynchronously on Tokio's thread pool.
    ///
    /// If no callback is registered, this is a no-op.
    ///
    /// The callback is invoked via `tokio::spawn`, which dispatches it to a
    /// background worker thread. This means `fire_callback` returns immediately
    /// — the caller is never blocked waiting for the C++ callback to complete.
    ///
    /// # Why clone the registration data?
    ///
    /// We copy the function pointer and context pointer out of the `Mutex`
    /// before spawning. This avoids holding the lock across the async boundary
    /// (which would be a deadlock risk if the callback itself tried to
    /// register a new callback).
    pub fn fire_callback(&self, wire_id: u64, result: McmMeasurementResult) {
        // Snapshot the callback under the lock, then drop the lock immediately.
        let snapshot = {
            let guard = self
                .callback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.as_ref().map(|reg| (reg.func, reg.ctx))
        };

        if let Some((func, ctx)) = snapshot {
            debug!(
                "Dispatching callback for wire_id={}, result={}",
                wire_id, result
            );
            // Convert the raw pointer to a usize for thread-safe transfer.
            // usize is inherently Send+Sync, sidestepping Rust's (intentional)
            // prohibition on sending raw pointers across thread boundaries.
            // This is a well-established FFI pattern; the safety contract
            // remains: the C++ caller guarantees the data behind `ctx` is
            // valid and thread-safe.
            let ctx_addr = ctx as usize;

            self.tokio_runtime.spawn_blocking(move || {
                // SAFETY: The C++ caller guarantees that `func` is a valid
                // function pointer and `ctx_addr` was a valid pointer.
                unsafe {
                    func(wire_id, result, ctx_addr as *mut c_void);
                }
            });
        } else {
            trace!("No callback registered, skipping dispatch");
        }
    }

    /// Build and return a human-readable status string as a C-compatible pointer.
    ///
    /// The returned `*const c_char` is valid until the next call to
    /// `status_string()` or until the runtime is destroyed.
    ///
    /// # Panics
    ///
    /// Panics if the formatted string contains interior NUL bytes (should
    /// never happen with our controlled format string).
    pub fn status_string(&self) -> *const c_char {
        trace!("Status string requested");
        let active = self.qubit_registry.count();
        let has_callback = self
            .callback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some();

        let status = format!(
            "MCM Runtime: active_qubits={}, callback_registered={}",
            active, has_callback
        );

        let c_string =
            CString::new(status).expect("status string must not contain interior NUL bytes");

        let mut cache = self
            .status_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *cache = c_string;
        cache.as_ptr()
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Helper: create a runtime with capacity for 64 qubits.
    fn make_runtime() -> McmRuntimeCore {
        McmRuntimeCore::new(64).expect("Tokio runtime should initialize")
    }

    #[test]
    fn create_runtime() {
        let rt = make_runtime();
        assert_eq!(rt.qubit_registry.count(), 0);
        // Callback should start as None.
        assert!(rt.callback.lock().unwrap().is_none());
    }

    #[test]
    fn qubit_operations_through_runtime() {
        let rt = make_runtime();
        assert!(rt.qubit_registry.allocate(0).is_ok());
        assert!(rt.qubit_registry.allocate(1).is_ok());
        assert_eq!(rt.qubit_registry.count(), 2);
        assert!(rt.qubit_registry.release(0).is_ok());
        assert_eq!(rt.qubit_registry.count(), 1);
    }

    #[test]
    fn register_and_fire_callback() {
        let rt = make_runtime();

        // Shared atomic counter to verify callback invocation from another thread.
        let counter = Arc::new(AtomicU64::new(0));
        let counter_ptr = Arc::into_raw(counter.clone());

        // A C-compatible callback that increments the atomic counter.
        unsafe extern "C" fn test_callback(
            _wire_id: u64,
            _result: McmMeasurementResult,
            ctx: *mut c_void,
        ) {
            let counter = unsafe { &*(ctx as *const AtomicU64) };
            counter.fetch_add(1, Ordering::SeqCst);
        }

        // Register the callback with the counter as the context.
        rt.register_callback(Some(test_callback), counter_ptr as *mut c_void);

        // Fire the callback.
        rt.fire_callback(0, 0);

        // Give Tokio's thread pool a moment to execute the spawned task.
        std::thread::sleep(std::time::Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Clean up: reconstruct the Arc so it gets dropped properly.
        unsafe {
            Arc::from_raw(counter_ptr);
        }
    }

    #[test]
    fn fire_callback_without_registration() {
        let rt = make_runtime();
        // Should be a no-op, not a panic.
        rt.fire_callback(42, 1);
    }

    #[test]
    fn status_string_returns_valid_cstring() {
        let rt = make_runtime();
        rt.qubit_registry.allocate(0).unwrap();
        rt.qubit_registry.allocate(1).unwrap();

        let ptr = rt.status_string();
        assert!(!ptr.is_null());

        // Read the C string back into Rust to verify content.
        let c_str = unsafe { std::ffi::CStr::from_ptr(ptr) };
        let s = c_str.to_str().expect("status should be valid UTF-8");

        assert!(s.contains("active_qubits=2"));
        assert!(s.contains("callback_registered=false"));
    }
}
