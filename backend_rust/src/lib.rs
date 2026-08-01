//! # MCM Runtime — Crate Root
//!
//! This crate provides a memory-safe, asynchronous Mid-Circuit Measurement
//! runtime engine for PennyLane-Catalyst. It compiles to a C-compatible
//! shared library (`cdylib`) that plugs into Catalyst's device interface.
//!
//! ## Module Layout
//!
//! - [`bindings`] — Auto-generated C type definitions from `capi.h` (via `bindgen`).
//! - [`runtime`] — Core `McmRuntimeCore` struct and lifecycle management.
//! - [`qubit`] — `QubitRegistry` for thread-safe qubit allocation and wire tracking.
//! - [`measurement`] — Measurement execution, result storage, and callback dispatch.
//! - [`conditional`] — Conditional evaluation for dynamic circuit control flow.
//! - [`ffi`] — The exported `#[no_mangle] extern "C"` entry points.

#![deny(missing_docs)]

#[allow(missing_docs)]
mod bindings;
mod conditional;
pub mod ffi;
mod measurement;
mod qubit;
mod runtime;
