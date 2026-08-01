/**
 * @file capi.h
 * @brief Public C-API for the Catalyst Mid-Circuit Measurement (MCM) Runtime.
 *
 * This header defines the complete Foreign Function Interface (FFI) contract
 * between the PennyLane-Catalyst C++ runtime and the Rust-based MCM backend.
 * It is consumed by the `bindgen` crate (via build.rs) to auto-generate
 * type-safe Rust bindings.
 *
 * All functions use C linkage and return status codes. Opaque handle types
 * hide internal Rust memory layout from C++ consumers.
 *
 * @note This is a pure C11 header. Do not add C++ constructs.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef CATALYST_MCM_CAPI_H
#define CATALYST_MCM_CAPI_H

#include <stdint.h>   /* Fixed-width integers: uint64_t, int32_t, etc.       */
#include <stdbool.h>  /* C99 boolean type: bool, true, false                 */
#include <stddef.h>   /* size_t for buffer/array sizes                       */

#ifdef __cplusplus
extern "C" {          /* Force C linkage so symbols are not name-mangled     */
#endif

/* =========================================================================
 * Section 1: Opaque Handle Types
 *
 * These structs are forward-declared only. Their internal fields are defined
 * exclusively in Rust. C/C++ code must NEVER dereference these pointers —
 * only pass them to the API functions below.
 *
 * WHY: Hiding internals guarantees that changing Rust struct layouts does
 *      not silently corrupt C++ code. This is the cornerstone of safe FFI.
 * ========================================================================= */

/**
 * Opaque handle representing the MCM runtime instance.
 *
 * Created by mcm_runtime_create(). Destroyed by mcm_runtime_destroy().
 * All other API calls require a valid pointer to this type.
 */
typedef struct McmRuntime McmRuntime;

/**
 * Opaque handle for a single qubit managed by the runtime.
 *
 * Qubit identity is tracked internally via a uint64_t wire index,
 * but the caller only ever sees this opaque pointer.
 * Reserved for future use when direct qubit handle passing is needed.
 */
typedef struct QubitHandle QubitHandle;

/* =========================================================================
 * Section 2: Status Codes
 *
 * Every API function returns one of these status codes. Callers MUST check
 * the return value after every call.
 *
 * WHY: Enums are self-documenting. When MCM_STATUS_INVALID_QUBIT appears
 *      in a log, the cause is immediately clear. A raw integer tells you
 *      nothing. In Rust, these become exhaustive match arms — the compiler
 *      warns if a case is unhandled.
 * ========================================================================= */

/**
 * Status codes returned by API functions.
 */
typedef enum {
    /** Operation completed successfully. */
    MCM_STATUS_OK              = 0,

    /** Supplied wire_id does not refer to an allocated qubit. */
    MCM_STATUS_INVALID_QUBIT   = 1,

    /** Internal async runtime failure (e.g., Tokio panic). */
    MCM_STATUS_RUNTIME_ERROR   = 2,

    /** The qubit on this wire was already measured (double-measure guard). */
    MCM_STATUS_ALREADY_MEASURED = 3,

    /** Qubit pool exhausted or wire_id exceeds max_qubits capacity. */
    MCM_STATUS_ALLOCATION_FAIL = 4
} McmStatus;

/* =========================================================================
 * Section 3: Measurement Result
 *
 * In quantum computing, measuring a qubit collapses its superposition
 * state to one of two classical outcomes: |0> or |1>. This enum carries
 * that classical bit back across the FFI boundary to the caller.
 * ========================================================================= */

/**
 * Classical result of a mid-circuit measurement.
 */
typedef enum {
    /** The qubit collapsed to the |0> state. */
    MCM_RESULT_ZERO = 0,

    /** The qubit collapsed to the |1> state. */
    MCM_RESULT_ONE  = 1
} McmMeasurementResult;

/* =========================================================================
 * Section 4: Callback Function Pointer Type
 *
 * When the Rust runtime completes a mid-circuit measurement, it invokes
 * this callback to notify the Catalyst scheduler of the result.
 *
 * WHY a callback instead of a return value? Mid-circuit measurements in
 * real quantum hardware are asynchronous. The measurement command is
 * dispatched, and the result arrives later (microseconds to milliseconds).
 * A callback lets the Rust runtime fire the result whenever it is ready,
 * rather than blocking the entire circuit execution pipeline.
 * ========================================================================= */

/**
 * Signature for the measurement result callback.
 *
 * @param wire_id  The logical wire index of the measured qubit (0-based).
 * @param result   The classical measurement outcome (ZERO or ONE).
 * @param ctx      An opaque user-provided context pointer. The Rust runtime
 *                 will NEVER dereference this pointer; it is passed through
 *                 so the C++ side can identify which circuit execution
 *                 this callback belongs to.
 */
typedef void (*McmMeasurementCallback)(
    uint64_t wire_id,
    McmMeasurementResult result,
    void* ctx
);

/* =========================================================================
 * Section 5: Lifecycle Management
 *
 * These functions create and destroy the runtime instance. Because C has
 * no destructors (unlike Rust's RAII or C++ destructors), the caller must
 * explicitly call mcm_runtime_destroy() when finished.
 *
 * OWNERSHIP CONTRACT:
 *   - mcm_runtime_create() transfers ownership TO the caller.
 *   - mcm_runtime_destroy() transfers ownership FROM the caller back
 *     to Rust, which frees all associated resources.
 * ========================================================================= */

/**
 * Create a new MCM runtime instance.
 *
 * Pre-allocates internal concurrent maps for the specified qubit capacity.
 * Initializes the Tokio async runtime for callback dispatch.
 *
 * @param max_qubits  Maximum number of qubits this runtime will manage.
 *                    Wire IDs must be in the range [0, max_qubits).
 * @return            An opaque pointer to the runtime, or NULL on failure.
 *
 * OWNERSHIP: Caller takes ownership. Must call mcm_runtime_destroy().
 */
McmRuntime* mcm_runtime_create(uint64_t max_qubits);

/**
 * Destroy the runtime and free all associated resources.
 *
 * After this call, the pointer is dangling. Any further use is
 * undefined behavior.
 *
 * @param runtime  The runtime to destroy. NULL is safely ignored.
 */
void mcm_runtime_destroy(McmRuntime* runtime);

/* =========================================================================
 * Section 6: Qubit Management
 *
 * Qubits are identified by "wire indices" — sequential integers
 * representing logical lines in the quantum circuit diagram. This matches
 * how Catalyst's MLIR representation addresses qubits, so our API aligns
 * directly with the intermediate representation (IR).
 * ========================================================================= */

/**
 * Allocate a new qubit on the specified wire.
 *
 * @param runtime  The active runtime instance (borrowed, not consumed).
 * @param wire_id  The logical wire index to assign (0-based).
 *                 Must be less than max_qubits.
 * @return         MCM_STATUS_OK on success.
 *                 MCM_STATUS_ALLOCATION_FAIL if wire_id >= max_qubits
 *                 or the wire is already in use.
 */
McmStatus mcm_qubit_allocate(McmRuntime* runtime, uint64_t wire_id);

/**
 * Release a qubit, returning its wire to the free pool.
 *
 * @param runtime  The active runtime instance.
 * @param wire_id  The wire index of the qubit to release.
 * @return         MCM_STATUS_OK on success.
 *                 MCM_STATUS_INVALID_QUBIT if the wire was not allocated.
 */
McmStatus mcm_qubit_release(McmRuntime* runtime, uint64_t wire_id);

/**
 * Query the total number of currently allocated (active) qubits.
 *
 * @param runtime  The active runtime instance (immutable borrow).
 * @return         The count of active qubits.
 */
uint64_t mcm_qubit_count(const McmRuntime* runtime);

/* =========================================================================
 * Section 7: Mid-Circuit Measurement
 *
 * This is the core capability of the entire runtime. It validates the
 * target qubit, executes (or simulates) the measurement, stores the
 * result, and optionally fires the registered callback asynchronously.
 * ========================================================================= */

/**
 * Perform a mid-circuit measurement on the specified qubit.
 *
 * Workflow:
 *   1. Validates the qubit is allocated and not already measured.
 *   2. Simulates or dispatches the measurement.
 *   3. Stores the result internally for conditional checks.
 *   4. If a callback is registered, fires it asynchronously via Tokio.
 *
 * @param runtime  The active runtime instance.
 * @param wire_id  The wire index of the qubit to measure.
 * @param result   OUT pointer. On success, receives the measurement outcome.
 *                 Caller allocates this; runtime writes to it.
 * @return         MCM_STATUS_OK on success.
 *                 MCM_STATUS_INVALID_QUBIT if wire is not allocated.
 *                 MCM_STATUS_ALREADY_MEASURED if qubit was already measured.
 */
McmStatus mcm_measure(
    McmRuntime* runtime,
    uint64_t wire_id,
    McmMeasurementResult* result
);

/**
 * Register a callback that fires whenever any qubit is measured.
 *
 * Only one callback can be active at a time. Registering a new callback
 * replaces the previous one.
 *
 * @param runtime   The active runtime instance.
 * @param callback  The function pointer to invoke on measurement.
 *                  Pass NULL to deregister the current callback.
 * @param ctx       An opaque context pointer passed through to the callback.
 *                  The runtime will NEVER dereference this pointer.
 * @return          MCM_STATUS_OK on success.
 *
 * ASYNC SAFETY WARNING:
 *   Callbacks are dispatched asynchronously on a background thread. This
 *   means that after mcm_measure() returns, the callback may still be
 *   in-flight or queued. Therefore:
 *     - The memory behind `ctx` MUST remain valid until BOTH:
 *       (a) the callback has been deregistered (pass NULL), AND
 *       (b) all prior mcm_measure() calls have fully completed their
 *           callback invocations.
 *     - To safely free ctx: deregister the callback, then call
 *       mcm_runtime_destroy() (which flushes all pending callbacks),
 *       then free ctx.
 *     - Freeing ctx immediately after deregistration without flushing
 *       may cause use-after-free if a callback is still in-flight.
 */
McmStatus mcm_register_measurement_callback(
    McmRuntime* runtime,
    McmMeasurementCallback callback,
    void* ctx
);

/* =========================================================================
 * Section 8: Conditional Execution (Dynamic Circuit Control Flow)
 *
 * This is what makes mid-circuit measurement USEFUL. Without conditional
 * logic, measuring mid-circuit is pointless — you would just measure at
 * the end. The power comes from using the measurement result DURING the
 * circuit to decide what gates to apply next.
 *
 * Example: "if measured(q0) == 1, then apply X(q1)"
 *
 * This capability is called "dynamic circuits" or "classical feedforward"
 * in quantum computing literature.
 * ========================================================================= */

/**
 * Evaluate a classical condition based on a prior measurement result.
 *
 * @param runtime       The active runtime instance (immutable borrow).
 * @param wire_id       The wire whose measurement result to check.
 * @param expected      The expected measurement outcome to compare against.
 * @param condition_met OUT pointer. Set to true if the stored measurement
 *                      result matches the expected value, false otherwise.
 *                      Caller allocates this; runtime writes to it.
 * @return              MCM_STATUS_OK on success.
 *                      MCM_STATUS_INVALID_QUBIT if the wire was never measured.
 */
McmStatus mcm_conditional_check(
    const McmRuntime* runtime,
    uint64_t wire_id,
    McmMeasurementResult expected,
    bool* condition_met
);

/* =========================================================================
 * Section 9: Telemetry / Diagnostics
 *
 * Provides human-readable runtime state for debugging and logging.
 * ========================================================================= */

/**
 * Return a human-readable string describing the runtime's current state.
 *
 * Includes: active qubit count, callback registration status, and
 * internal health indicators.
 *
 * @param runtime  The active runtime instance (immutable borrow).
 * @return         A null-terminated C string. The pointer is valid until
 *                 the next call to mcm_runtime_status_string() or
 *                 mcm_runtime_destroy(). Do NOT free this pointer.
 *
 * THREAD SAFETY:
 *   This function is NOT safe to call concurrently from multiple threads.
 *   Each call overwrites the internal string buffer, invalidating any
 *   pointer returned by a previous call. Callers must serialize access
 *   or copy the string immediately after each call.
 */
const char* mcm_runtime_status_string(const McmRuntime* runtime);

/* ========================================================================= */

#ifdef __cplusplus
}  /* end extern "C" */
#endif

#endif /* CATALYST_MCM_CAPI_H */
