// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! wasm32 stand-in for [`crate::metrics`].
//!
//! `prometheus` pulls in libc and can't compile to `wasm32-unknown-unknown`.
//! The execution engine reads metric fields by name and invokes
//! `inc`/`observe`/`start_timer`; we replicate that surface with no-op types so
//! the call sites compile unchanged. Constructors take no arguments (instead of
//! a `prometheus::Registry`) — wasm callers use `LimitsMetrics::new_stub()` /
//! `BytecodeVerifierMetrics::new_stub()`.

#[derive(Default, Clone)]
pub struct StubCounter;
impl StubCounter {
    pub fn new(_name: &str, _help: &str) -> Result<Self, ()> {
        Ok(StubCounter)
    }
    pub fn inc(&self) {}
    pub fn inc_by(&self, _v: u64) {}
}
/// Aliased so external call sites that use `prometheus::IntCounter` keep
/// compiling.
pub type StubIntCounter = StubCounter;

pub struct StubCounterVec;
impl StubCounterVec {
    pub fn with_label_values(&self, _labels: &[&str]) -> StubCounter {
        StubCounter
    }
}

pub struct StubTimer;
impl Drop for StubTimer {
    fn drop(&mut self) {}
}
impl StubTimer {
    pub fn observe_duration(self) {}
    pub fn stop_and_record(self) -> f64 {
        0.0
    }
    /// Real prometheus returns `f64` here too; the adapter pipes this straight
    /// into a histogram's `observe`. Returning `f64` keeps both call paths
    /// typing identically on wasm.
    pub fn stop_and_discard(self) -> f64 {
        0.0
    }
}

pub struct StubHistogram;
impl StubHistogram {
    pub fn observe(&self, _v: f64) {}
    pub fn start_timer(&self) -> StubTimer {
        StubTimer
    }
}

pub struct LimitsMetrics {
    pub excessive_estimated_effects_size: StubCounterVec,
    pub excessive_written_objects_size: StubCounterVec,
    pub excessive_new_move_object_ids: StubCounterVec,
    pub excessive_deleted_move_object_ids: StubCounterVec,
    pub excessive_transferred_move_object_ids: StubCounterVec,
    pub excessive_object_runtime_cached_objects: StubCounterVec,
    pub excessive_object_runtime_store_entries: StubCounterVec,
}

impl LimitsMetrics {
    pub fn new_stub() -> Self {
        Self {
            excessive_estimated_effects_size: StubCounterVec,
            excessive_written_objects_size: StubCounterVec,
            excessive_new_move_object_ids: StubCounterVec,
            excessive_deleted_move_object_ids: StubCounterVec,
            excessive_transferred_move_object_ids: StubCounterVec,
            excessive_object_runtime_cached_objects: StubCounterVec,
            excessive_object_runtime_store_entries: StubCounterVec,
        }
    }
}

pub struct BytecodeVerifierMetrics {
    pub verifier_timeout_metrics: StubCounterVec,
    pub verifier_runtime_per_module_success_latency: StubHistogram,
    pub verifier_runtime_per_ptb_success_latency: StubHistogram,
    pub verifier_runtime_per_module_timeout_latency: StubHistogram,
    pub verifier_runtime_per_ptb_timeout_latency: StubHistogram,
}

impl BytecodeVerifierMetrics {
    pub const OVERALL_TAG: &'static str = "overall";
    pub const SUCCESS_TAG: &'static str = "success";
    pub const TIMEOUT_TAG: &'static str = "failed";

    pub fn new_stub() -> Self {
        Self {
            verifier_timeout_metrics: StubCounterVec,
            verifier_runtime_per_module_success_latency: StubHistogram,
            verifier_runtime_per_ptb_success_latency: StubHistogram,
            verifier_runtime_per_module_timeout_latency: StubHistogram,
            verifier_runtime_per_ptb_timeout_latency: StubHistogram,
        }
    }
}
