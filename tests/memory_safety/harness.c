/**
 * harness.c — Memory safety test harness for the MCM Runtime FFI.
 *
 * This program exercises every FFI endpoint in the MCM runtime library
 * and is designed to be run under Valgrind and AddressSanitizer to
 * detect memory leaks, use-after-free, and invalid memory accesses.
 *
 * Compile: gcc -o harness harness.c -I../../include -L../../backend_rust/target/debug -lmcm_runtime
 * Run:     LD_LIBRARY_PATH=../../backend_rust/target/debug valgrind --leak-check=full ./harness
 */

#include "catalyst_bindings/capi.h"
#include <assert.h>
#include <stdio.h>
#include <stdbool.h>

#include <unistd.h>

static void measurement_callback(uint64_t wire_id, McmMeasurementResult result, void* ctx) {
    (void)wire_id;
    (void)result;
    int* count = (int*)ctx;
    (*count)++;
}

/**
 * Scenario 1: Basic create/destroy — tests that an empty runtime
 * is fully freed with zero leaks.
 */
static void test_basic_lifecycle(void) {
    printf("[Scenario 1] Basic lifecycle...\n");
    McmRuntime* rt = mcm_runtime_create(64);
    assert(rt != NULL);
    mcm_runtime_destroy(rt);
    printf("  PASS\n");
}

/**
 * Scenario 2: Full teleportation protocol — exercises allocate,
 * measure, conditional_check, status_string, release, destroy.
 */
static void test_full_protocol(void) {
    printf("[Scenario 2] Full protocol...\n");
    McmRuntime* rt = mcm_runtime_create(16);
    assert(rt != NULL);

    assert(mcm_qubit_allocate(rt, 0) == MCM_STATUS_OK);
    assert(mcm_qubit_allocate(rt, 1) == MCM_STATUS_OK);
    assert(mcm_qubit_allocate(rt, 2) == MCM_STATUS_OK);
    assert(mcm_qubit_count(rt) == 3);

    int cb_count = 0;
    assert(mcm_register_measurement_callback(rt, measurement_callback, &cb_count) == MCM_STATUS_OK);

    McmMeasurementResult result;
    assert(mcm_measure(rt, 0, &result) == MCM_STATUS_OK);
    assert(result == MCM_RESULT_ZERO);
    assert(mcm_measure(rt, 1, &result) == MCM_STATUS_OK);
    assert(result == MCM_RESULT_ONE);

    bool met;
    assert(mcm_conditional_check(rt, 0, MCM_RESULT_ZERO, &met) == MCM_STATUS_OK);
    assert(met == true);
    assert(mcm_conditional_check(rt, 1, MCM_RESULT_ONE, &met) == MCM_STATUS_OK);
    assert(met == true);

    const char* status = mcm_runtime_status_string(rt);
    assert(status != NULL);

    assert(mcm_qubit_release(rt, 0) == MCM_STATUS_OK);
    assert(mcm_qubit_release(rt, 1) == MCM_STATUS_OK);
    assert(mcm_qubit_release(rt, 2) == MCM_STATUS_OK);
    assert(mcm_qubit_count(rt) == 0);

    usleep(200000);  /* 200ms — allow async callbacks to complete */

    mcm_runtime_destroy(rt);
    printf("  Callbacks fired: %d\n", cb_count);
    printf("  PASS\n");
}

/**
 * Scenario 3: Callback race — measure and immediately destroy,
 * testing that Tokio's Runtime::drop() safely joins worker threads.
 */
static void test_callback_race(void) {
    printf("[Scenario 3] Callback race window...\n");
    for (int i = 0; i < 10; i++) {
        McmRuntime* rt = mcm_runtime_create(8);
        assert(rt != NULL);

        int cb_count = 0;
        mcm_register_measurement_callback(rt, measurement_callback, &cb_count);

        mcm_qubit_allocate(rt, 0);
        mcm_qubit_allocate(rt, 1);

        McmMeasurementResult r;
        mcm_measure(rt, 0, &r);
        mcm_measure(rt, 1, &r);

        /* Destroy immediately — no sleep. Tokio must join cleanly. */
        mcm_runtime_destroy(rt);
    }
    printf("  PASS (10 iterations, no use-after-free)\n");
}

/**
 * Scenario 4: Null pointer safety — all endpoints must handle
 * NULL without crashing or accessing invalid memory.
 */
static void test_null_safety(void) {
    printf("[Scenario 4] Null safety...\n");
    mcm_runtime_destroy(NULL);
    assert(mcm_qubit_allocate(NULL, 0) == MCM_STATUS_RUNTIME_ERROR);
    assert(mcm_qubit_release(NULL, 0) == MCM_STATUS_RUNTIME_ERROR);
    assert(mcm_qubit_count(NULL) == 0);

    McmMeasurementResult r;
    assert(mcm_measure(NULL, 0, &r) == MCM_STATUS_RUNTIME_ERROR);

    bool met;
    assert(mcm_conditional_check(NULL, 0, MCM_RESULT_ZERO, &met) == MCM_STATUS_RUNTIME_ERROR);

    assert(mcm_runtime_status_string(NULL) == NULL);

    /* Also test null output pointers with a valid runtime */
    McmRuntime* rt = mcm_runtime_create(8);
    assert(rt != NULL);
    mcm_qubit_allocate(rt, 0);
    assert(mcm_measure(rt, 0, NULL) == MCM_STATUS_RUNTIME_ERROR);
    assert(mcm_conditional_check(rt, 0, MCM_RESULT_ZERO, NULL) == MCM_STATUS_RUNTIME_ERROR);
    mcm_runtime_destroy(rt);

    printf("  PASS\n");
}

/**
 * Scenario 5: Rapid lifecycle stress — 100 create/use/destroy cycles
 * to detect cumulative leaks that single-cycle tests miss.
 */
static void test_rapid_cycles(void) {
    printf("[Scenario 5] Rapid lifecycle stress (100 cycles)...\n");
    for (int i = 0; i < 100; i++) {
        McmRuntime* rt = mcm_runtime_create(8);
        assert(rt != NULL);

        mcm_qubit_allocate(rt, 0);
        mcm_qubit_allocate(rt, 1);

        McmMeasurementResult r;
        mcm_measure(rt, 0, &r);

        bool met;
        mcm_conditional_check(rt, 0, MCM_RESULT_ZERO, &met);

        const char* s = mcm_runtime_status_string(rt);
        (void)s;

        mcm_runtime_destroy(rt);
    }
    printf("  PASS\n");
}

int main(void) {
    printf("============================================================\n");
    printf("  MCM Runtime — Memory Safety Test Harness\n");
    printf("============================================================\n\n");

    test_basic_lifecycle();
    test_full_protocol();
    test_callback_race();
    test_null_safety();
    test_rapid_cycles();

    printf("\n============================================================\n");
    printf("  All scenarios passed. Check Valgrind/ASAN output above.\n");
    printf("============================================================\n");
    return 0;
}
