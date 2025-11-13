use std::sync::{Arc, Mutex};

use iota_metrics::histogram::{Histogram, HistogramVec};
use once_cell::sync::OnceCell;
use prometheus::{IntCounterVec, IntGauge, IntGaugeVec, Registry, register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry, register_int_gauge_with_registry};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, get_current_pid};

/// Percentiles to export for latency histograms (in 1/1000ths).
const PCTS: &[usize] = &[500, 900, 990];

/// Prometheus metrics for gas price prediction pipeline.
#[derive(Clone)]
pub struct GasMetrics {
    /// Generic latency histogram in milliseconds. Label: component.
    pub latency_ms: HistogramVec,
    /// Histogram for per-checkpoint touched objects count. Label: component.
    pub touched_per_cp: HistogramVec,

    /// Queue length gauges. Label: queue ("update" | "train").
    pub queue_len: IntGaugeVec,
    /// Pending training items gauge.
    pub pending_train_items: IntGauge,

    /// Batches accounting. Labels: type ("update"|"train"), status ("received"|"dropped").
    pub batches_total: IntCounterVec,
    /// Inference accounting. Label: status ("success"|"fallback"|"none").
    pub infer_total: IntCounterVec,

    /// Per-operation process CPU usage (percent) as histogram; label: component.
    pub proc_cpu_pct: HistogramVec,
    /// Per-operation process RSS memory (bytes) as histogram; label: component.
    pub proc_mem_bytes: HistogramVec,

    sampler: Arc<GasHwSampler>,
}

impl GasMetrics {
    pub fn new(registry: &Registry) -> Self {
        let latency_ms = HistogramVec::new_in_registry_with_percentiles(
            "gas_predict_latency_ms",
            "Latency for gas prediction components (ms)",
            &["component"],
            registry,
            PCTS.to_vec(),
        );
        let touched_per_cp = HistogramVec::new_in_registry_with_percentiles(
            "congestion_touched_objects_per_cp",
            "Number of touched objects per checkpoint (distribution)",
            &["component"],
            registry,
            PCTS.to_vec(),
        );

        let queue_len = register_int_gauge_vec_with_registry!(
            "gas_predict_queue_len",
            "Queue lengths for gas predictor workers",
            &["queue"],
            registry,
        )
        .unwrap();

        let pending_train_items = register_int_gauge_with_registry!(
            "gas_predict_pending_train_items",
            "Pending training items count",
            registry,
        ).unwrap();

        let batches_total = register_int_counter_vec_with_registry!(
            "gas_predict_batches_total",
            "Batches observed by gas predictor",
            &["type", "status"],
            registry,
        )
        .unwrap();

        let infer_total = register_int_counter_vec_with_registry!(
            "gas_predict_infer_total",
            "Inference attempts and fallbacks",
            &["status"],
            registry,
        )
        .unwrap();

        let proc_cpu_pct = HistogramVec::new_in_registry_with_percentiles(
            "gas_predict_proc_cpu_pct",
            "Process CPU usage percent during gas NN operation",
            &["component"],
            registry,
            PCTS.to_vec(),
        );
        let proc_mem_bytes = HistogramVec::new_in_registry_with_percentiles(
            "gas_predict_proc_mem_bytes",
            "Process memory RSS bytes during gas NN operation",
            &["component"],
            registry,
            PCTS.to_vec(),
        );

        let sampler = Arc::new(GasHwSampler::new());

        Self {
            latency_ms,
            touched_per_cp,
            queue_len,
            pending_train_items,
            batches_total,
            infer_total,
            proc_cpu_pct,
            proc_mem_bytes,
            sampler,
        }
    }

    /// Pre-create time series for common components so that dashboards show lines even
    /// when NN is disabled or idle. This sends a single zero sample per series.
    pub fn warmup_series(&self) {
        let components = [
            "model_updater.predict",
            "model_updater.train_step",
            "model_updater.update_batch",
            "model_updater.build_snapshot",
            "congestion.inform_model",
            "congestion.process_checkpoint_effects",
            "congestion.process_cp_data",
            "congestion.compute_cp_info",
            "congestion.update_cache",
        ];
        for c in components.iter() {
            self.latency_component(c).observe(0);
            self.proc_cpu_pct.with_label_values(&[c]).observe(0);
            self.proc_mem_bytes.with_label_values(&[c]).observe(0);
        }
        // also touched histogram
        self.touched_hist().observe(0);
    }

    pub fn latency_component(&self, name: &str) -> Histogram {
        self.latency_ms.with_label_values(&[name])
    }

    pub fn touched_hist(&self) -> Histogram {
        self.touched_per_cp.with_label_values(&["congestion.touched"]) 
    }

    /// Sample process CPU% and RSS memory and record to histograms for the given component.
    pub fn record_hw_sample(&self, component: &str) {
        if let Some((cpu_pct, rss_bytes)) = self.sampler.sample_proc() {
            self.proc_cpu_pct
                .with_label_values(&[component])
                .observe(cpu_pct as u64);
            self.proc_mem_bytes
                .with_label_values(&[component])
                .observe(rss_bytes as u64);
        }
    }
}

static GLOBAL: OnceCell<Arc<GasMetrics>> = OnceCell::new();

pub fn init_gas_metrics(registry: &Registry) -> Arc<GasMetrics> {
    let m = GLOBAL.get_or_init(|| Arc::new(GasMetrics::new(registry))).clone();
    // warmup once per process
    m.warmup_series();
    m
}

pub fn get_gas_metrics() -> Option<Arc<GasMetrics>> {
    GLOBAL.get().cloned()
}

struct GasHwSampler {
    sys: Mutex<System>,
    pid: sysinfo::Pid,
    refresh_kind: ProcessRefreshKind,
}

impl GasHwSampler {
    fn new() -> Self {
        let mut sys = System::new();
        let pid = get_current_pid().unwrap_or_else(|_| sysinfo::Pid::from_u32(0));
        // Initialize baseline data for CPU% calculation.
        let _ = sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
        let refresh_kind = ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
            .with_exe(UpdateKind::OnlyIfNotSet);
        Self {
            sys: Mutex::new(sys),
            pid,
            refresh_kind,
        }
    }

    fn sample_proc(&self) -> Option<(f32, u64)> {
        let mut sys = self.sys.lock().ok()?;
        let _ = sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[self.pid]), false, self.refresh_kind);
        let p = sys.process(self.pid)?;
        let cpu = p.cpu_usage();
        let rss = p.memory();
        Some((cpu, rss))
    }
}
