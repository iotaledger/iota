// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(target_arch = "wasm32"))]
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(not(target_arch = "wasm32"))]
use prometheus_filtered::{
    Histogram, IntCounterVec, IntGauge, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_with_registry,
};

#[cfg(not(target_arch = "wasm32"))]
pub struct LimitsMetrics {
    /// Execution limits metrics
    pub excessive_estimated_effects_size: IntCounterVec,
    pub excessive_written_objects_size: IntCounterVec,
    pub excessive_new_move_object_ids: IntCounterVec,
    pub excessive_deleted_move_object_ids: IntCounterVec,
    pub excessive_transferred_move_object_ids: IntCounterVec,
    pub excessive_object_runtime_cached_objects: IntCounterVec,
    pub excessive_object_runtime_store_entries: IntCounterVec,
    pub excessive_move_call_argument_nodes: IntCounterVec,
    pub move_call_argument_nodes: Histogram,
    /// Package cache and runtime figures of the Move VM. Only the vm2 execution cut reports
    /// them.
    pub vm_telemetry_metrics: MoveVMTelemetryMetrics,
}

#[cfg(not(target_arch = "wasm32"))]
impl LimitsMetrics {
    const MOVE_CALL_ARGUMENT_NODES_BUCKETS: &'static [f64] = &[
        1.0, 4.0, 16.0, 64.0, 256.0, 1024.0, 4096.0, 16384.0, 65536.0, 262144.0, 1048576.0,
        2097152.0,
    ];

    pub fn new(registry: &prometheus_filtered::Registry) -> LimitsMetrics {
        Self {
            excessive_estimated_effects_size: register_int_counter_vec_with_registry!(
                "excessive_estimated_effects_size",
                "Number of transactions with estimated effects size exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_written_objects_size: register_int_counter_vec_with_registry!(
                "excessive_written_objects_size",
                "Number of transactions with written objects size exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_new_move_object_ids: register_int_counter_vec_with_registry!(
                "excessive_new_move_object_ids_size",
                "Number of transactions with new move object ID count exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_deleted_move_object_ids: register_int_counter_vec_with_registry!(
                "excessive_deleted_move_object_ids_size",
                "Number of transactions with deleted move object ID count exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_transferred_move_object_ids: register_int_counter_vec_with_registry!(
                "excessive_transferred_move_object_ids_size",
                "Number of transactions with transferred move object ID count exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_object_runtime_cached_objects: register_int_counter_vec_with_registry!(
                "excessive_object_runtime_cached_objects_size",
                "Number of transactions with object runtime cached object count exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_object_runtime_store_entries: register_int_counter_vec_with_registry!(
                "excessive_object_runtime_store_entries_size",
                "Number of transactions with object runtime store entry count exceeding the limit",
                &["metered", "limit_type"],
                registry,
            )
                .unwrap(),
            excessive_move_call_argument_nodes: register_int_counter_vec_with_registry!(
                "excessive_move_call_argument_nodes",
                "Number of Move calls whose arguments exceed the per-call or per-transaction value limit",
                &["limit_type"],
                registry,
            )
                .unwrap(),
            move_call_argument_nodes: register_histogram_with_registry!(
                "move_call_argument_nodes",
                "Struct, enum and vector values in the arguments of one Move call",
                Self::MOVE_CALL_ARGUMENT_NODES_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            vm_telemetry_metrics: MoveVMTelemetryMetrics::new(registry),
        }
    }
}

/// Gauges for the package cache and runtime figures of the Move VM, set from the runtime's
/// telemetry report at most once per reporting interval.
#[cfg(not(target_arch = "wasm32"))]
pub struct MoveVMTelemetryMetrics {
    pub move_vm_package_cache_count: IntGauge,
    pub move_vm_total_arena_size_bytes: IntGauge,
    pub move_vm_module_count: IntGauge,
    pub move_vm_function_count: IntGauge,
    pub move_vm_type_count: IntGauge,
    pub move_vm_interner_size: IntGauge,
    pub move_vm_vtable_cache_count: IntGauge,
    pub move_vm_vtable_cache_hits: IntGauge,
    pub move_vm_vtable_cache_misses: IntGauge,
    pub move_vm_load_time_ms: IntGauge,
    pub move_vm_load_count: IntGauge,
    pub move_vm_validation_time_ms: IntGauge,
    pub move_vm_validation_count: IntGauge,
    pub move_vm_jit_time_ms: IntGauge,
    pub move_vm_jit_count: IntGauge,
    pub move_vm_execution_time_ms: IntGauge,
    pub move_vm_execution_count: IntGauge,
    pub move_vm_interpreter_time_ms: IntGauge,
    pub move_vm_interpreter_count: IntGauge,
    pub move_vm_max_callstack_size: IntGauge,
    pub move_vm_max_valuestack_size: IntGauge,
    pub move_vm_total_time_ms: IntGauge,
    pub move_vm_total_count: IntGauge,
    last_report_ms: AtomicU64,
}

#[cfg(not(target_arch = "wasm32"))]
impl MoveVMTelemetryMetrics {
    const REPORT_INTERVAL_MS: u64 = 30_000;

    pub fn new(registry: &prometheus_filtered::Registry) -> Self {
        Self {
            move_vm_package_cache_count: register_int_gauge_with_registry!(
                "move_vm_package_cache_count",
                "Number of packages in the Move VM cache",
                registry,
            )
            .unwrap(),
            move_vm_total_arena_size_bytes: register_int_gauge_with_registry!(
                "move_vm_total_arena_size_bytes",
                "Total arena memory of cached Move VM packages in bytes",
                registry,
            )
            .unwrap(),
            move_vm_module_count: register_int_gauge_with_registry!(
                "move_vm_module_count",
                "Total modules across cached Move VM packages",
                registry,
            )
            .unwrap(),
            move_vm_function_count: register_int_gauge_with_registry!(
                "move_vm_function_count",
                "Total functions across cached Move VM packages",
                registry,
            )
            .unwrap(),
            move_vm_type_count: register_int_gauge_with_registry!(
                "move_vm_type_count",
                "Total types across cached Move VM packages",
                registry,
            )
            .unwrap(),
            move_vm_interner_size: register_int_gauge_with_registry!(
                "move_vm_interner_size",
                "Number of entries in the Move VM string interner",
                registry,
            )
            .unwrap(),
            move_vm_vtable_cache_count: register_int_gauge_with_registry!(
                "move_vm_vtable_cache_count",
                "Number of entries in the Move VM VTable cache",
                registry,
            )
            .unwrap(),
            move_vm_vtable_cache_hits: register_int_gauge_with_registry!(
                "move_vm_vtable_cache_hits",
                "Cumulative VTable cache hits in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_vtable_cache_misses: register_int_gauge_with_registry!(
                "move_vm_vtable_cache_misses",
                "Cumulative VTable cache misses in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_load_time_ms: register_int_gauge_with_registry!(
                "move_vm_load_time_ms",
                "Cumulative package load time in the Move VM (ms)",
                registry,
            )
            .unwrap(),
            move_vm_load_count: register_int_gauge_with_registry!(
                "move_vm_load_count",
                "Cumulative number of packages loaded by the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_validation_time_ms: register_int_gauge_with_registry!(
                "move_vm_validation_time_ms",
                "Cumulative validation time in the Move VM (ms)",
                registry,
            )
            .unwrap(),
            move_vm_validation_count: register_int_gauge_with_registry!(
                "move_vm_validation_count",
                "Cumulative number of validations in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_jit_time_ms: register_int_gauge_with_registry!(
                "move_vm_jit_time_ms",
                "Cumulative JIT compilation time in the Move VM (ms)",
                registry,
            )
            .unwrap(),
            move_vm_jit_count: register_int_gauge_with_registry!(
                "move_vm_jit_count",
                "Cumulative number of JIT compilations in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_execution_time_ms: register_int_gauge_with_registry!(
                "move_vm_execution_time_ms",
                "Cumulative execution time in the Move VM (ms)",
                registry,
            )
            .unwrap(),
            move_vm_execution_count: register_int_gauge_with_registry!(
                "move_vm_execution_count",
                "Cumulative number of execution calls in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_interpreter_time_ms: register_int_gauge_with_registry!(
                "move_vm_interpreter_time_ms",
                "Cumulative interpreter time in the Move VM (ms)",
                registry,
            )
            .unwrap(),
            move_vm_interpreter_count: register_int_gauge_with_registry!(
                "move_vm_interpreter_count",
                "Cumulative number of interpreter calls in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_max_callstack_size: register_int_gauge_with_registry!(
                "move_vm_max_callstack_size",
                "Maximum observed callstack depth in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_max_valuestack_size: register_int_gauge_with_registry!(
                "move_vm_max_valuestack_size",
                "Maximum observed value stack size in the Move VM",
                registry,
            )
            .unwrap(),
            move_vm_total_time_ms: register_int_gauge_with_registry!(
                "move_vm_total_time_ms",
                "Cumulative total time spent in the Move VM (ms)",
                registry,
            )
            .unwrap(),
            move_vm_total_count: register_int_gauge_with_registry!(
                "move_vm_total_count",
                "Cumulative total number of Move VM interactions",
                registry,
            )
            .unwrap(),
            last_report_ms: AtomicU64::new(0),
        }
    }

    /// Calls `f` to set the gauges when the reporting interval has elapsed since the last
    /// update, and does nothing otherwise. Building the runtime's telemetry report scans the
    /// whole package cache, so it belongs inside `f`.
    pub fn try_update(&self, f: impl FnOnce(&Self)) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or_default();
        let last = self.last_report_ms.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last) < Self::REPORT_INTERVAL_MS {
            return;
        }
        self.last_report_ms.store(now_ms, Ordering::Relaxed);
        f(self);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct BytecodeVerifierMetrics {
    /// Bytecode verifier metrics timeout counter
    pub verifier_timeout_metrics: IntCounterVec,
    /// Bytecode verifier runtime latency for each module successfully verified
    pub verifier_runtime_per_module_success_latency: Histogram,
    /// Bytecode verifier runtime latency for each programmable transaction
    /// block successfully verified
    pub verifier_runtime_per_ptb_success_latency: Histogram,
    /// Bytecode verifier runtime latency for each module which timed out
    pub verifier_runtime_per_module_timeout_latency: Histogram,
    /// Bytecode verifier runtime latency for each programmable transaction
    /// block which timed out
    pub verifier_runtime_per_ptb_timeout_latency: Histogram,
}

#[cfg(not(target_arch = "wasm32"))]
impl BytecodeVerifierMetrics {
    pub const OVERALL_TAG: &'static str = "overall";
    pub const SUCCESS_TAG: &'static str = "success";
    pub const TIMEOUT_TAG: &'static str = "failed";
    const LATENCY_SEC_BUCKETS: &'static [f64] = &[
        0.000_010, 0.000_025, 0.000_050, 0.000_100, // sub 100 nanos
        0.000_250, 0.000_500, 0.001_000, 0.002_500, 0.005_000, 0.010_000, // sub 10 ms: p99
        0.025_000, 0.050_000, 0.100_000, 0.250_000, 0.500_000, 1.000_000, // sub 1 s
        10.000_000, 20.000_000, 50.000_000, 100.0, // We should almost never get here
    ];
    pub fn new(registry: &prometheus_filtered::Registry) -> Self {
        Self {
            verifier_timeout_metrics: register_int_counter_vec_with_registry!(
                "verifier_timeout_metrics",
                "Number of timeouts in bytecode verifier",
                &["verifier_meter", "status"],
                registry,
            )
            .unwrap(),
            verifier_runtime_per_module_success_latency: register_histogram_with_registry!(
                "verifier_runtime_per_module_success_latency",
                "Time spent running bytecode verifier to completion at `run_metered_move_bytecode_verifier_impl`",
                Self::LATENCY_SEC_BUCKETS.to_vec(),
                registry
            )
            .unwrap(),
            verifier_runtime_per_ptb_success_latency: register_histogram_with_registry!(
                "verifier_runtime_per_ptb_success_latency",
                "Time spent running bytecode verifier to completion over the entire PTB at `transaction_input_checker::check_non_system_packages_to_be_published`",
                Self::LATENCY_SEC_BUCKETS.to_vec(),
                registry
            ).unwrap(),
            verifier_runtime_per_module_timeout_latency:  register_histogram_with_registry!(
                "verifier_runtime_per_module_timeout_latency",
                "Time spent running bytecode verifier to timeout at `run_metered_move_bytecode_verifier_impl`",
                Self::LATENCY_SEC_BUCKETS.to_vec(),
                registry
            )
            .unwrap(),
            verifier_runtime_per_ptb_timeout_latency: register_histogram_with_registry!(
                "verifier_runtime_per_ptb_timeout_latency",
                "Time spent running bytecode verifier to timeout over the entire PTB at `transaction_input_checker::check_non_system_packages_to_be_published`",
                Self::LATENCY_SEC_BUCKETS.to_vec(),
                registry
            ).unwrap(),
        }
    }
}

// `prometheus` isn't available on wasm32; provide no-op stubs with a matching
// API for the execution path. Constructed via `new_stub()` (no registry).
#[cfg(target_arch = "wasm32")]
mod wasm_stubs {
    #[derive(Default, Clone)]
    pub struct StubCounter;
    impl StubCounter {
        pub fn inc(&self) {}
        pub fn inc_by(&self, _v: u64) {}
    }

    #[derive(Default, Clone)]
    pub struct StubCounterVec;
    impl StubCounterVec {
        pub fn with_label_values(&self, _labels: &[&str]) -> StubCounter {
            StubCounter
        }
    }

    pub struct StubTimer;
    impl StubTimer {
        pub fn observe_duration(self) {}
        pub fn stop_and_record(self) -> f64 {
            0.0
        }
        pub fn stop_and_discard(self) -> f64 {
            0.0
        }
    }

    #[derive(Default, Clone)]
    pub struct StubHistogram;
    impl StubHistogram {
        pub fn observe(&self, _v: f64) {}
        pub fn start_timer(&self) -> StubTimer {
            StubTimer
        }
    }

    #[derive(Default, Clone)]
    pub struct StubGauge;
    impl StubGauge {
        pub fn set(&self, _v: i64) {}
    }

    pub struct LimitsMetrics {
        pub excessive_estimated_effects_size: StubCounterVec,
        pub excessive_written_objects_size: StubCounterVec,
        pub excessive_new_move_object_ids: StubCounterVec,
        pub excessive_deleted_move_object_ids: StubCounterVec,
        pub excessive_transferred_move_object_ids: StubCounterVec,
        pub excessive_object_runtime_cached_objects: StubCounterVec,
        pub excessive_object_runtime_store_entries: StubCounterVec,
        pub excessive_move_call_argument_nodes: StubCounterVec,
        pub move_call_argument_nodes: StubHistogram,
        pub vm_telemetry_metrics: MoveVMTelemetryMetrics,
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
                excessive_move_call_argument_nodes: StubCounterVec,
                move_call_argument_nodes: StubHistogram,
                vm_telemetry_metrics: MoveVMTelemetryMetrics::default(),
            }
        }
    }

    #[derive(Default)]
    pub struct MoveVMTelemetryMetrics {
        pub move_vm_package_cache_count: StubGauge,
        pub move_vm_total_arena_size_bytes: StubGauge,
        pub move_vm_module_count: StubGauge,
        pub move_vm_function_count: StubGauge,
        pub move_vm_type_count: StubGauge,
        pub move_vm_interner_size: StubGauge,
        pub move_vm_vtable_cache_count: StubGauge,
        pub move_vm_vtable_cache_hits: StubGauge,
        pub move_vm_vtable_cache_misses: StubGauge,
        pub move_vm_load_time_ms: StubGauge,
        pub move_vm_load_count: StubGauge,
        pub move_vm_validation_time_ms: StubGauge,
        pub move_vm_validation_count: StubGauge,
        pub move_vm_jit_time_ms: StubGauge,
        pub move_vm_jit_count: StubGauge,
        pub move_vm_execution_time_ms: StubGauge,
        pub move_vm_execution_count: StubGauge,
        pub move_vm_interpreter_time_ms: StubGauge,
        pub move_vm_interpreter_count: StubGauge,
        pub move_vm_max_callstack_size: StubGauge,
        pub move_vm_max_valuestack_size: StubGauge,
        pub move_vm_total_time_ms: StubGauge,
        pub move_vm_total_count: StubGauge,
    }

    impl MoveVMTelemetryMetrics {
        /// Never calls `f`: there is nothing to report to on wasm32.
        pub fn try_update(&self, _f: impl FnOnce(&Self)) {}
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
}

#[cfg(target_arch = "wasm32")]
pub use wasm_stubs::*;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn telemetry_updates_at_most_once_per_interval() {
        let metrics = MoveVMTelemetryMetrics::new(&prometheus_filtered::Registry::new());

        let mut calls = 0;
        metrics.try_update(|m| {
            calls += 1;
            m.move_vm_package_cache_count.set(3);
        });
        metrics.try_update(|m| {
            calls += 1;
            m.move_vm_package_cache_count.set(5);
        });

        assert_eq!(calls, 1);
        assert_eq!(metrics.move_vm_package_cache_count.get(), 3);
    }
}
