"""
conftest.py — pytest fixtures for MCM integration tests.

Provides shared fixtures at two levels:

- **Session-scoped**: ``ensure_lib_built`` runs ``cargo build`` once per session.
- **Function-scoped**: ``mcm_lib`` (raw ctypes handle) and ``runtime``
  (high-level McmRuntime context manager) are created fresh per test.
"""

import pathlib
import subprocess

import pytest

from mcm_ffi import McmRuntime, load_mcm_runtime

# Resolve paths relative to this file.
_PROJECT_ROOT = pathlib.Path(__file__).resolve().parent.parent
_BACKEND_DIR = _PROJECT_ROOT / "backend_rust"
_LIB_PATH = _BACKEND_DIR / "target" / "debug" / "libmcm_runtime.so"


# -------------------------------------------------------------------------
# Session-scoped: build the shared library once
# -------------------------------------------------------------------------


@pytest.fixture(scope="session", autouse=True)
def ensure_lib_built():
    """
    Run ``cargo build`` once at the start of the test session to ensure
    the shared library is current.

    This fixture is session-scoped and autouse, so it runs automatically
    before any test in any file — no manual build step needed.
    """
    result = subprocess.run(
        ["bash", "-lc", "source ~/.cargo/env && cargo build"],
        cwd=str(_BACKEND_DIR),
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        pytest.fail(f"cargo build failed:\n{result.stderr}")


# -------------------------------------------------------------------------
# Function-scoped: raw ctypes handle (backward compatible)
# -------------------------------------------------------------------------


@pytest.fixture
def mcm_lib():
    """
    Pytest fixture: loads and returns the raw ctypes library handle.

    Fails the test immediately if the .so file does not exist.
    """
    if not _LIB_PATH.exists():
        pytest.fail(
            f"Shared library not found at {_LIB_PATH}. "
            f"Run 'cargo build' in backend_rust/ first."
        )
    return load_mcm_runtime(str(_LIB_PATH))


# -------------------------------------------------------------------------
# Function-scoped: high-level McmRuntime wrapper
# -------------------------------------------------------------------------


@pytest.fixture
def runtime():
    """
    Pytest fixture: yields a fresh McmRuntime instance (max_qubits=64)
    wrapped in a context manager for automatic cleanup.
    """
    with McmRuntime(max_qubits=64) as rt:
        yield rt
