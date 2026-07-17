// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Drop-in replacement for the `prometheus` crate with optional per-metric
//! filtering.
//!
//! Replace `use prometheus::*` with `use prometheus_filtered::*` to control
//! which metrics are exposed. The active filter combines the node config's
//! directives with the `METRICS_FILTER` environment variable's; where both
//! match the same metric, the env var wins.
//!
//! Filter syntax: comma-separated `pattern=LEVEL` directives, last-match
//! wins, where `LEVEL` is one of `off`, `warn`, `info`, `debug`, `trace`.
//! A bare `LEVEL` token (no `pattern=`) sets the global default. A pattern
//! matches if it is a prefix of the metric name OR is a component/prefix of the
//! calling module path (e.g. `traffic_controller` matches
//! `iota_core::traffic_controller::metrics`).
//!
//! Examples:
//! - `METRICS_FILTER=off,authority=warn`
//! - `METRICS_FILTER=authority=off`
//!
//! The directives act as **exposure**
//! thresholds deciding which metrics [`Registry::gather`] includes in its
//! output (`off` exposes none of the matched metrics). Metrics matched by no
//! directive are exposed unconditionally, so with no filter configured the
//! crate behaves exactly like plain `prometheus`; use a bare `LEVEL` directive
//! to set a stricter global default.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

/// Re-exported under a hidden alias so `$crate::prometheus::xxx!` works
/// inside `#[macro_export]` macros without requiring callers to depend
/// directly on the `prometheus` crate.
#[doc(hidden)]
pub use prometheus;
// Re-export prometheus primitives that require no wrapping.
pub use prometheus::{
    DEFAULT_BUCKETS, Encoder, Error, HistogramOpts, Opts, PROTOBUF_FORMAT, ProtobufEncoder, Result,
    TextEncoder, exponential_buckets, gather, histogram_opts, linear_buckets, opts, proto,
};
use tracing::warn;

// ---------------------------------------------------------------------------
// core sub-module
// ---------------------------------------------------------------------------

/// Mirrors `prometheus::core` and provides `GenericGauge`/`GenericCounter`
/// wrappers compatible with prometheus's own generic types.
///
/// `crate::IntGauge`, `crate::Gauge`, `crate::IntCounter`, and
/// `crate::Counter` are type aliases for concrete instantiations of these
/// types, so `Option<IntGauge>` and `Option<GenericGauge<AtomicI64>>` are
/// the same type.
pub mod core {
    use std::mem::ManuallyDrop;

    pub use prometheus::core::{
        Atomic, AtomicF64, AtomicI64, AtomicU64, Collector, Desc, Describer, Metric,
        MetricVecBuilder, Number,
    };

    macro_rules! impl_generic_metric_traits {
        ($T:ident) => {
            impl<P: Atomic> Clone for $T<P> {
                fn clone(&self) -> Self {
                    Self(self.0.clone())
                }
            }

            impl<P: Atomic> std::fmt::Debug for $T<P> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", stringify!($T))?;
                    if self.0.is_none() {
                        write!(f, "(disabled)")?;
                    }
                    Ok(())
                }
            }

            impl<P: Atomic> prometheus::core::Collector for $T<P> {
                fn desc(&self) -> Vec<&Desc> {
                    self.0
                        .as_ref()
                        .map(|inner| inner.desc())
                        .unwrap_or_default()
                }

                fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
                    self.0
                        .as_ref()
                        .map(|inner| inner.collect())
                        .unwrap_or_default()
                }
            }
        };
    }

    macro_rules! impl_metric_traits {
        ($T:ident) => {
            impl Clone for $T {
                fn clone(&self) -> Self {
                    Self(self.0.clone())
                }
            }

            impl std::fmt::Debug for $T {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", stringify!($T))?;
                    if self.0.is_none() {
                        write!(f, "(disabled)")?;
                    }
                    Ok(())
                }
            }

            impl prometheus::core::Collector for $T {
                fn desc(&self) -> Vec<&Desc> {
                    self.0
                        .as_ref()
                        .map(|inner| inner.desc())
                        .unwrap_or_default()
                }

                fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
                    self.0
                        .as_ref()
                        .map(|inner| inner.collect())
                        .unwrap_or_default()
                }
            }
        };
    }

    macro_rules! impl_generic_metric_vec {
        ($T:ident, $M:ident) => {
            impl<P: Atomic> $T<P> {
                pub fn new_some(inner: prometheus::core::$T<P>) -> Self {
                    Self(Some(inner))
                }

                pub fn new_none() -> Self {
                    Self(None)
                }

                #[inline]
                pub fn with_label_values<V>(&self, vals: &[V]) -> $M<P>
                where
                    V: AsRef<str> + std::fmt::Debug,
                {
                    $M::<P>(self.0.as_ref().map(|inner| inner.with_label_values(vals)))
                }

                #[inline]
                pub fn remove_label_values<V>(&self, vals: &[V]) -> super::Result<()>
                where
                    V: AsRef<str> + std::fmt::Debug,
                {
                    self.0
                        .as_ref()
                        .map(|inner| inner.remove_label_values(vals))
                        .unwrap_or(Ok(()))
                }

                #[inline]
                pub fn get_metric_with<V, S: std::hash::BuildHasher>(
                    &self,
                    labels: &std::collections::HashMap<&str, V, S>,
                ) -> super::Result<$M<P>>
                where
                    V: AsRef<str> + std::fmt::Debug,
                {
                    self.0
                        .as_ref()
                        .map(|inner| inner.get_metric_with(labels).map($M::<P>::new_some))
                        .unwrap_or(Ok($M::<P>::new_none()))
                }

                #[inline]
                pub fn get_metric_with_label_values<V>(&self, vals: &[V]) -> super::Result<$M<P>>
                where
                    V: AsRef<str> + std::fmt::Debug,
                {
                    self.0
                        .as_ref()
                        .map(|inner| {
                            inner
                                .get_metric_with_label_values(vals)
                                .map($M::<P>::new_some)
                        })
                        .unwrap_or(Ok($M::<P>::new_none()))
                }

                #[inline]
                pub fn reset(&self) {
                    if let Some(v) = &self.0 {
                        v.reset();
                    }
                }
            }
        };
    }

    pub struct GenericCounter<P: Atomic>(Option<prometheus::core::GenericCounter<P>>);

    impl<P: Atomic> GenericCounter<P> {
        pub fn new_some(inner: prometheus::core::GenericCounter<P>) -> Self {
            Self(Some(inner))
        }

        pub fn new_none() -> Self {
            Self(None)
        }

        pub fn new(name: &str, help: &str) -> prometheus::Result<Self> {
            prometheus::core::GenericCounter::new(name, help).map(Self::new_some)
        }

        pub fn with_opts(opts: super::Opts) -> super::Result<Self> {
            prometheus::core::GenericCounter::with_opts(opts).map(Self::new_some)
        }

        #[inline]
        pub fn get(&self) -> P::T {
            self.0
                .as_ref()
                .map(|inner| inner.get())
                .unwrap_or(<P::T>::from_i64(0))
        }

        #[inline]
        pub fn inc(&self) {
            if let Some(inner) = &self.0 {
                inner.inc();
            }
        }

        #[inline]
        pub fn inc_by(&self, v: <P as Atomic>::T) {
            if let Some(inner) = &self.0 {
                inner.inc_by(v);
            }
        }

        #[inline]
        pub fn reset(&self) {
            if let Some(inner) = &self.0 {
                inner.reset();
            }
        }
    }

    impl_generic_metric_traits!(GenericCounter);

    pub struct GenericGauge<P: Atomic>(Option<prometheus::core::GenericGauge<P>>);

    impl<P: Atomic> GenericGauge<P> {
        pub fn new_some(inner: prometheus::core::GenericGauge<P>) -> Self {
            Self(Some(inner))
        }

        pub fn new_none() -> Self {
            Self(None)
        }

        pub fn new(name: &str, help: &str) -> super::Result<Self> {
            prometheus::core::GenericGauge::new(name, help).map(Self::new_some)
        }

        pub fn with_opts(opts: super::Opts) -> super::Result<Self> {
            prometheus::core::GenericGauge::with_opts(opts).map(Self::new_some)
        }

        #[inline]
        pub fn get(&self) -> P::T {
            self.0
                .as_ref()
                .map(|inner| inner.get())
                .unwrap_or(<P::T>::from_i64(0))
        }

        #[inline]
        pub fn set(&self, v: P::T) {
            if let Some(inner) = &self.0 {
                inner.set(v);
            }
        }

        #[inline]
        pub fn inc(&self) {
            if let Some(inner) = &self.0 {
                inner.inc();
            }
        }

        #[inline]
        pub fn dec(&self) {
            if let Some(inner) = &self.0 {
                inner.dec();
            }
        }

        #[inline]
        pub fn add(&self, v: P::T) {
            if let Some(inner) = &self.0 {
                inner.add(v);
            }
        }

        #[inline]
        pub fn sub(&self, v: P::T) {
            if let Some(inner) = &self.0 {
                inner.sub(v);
            }
        }
    }

    impl_generic_metric_traits!(GenericGauge);

    pub struct GenericCounterVec<P: Atomic>(Option<prometheus::core::GenericCounterVec<P>>);

    impl_generic_metric_traits!(GenericCounterVec);
    impl_generic_metric_vec!(GenericCounterVec, GenericCounter);

    pub struct GenericGaugeVec<P: Atomic>(Option<prometheus::core::GenericGaugeVec<P>>);

    impl_generic_metric_traits!(GenericGaugeVec);
    impl_generic_metric_vec!(GenericGaugeVec, GenericGauge);

    pub struct Histogram(Option<prometheus::Histogram>);

    impl_metric_traits!(Histogram);

    impl Histogram {
        pub fn new_some(inner: prometheus::Histogram) -> Self {
            Self(Some(inner))
        }

        pub fn new_none() -> Self {
            Self(None)
        }

        pub fn with_opts(opts: prometheus::HistogramOpts) -> prometheus::Result<Self> {
            prometheus::Histogram::with_opts(opts).map(|h| Self(Some(h)))
        }

        #[inline]
        pub fn observe(&self, v: f64) {
            if let Some(h) = &self.0 {
                h.observe(v);
            }
        }

        #[inline]
        pub fn start_timer(&self) -> HistogramTimer {
            HistogramTimer(self.0.as_ref().map(|h| h.start_timer()))
        }

        #[inline]
        pub fn get_sample_count(&self) -> u64 {
            self.0.as_ref().map_or(0, |h| h.get_sample_count())
        }

        #[inline]
        pub fn get_sample_sum(&self) -> f64 {
            self.0.as_ref().map_or(0.0, |h| h.get_sample_sum())
        }
    }

    pub struct HistogramTimer(Option<prometheus::HistogramTimer>);

    impl std::fmt::Debug for HistogramTimer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "HistogramTimer")?;
            if self.0.is_none() {
                write!(f, "(disabled)")?;
            }
            Ok(())
        }
    }

    impl Drop for HistogramTimer {
        fn drop(&mut self) {
            // Dropping the inner prometheus::HistogramTimer records the observation.
            drop(self.0.take());
        }
    }

    impl HistogramTimer {
        /// Records the elapsed time and returns it; prevents the `Drop` impl
        /// from recording a second time.
        #[inline]
        pub fn stop_and_record(self) -> f64 {
            // ManuallyDrop prevents our Drop impl from running, so the inner timer
            // can be consumed by its own stop_and_record without double-recording.
            let mut wrapper = ManuallyDrop::new(self);
            wrapper
                .0
                .take()
                .map(|t| t.stop_and_record())
                .unwrap_or_default()
        }

        /// Records the duration; provided for compatibility with older
        /// prometheus APIs.
        #[inline]
        pub fn observe_duration(self) {
            let _ = self.stop_and_record();
        }

        /// Discards the timer without recording; returns the elapsed seconds.
        #[inline]
        pub fn stop_and_discard(self) -> f64 {
            let mut wrapper = ManuallyDrop::new(self);
            wrapper
                .0
                .take()
                .map(|t| t.stop_and_discard())
                .unwrap_or_default()
        }
    }

    pub struct HistogramVec(Option<prometheus::HistogramVec>);

    impl_metric_traits!(HistogramVec);

    impl HistogramVec {
        pub fn new_some(inner: prometheus::HistogramVec) -> Self {
            Self(Some(inner))
        }

        pub fn new_none() -> Self {
            Self(None)
        }

        #[inline]
        pub fn with_label_values(&self, vals: &[&str]) -> Histogram {
            Histogram(self.0.as_ref().map(|v| v.with_label_values(vals)))
        }

        #[inline]
        pub fn remove_label_values(&self, vals: &[&str]) -> prometheus::Result<()> {
            match &self.0 {
                Some(v) => v.remove_label_values(vals),
                None => Ok(()),
            }
        }
    }
}

pub type Counter = core::GenericCounter<prometheus::core::AtomicF64>;
pub type IntCounter = core::GenericCounter<prometheus::core::AtomicU64>;
pub type Gauge = core::GenericGauge<prometheus::core::AtomicF64>;
pub type IntGauge = core::GenericGauge<prometheus::core::AtomicI64>;

pub type CounterVec = core::GenericCounterVec<prometheus::core::AtomicF64>;
pub type IntCounterVec = core::GenericCounterVec<prometheus::core::AtomicU64>;
pub type GaugeVec = core::GenericGaugeVec<prometheus::core::AtomicF64>;
pub type IntGaugeVec = core::GenericGaugeVec<prometheus::core::AtomicI64>;

pub use core::{Histogram, HistogramTimer, HistogramVec};

// ---------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------

/// Verbosity level for a metric.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricLevel {
    /// As a filter threshold: expose none of the matched metrics. Not
    /// meaningful as a per-metric level — tag metrics `Warn`..`Trace`.
    Off,
    Warn,
    Info,
    // The default for an untagged metric.
    #[default]
    Debug,
    Trace,
}

impl MetricLevel {
    pub(crate) const fn verbosity(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Warn => 1,
            Self::Info => 2,
            Self::Debug => 3,
            Self::Trace => 4,
        }
    }
}

/// Default threshold when no directive matches a metric: expose it. Filtering
/// is opt-in, so an unfiltered registry behaves like plain `prometheus`.
const DEFAULT_THRESHOLD: u8 = MetricLevel::Trace.verbosity();

#[derive(Clone)]
struct FilterDirective {
    /// Empty string means global catch-all.
    pattern: String,
    /// Metrics matched by this directive are exposed iff their verbosity is
    /// `<= threshold`. `off=0`, `warn=1`, `info=2`, `debug=3`, `trace=4`.
    threshold: u8,
}

/// Parses and evaluates `METRICS_FILTER`-style directives.
///
/// Filter string: comma-separated `pattern=LEVEL` directives, last-match
/// wins, a metric is exposed when its own level is at or below the threshold.
#[derive(Default)]
pub struct Filter {
    directives: Vec<FilterDirective>,
}

/// Parses one `pattern=LEVEL` directive. `None` for an empty segment or an
/// invalid level (dropped with a warning).
fn parse_directive(part: &str) -> Option<FilterDirective> {
    let part = part.trim();
    if part.is_empty() {
        return None;
    }
    let (pattern, value) = if let Some(eq) = part.rfind('=') {
        (part[..eq].trim().to_owned(), part[eq + 1..].trim())
    } else {
        (String::new(), part)
    };
    let threshold = match value {
        "off" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        "trace" => 4,
        other => {
            warn!(
                "dropping prometheus filter directive {part:?}: invalid level {other:?}, \
                 expected one of off/warn/info/debug/trace"
            );
            return None;
        }
    };
    Some(FilterDirective { pattern, threshold })
}

/// Evaluates `directives` for a metric, returning the last matching
/// directive's threshold, or [`DEFAULT_THRESHOLD`] when none matches.
///
/// Matching order (last wins):
/// 1. Empty pattern — global default.
/// 2. `name.starts_with(pattern)` — metric name prefix.
/// 3. `module.starts_with(pattern)` — module path prefix.
/// 4. `module` contains `"::{pattern}"` — exact module component.
fn threshold_for(directives: &[FilterDirective], name: &str, module: &str) -> u8 {
    let mut threshold = DEFAULT_THRESHOLD;
    for dir in directives {
        if dir.pattern.is_empty()
            || name.starts_with(dir.pattern.as_str())
            || module.starts_with(dir.pattern.as_str())
            || module.contains(&format!("::{}", dir.pattern))
        {
            threshold = dir.threshold;
        }
    }
    threshold
}

impl Filter {
    /// Parses a directive string, ignoring the `METRICS_FILTER` env var; use
    /// [`Filter::resolve`] to honour it.
    pub fn parse(s: &str) -> Self {
        let directives = s.split(',').filter_map(parse_directive).collect();
        Self { directives }
    }

    /// Returns `true` if a registered metric named `name` in `module` at
    /// verbosity `level` should be exposed when gathering.
    #[inline]
    pub fn is_exposed(&self, name: &str, module: &str, level: MetricLevel) -> bool {
        threshold_for(&self.directives, name, module) >= level.verbosity()
    }

    /// Resolves the metrics filter from `fallback` (the node config)
    /// and the `METRICS_FILTER` env variables. If the same key exists in both,
    /// the env var takes precedence.
    pub fn resolve(fallback: Option<&str>) -> Self {
        let env = std::env::var("METRICS_FILTER").ok();
        match (fallback, env.as_deref()) {
            (Some(f), Some(e)) => Self::parse(&format!("{f},{e}")),
            (Some(f), None) => Self::parse(f),
            (None, Some(e)) => Self::parse(e),
            (None, None) => Self::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Wraps `prometheus::Registry` with an embedded `Filter` so that
/// `register_*_with_registry!` macros can decide at construction time whether
/// a metric should be active.
///
/// Metrics registered through the wrapper macros are recorded with their
/// module path and level, so [`Registry::gather`] can apply the filter's
/// exposure directives to them.
#[derive(Clone)]
pub struct Registry {
    inner: prometheus::Registry,
    filter: Arc<Filter>,
    /// Name prefix passed to [`Registry::new_custom`]; gathered family names
    /// include it.
    prefix: Option<String>,
    /// Gathered family name → (module path, level) for metrics registered via
    /// the wrapper macros; consulted by [`Registry::gather`].
    registered: Arc<RwLock<HashMap<String, (String, MetricLevel)>>>,
}

impl Registry {
    /// Creates a registry whose filter is resolved from the `METRICS_FILTER`
    /// env var (permissive when unset).
    pub fn new() -> Self {
        Self {
            inner: prometheus::Registry::new(),
            filter: Arc::new(Filter::resolve(None)),
            prefix: None,
            registered: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a custom-prefixed registry.
    pub fn new_custom(
        prefix: Option<String>,
        labels: Option<std::collections::HashMap<String, String>>,
        filter: Option<Arc<Filter>>,
    ) -> prometheus::Result<Self> {
        Ok(Self {
            inner: prometheus::Registry::new_custom(prefix.clone(), labels)?,
            filter: filter.unwrap_or_else(|| Arc::new(Filter::resolve(None))),
            prefix,
            registered: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Returns the registry's filter, so related registries can be built to
    /// share it via [`Registry::new_custom`].
    #[inline]
    pub fn filter(&self) -> Arc<Filter> {
        self.filter.clone()
    }

    /// Used by the wrapper macros: records a registering metric's module path
    /// and level, so [`Registry::gather`] can apply the filter's exposure
    /// directives to it.
    #[inline]
    pub fn record(&self, name: &str, module: &str, level: MetricLevel) {
        let exposed_name = match &self.prefix {
            Some(prefix) => format!("{prefix}_{name}"),
            None => name.to_owned(),
        };
        self.registered
            .write()
            .unwrap()
            .insert(exposed_name, (module.to_owned(), level));
    }

    /// Returns the underlying `prometheus::Registry` for use inside wrapper
    /// macros.
    #[inline]
    pub fn inner(&self) -> &prometheus::Registry {
        &self.inner
    }

    pub fn register(&self, c: Box<dyn prometheus::core::Collector>) -> prometheus::Result<()> {
        self.inner.register(c)
    }

    pub fn unregister(&self, c: Box<dyn prometheus::core::Collector>) -> prometheus::Result<()> {
        self.inner.unregister(c)
    }

    /// Gathers the registry's metric families, dropping those disabled by the
    /// filter's exposure directives. Families not registered through the
    /// wrapper macros (e.g. direct collectors) always pass through.
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        let registered = self.registered.read().unwrap();
        self.inner
            .gather()
            .into_iter()
            .filter(|family| {
                registered.get(family.name()).is_none_or(|(module, level)| {
                    self.filter.is_exposed(family.name(), module, *level)
                })
            })
            .collect()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry").finish_non_exhaustive()
    }
}

/// Returns the process-wide `Filter` of the [`default_registry`], resolved
/// once from `METRICS_FILTER` (permissive when unset).
fn default_filter() -> &'static Arc<Filter> {
    static INSTANCE: OnceLock<Arc<Filter>> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Filter::resolve(None)))
}

/// Returns a reference to the global default `Registry`, wrapping the
/// underlying `prometheus::default_registry()`. Metrics registered here
/// appear in the standard prometheus default gather output.
pub fn default_registry() -> &'static Registry {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<Registry> = OnceLock::new();
    INSTANCE.get_or_init(|| Registry {
        inner: prometheus::default_registry().clone(),
        filter: default_filter().clone(),
        prefix: None,
        registered: Arc::new(RwLock::new(HashMap::new())),
    })
}

// ---------------------------------------------------------------------------
// Wrapper macros
// ---------------------------------------------------------------------------
//
// Each macro captures `module_path!()` at the call site so the filter can
// match by subsystem in addition to metric name.
//
// The `$registry` must be a `prometheus_filtered::Registry`. On success the
// macro always returns `Ok(WrappedType(Some(...)))` or `Ok(WrappedType(None))`
// — never `Err` from the filtering logic itself.
//
// `$crate::prometheus::` is used for inner prometheus macro calls so that
// callers don't need a direct `prometheus` crate dependency.
//
// `let _n = $name; let name: &str = &*_n;` handles both `&str` literals and
// `format!(...)` String expressions uniformly.

/// register_int_counter_with_registry!(name, help, registry)
#[macro_export]
macro_rules! register_int_counter_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {
        $crate::register_int_counter_with_registry!(
            $name, $help, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_int_counter_with_registry!(
            name,
            $help,
            ($registry).inner()
        )
        .map($crate::core::GenericCounter::new_some)
    }};
}

/// register_int_counter_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_int_counter_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {
        $crate::register_int_counter_vec_with_registry!(
            $name, $help, $labels, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $labels:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_int_counter_vec_with_registry!(
            name,
            $help,
            $labels,
            ($registry).inner()
        )
        .map($crate::IntCounterVec::new_some)
    }};
}

/// register_int_gauge_with_registry!(name, help, registry)
#[macro_export]
macro_rules! register_int_gauge_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {
        $crate::register_int_gauge_with_registry!(
            $name, $help, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_int_gauge_with_registry!(name, $help, ($registry).inner())
            .map($crate::core::GenericGauge::new_some)
    }};
}

/// register_int_gauge_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_int_gauge_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {
        $crate::register_int_gauge_vec_with_registry!(
            $name, $help, $labels, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $labels:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_int_gauge_vec_with_registry!(
            name,
            $help,
            $labels,
            ($registry).inner()
        )
        .map($crate::IntGaugeVec::new_some)
    }};
}

/// register_histogram_with_registry!(name, help, registry)
/// register_histogram_with_registry!(name, help, buckets, registry)
#[macro_export]
macro_rules! register_histogram_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {
        $crate::register_histogram_with_registry!($name, $help, $registry; $crate::MetricLevel::Debug)
    };
    ($name:expr, $help:expr, $buckets:expr, $registry:expr $(,)?) => {
        $crate::register_histogram_with_registry!(
            $name, $help, $buckets, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_histogram_with_registry!(name, $help, ($registry).inner())
            .map($crate::Histogram::new_some)
    }};
    ($name:expr, $help:expr, $buckets:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_histogram_with_registry!(
            name,
            $help,
            $buckets,
            ($registry).inner()
        )
        .map($crate::Histogram::new_some)
    }};
}

/// register_histogram_vec_with_registry!(name, help, labels, registry)
/// register_histogram_vec_with_registry!(name, help, labels, buckets, registry)
#[macro_export]
macro_rules! register_histogram_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {
        $crate::register_histogram_vec_with_registry!(
            $name, $help, $labels, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $labels:expr, $buckets:expr, $registry:expr $(,)?) => {
        $crate::register_histogram_vec_with_registry!(
            $name, $help, $labels, $buckets, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $labels:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_histogram_vec_with_registry!(
            name,
            $help,
            $labels,
            ($registry).inner()
        )
        .map($crate::HistogramVec::new_some)
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_histogram_vec_with_registry!(
            name,
            $help,
            $labels,
            $buckets,
            ($registry).inner()
        )
        .map($crate::HistogramVec::new_some)
    }};
}

/// register_gauge_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_gauge_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {
        $crate::register_gauge_vec_with_registry!(
            $name, $help, $labels, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $labels:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_gauge_vec_with_registry!(
            name,
            $help,
            $labels,
            ($registry).inner()
        )
        .map($crate::core::GenericGaugeVec::new_some)
    }};
}

/// register_gauge_with_registry!(name, help, registry)
#[macro_export]
macro_rules! register_gauge_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {
        $crate::register_gauge_with_registry!($name, $help, $registry; $crate::MetricLevel::Debug)
    };
    ($name:expr, $help:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $level);
        $crate::prometheus::register_gauge_with_registry!(name, $help, ($registry).inner())
            .map($crate::core::GenericGauge::new_some)
    }};
}

/// register_counter!(name, help) - global prometheus registry, filtered.
#[macro_export]
macro_rules! register_counter {
    ($name:expr, $help:expr $(,)?) => {
        $crate::register_counter!($name, $help; $crate::MetricLevel::Debug)
    };
    ($name:expr, $help:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::default_registry().record(name, module, $level);
        $crate::prometheus::register_counter!(name, $help)
            .map($crate::core::GenericCounter::new_some)
    }};
}

/// register_counter_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_counter_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry).record(name, module, $crate::MetricLevel::Debug);
        $crate::prometheus::register_counter_vec_with_registry!(
            name,
            $help,
            $labels,
            ($registry).inner()
        )
        .map($crate::core::GenericCounterVec::new_some)
    }};
}

/// register_counter_vec!(name, help, labels) - global registry, filtered.
#[macro_export]
macro_rules! register_counter_vec {
    ($name:expr, $help:expr, $labels:expr $(,)?) => {
        $crate::register_counter_vec!($name, $help, $labels; $crate::MetricLevel::Debug)
    };
    ($name:expr, $help:expr, $labels:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::default_registry().record(name, module, $level);
        $crate::prometheus::register_counter_vec!(name, $help, $labels)
            .map($crate::core::GenericCounterVec::new_some)
    }};
}

/// register_histogram_vec!(opts, labels) or (name, help, labels) or (name,
/// help, labels, buckets) — global prometheus registry, filtered.
#[macro_export]
macro_rules! register_histogram_vec {
    ($opts:expr, $labels:expr $(,)?) => {
        $crate::register_histogram_vec!($opts, $labels; $crate::MetricLevel::Debug)
    };
    ($name:expr, $help:expr, $labels:expr $(,)?) => {
        $crate::register_histogram_vec!($name, $help, $labels; $crate::MetricLevel::Debug)
    };
    ($name:expr, $help:expr, $labels:expr, $buckets:expr $(,)?) => {
        $crate::register_histogram_vec!(
            $name, $help, $labels, $buckets; $crate::MetricLevel::Debug
        )
    };
    ($opts:expr, $labels:expr ; $level:expr $(,)?) => {{
        let opts = $opts;
        let name: &str = &opts.common_opts.name;
        let module: &str = module_path!();
        $crate::default_registry().record(name, module, $level);
        $crate::prometheus::register_histogram_vec!(opts, $labels)
            .map($crate::HistogramVec::new_some)
    }};
    ($name:expr, $help:expr, $labels:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::default_registry().record(name, module, $level);
        $crate::prometheus::register_histogram_vec!(name, $help, $labels)
            .map($crate::HistogramVec::new_some)
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::default_registry().record(name, module, $level);
        $crate::prometheus::register_histogram_vec!(name, $help, $labels, $buckets)
            .map($crate::HistogramVec::new_some)
    }};
}

#[cfg(test)]
mod tests {
    use super::MetricLevel::Debug;

    #[test]
    fn filter_matches_metric_or_module_name_prefix() {
        // An `off` directive hides exactly the metrics its pattern matches;
        // unmatched metrics stay exposed (the permissive default).
        let filter = super::Filter::parse("authority=off");
        assert!(filter.is_exposed("some_authority", "iota_core::checkpoints", Debug));
        assert!(!filter.is_exposed("authority", "iota_core::checkpoints", Debug));
        assert!(!filter.is_exposed("authority_aggregator", "iota_core::checkpoints", Debug));
        assert!(filter.is_exposed("certs_total", "iota_core::some_authority", Debug));
        assert!(!filter.is_exposed("certs_total", "iota_core::authority", Debug));
        assert!(!filter.is_exposed("certs_total", "iota_core::authority_aggregator", Debug));

        // the last matching prefix shadows the previous ones
        let filter = super::Filter::parse("authority=off,authority_aggregator=trace");
        assert!(!filter.is_exposed("authority", "iota_core::checkpoints", Debug));
        assert!(filter.is_exposed("authority_aggregator", "iota_core::checkpoints", Debug));
        assert!(!filter.is_exposed("certs_total", "iota_core::authority", Debug));
        assert!(filter.is_exposed("certs_total", "iota_core::authority_aggregator", Debug));

        // filter can be set off by default
        let filter = super::Filter::parse("off,authority_aggregator=trace");
        assert!(!filter.is_exposed("some_authority", "iota_core::checkpoints", Debug));
        assert!(!filter.is_exposed("authority", "iota_core::checkpoints", Debug));
        assert!(filter.is_exposed("authority_aggregator", "iota_core::checkpoints", Debug));
        assert!(!filter.is_exposed("certs_total", "iota_core::some_authority", Debug));
        assert!(!filter.is_exposed("certs_total", "iota_core::authority", Debug));
        assert!(filter.is_exposed("certs_total", "iota_core::authority_aggregator", Debug));

        // the full prefix must be matched
        let filter = super::Filter::parse("authority_aggregator=off");
        assert!(filter.is_exposed("authority", "iota_core::checkpoints", Debug));
        assert!(!filter.is_exposed("authority_aggregator", "iota_core::checkpoints", Debug));
        assert!(filter.is_exposed("certs_total", "iota_core::authority", Debug));
        assert!(!filter.is_exposed("certs_total", "iota_core::authority_aggregator", Debug));
    }

    #[test]
    fn unmatched_metrics_are_exposed() {
        use super::MetricLevel::{Info, Trace, Warn};
        // Filtering is opt-in: with no matching directive every metric is
        // exposed, matching plain `prometheus` behaviour.
        for filter in [
            super::Filter::parse(""),
            super::Filter::default(),
            // empty segments are ignored rather than treated as directives.
            super::Filter::parse(",,"),
        ] {
            assert!(filter.is_exposed("anything", "any::module", Warn));
            assert!(filter.is_exposed("anything", "any::module", Info));
            assert!(filter.is_exposed("anything", "any::module", Debug));
            assert!(filter.is_exposed("anything", "any::module", Trace));
        }
    }

    #[test]
    fn rejects_boolean_and_numeric_aliases() {
        use super::MetricLevel::Trace;
        // Only the RUST_LOG-style level names are accepted; the former
        // `on`/`true`/`1` and `false`/`0` aliases are now invalid, so they are
        // dropped and the directive falls back to the permissive default.
        for alias in ["on", "true", "1", "false", "0"] {
            let filter = super::Filter::parse(&format!("authority={alias}"));
            assert!(
                filter.is_exposed("authority", "m", Trace),
                "{alias} should be dropped as invalid, leaving the default"
            );
        }
        // `off` still disables.
        assert!(
            !super::Filter::parse("authority=off").is_exposed("authority", "m", Debug),
            "off should disable"
        );
    }

    #[test]
    fn invalid_directives_are_dropped() {
        use super::MetricLevel::Trace;
        // an unrecognised value leaves the directive out, falling back to the
        // permissive default.
        assert!(super::Filter::parse("authority=maybe").is_exposed("authority", "m", Trace));
        // a bare token without `=LEVEL` is parsed as a global value and, being
        // invalid, dropped — it does NOT enable/disable the `authority` subsystem.
        assert!(super::Filter::parse("authority").is_exposed("authority", "m", Trace));
        // a valid directive alongside an invalid one still takes effect.
        let filter = super::Filter::parse("authority=off,bogus=nope");
        assert!(!filter.is_exposed("authority", "m", Debug));
    }

    #[test]
    fn matches_module_path_prefix() {
        // a pattern that is a prefix of the full module path (not only a `::`
        // component) matches.
        let filter = super::Filter::parse("iota_core=off");
        assert!(!filter.is_exposed("certs_total", "iota_core::authority", Debug));
        assert!(filter.is_exposed("certs_total", "starfish::core", Debug));
    }

    #[test]
    fn global_trace_default() {
        // last-match-wins applies to bare global directives too.
        assert!(super::Filter::parse("off,trace").is_exposed("authority", "m", Debug));
        // an explicit permissive default with a targeted `off` override.
        let filter = super::Filter::parse("trace,authority=off");
        assert!(filter.is_exposed("certs_total", "m", Debug));
        assert!(!filter.is_exposed("authority", "m", Debug));
    }

    #[test]
    fn whitespace_is_trimmed() {
        let filter = super::Filter::parse("  authority = off ,  authority_aggregator = trace  ");
        assert!(!filter.is_exposed("authority", "m", Debug));
        assert!(filter.is_exposed("authority_aggregator", "m", Debug));
    }

    #[test]
    fn resolve_applies_fallback() {
        use super::{Arc, Filter, MetricLevel, Registry};

        // The env var's directives are merged after the fallback's, so the
        // assertions below only hold when it is unset.
        if std::env::var_os("METRICS_FILTER").is_some() {
            return;
        }

        // No env, no fallback -> everything is exposed.
        assert!(Filter::resolve(None).is_exposed("anything", "m", MetricLevel::Trace));
        assert!(Filter::resolve(None).is_exposed("anything", "m", Debug));

        // No env -> the fallback directives apply.
        let filter = Arc::new(Filter::resolve(Some("off,authority=trace")));
        assert!(filter.is_exposed("authority", "m", MetricLevel::Debug));
        assert!(!filter.is_exposed("consensus", "m", MetricLevel::Debug));

        // Registries built to share the filter see the same decisions.
        let registry = Registry::new_custom(None, None, Some(filter.clone())).unwrap();
        let shared = Registry::new_custom(None, None, Some(filter)).unwrap();
        assert!(std::sync::Arc::ptr_eq(&registry.filter(), &shared.filter()));
    }

    #[test]
    fn level_thresholds() {
        use super::MetricLevel::{Debug, Info, Trace, Warn};
        // `warn` threshold exposes only warn metrics.
        let f = super::Filter::parse("authority=warn");
        assert!(f.is_exposed("x", "iota_core::authority", Warn));
        assert!(!f.is_exposed("x", "iota_core::authority", Info));
        assert!(!f.is_exposed("x", "iota_core::authority", Debug));
        // `info` threshold exposes warn+info, hides debug.
        let f = super::Filter::parse("authority=info");
        assert!(f.is_exposed("x", "iota_core::authority", Warn));
        assert!(f.is_exposed("x", "iota_core::authority", Info));
        assert!(!f.is_exposed("x", "iota_core::authority", Debug));
        // `debug` exposes everything untagged and below, but not trace.
        let f = super::Filter::parse("authority=debug");
        assert!(f.is_exposed("x", "iota_core::authority", Debug));
        assert!(!f.is_exposed("x", "iota_core::authority", Trace));
        // `trace` exposes everything.
        let f = super::Filter::parse("authority=trace");
        assert!(f.is_exposed("x", "iota_core::authority", Trace));
        // `off` exposes nothing.
        assert!(!super::Filter::parse("authority=off").is_exposed(
            "x",
            "iota_core::authority",
            Warn
        ));
        // No directive -> exposed at every level.
        assert!(super::Filter::parse("").is_exposed("x", "m", Info));
        assert!(super::Filter::parse("").is_exposed("x", "m", Trace));
    }
}

#[cfg(test)]
mod gather_filter_tests {
    use super::{Filter, MetricLevel, Registry};

    fn registry(filter: &str) -> Registry {
        Registry::new_custom(None, None, Some(std::sync::Arc::new(Filter::parse(filter)))).unwrap()
    }

    fn gathered_names(registry: &Registry) -> Vec<String> {
        let mut names: Vec<_> = registry
            .gather()
            .iter()
            .map(|f| f.name().to_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn gather_applies_level_thresholds_by_module() {
        // Metrics register in this module (`prometheus_filtered::gather_filter_tests`).
        let reg = registry("gather_filter_tests=warn");
        crate::register_int_gauge_with_registry!("g_warn", "h", &reg; MetricLevel::Warn).unwrap();
        let g_debug = crate::register_int_gauge_with_registry!("g_debug", "h", &reg).unwrap();
        g_debug.set(7);

        // Only the warn-tagged metric is exposed; the debug one is registered
        // and keeps collecting.
        assert_eq!(gathered_names(&reg), ["g_warn"]);
        assert_eq!(g_debug.get(), 7);
    }

    #[test]
    fn off_directive_hides_but_still_registers() {
        let reg = registry("g_hidden=off");
        let g = crate::register_int_gauge_with_registry!("g_hidden", "h", &reg).unwrap();
        // Registered (a disabled wrapper would print "(disabled)") and
        // collecting, but absent from gather output.
        assert_eq!(format!("{g:?}"), "GenericGauge");
        g.set(9);
        assert_eq!(g.get(), 9);
        assert_eq!(gathered_names(&reg), Vec::<String>::new());
    }

    #[test]
    fn prefixed_registry_records_exposed_family_names() {
        let exposed = Registry::new_custom(
            Some("consensus".to_owned()),
            None,
            Some(std::sync::Arc::new(Filter::parse(""))),
        )
        .unwrap();
        crate::register_int_gauge_with_registry!("g", "h", &exposed; MetricLevel::Warn).unwrap();
        assert_eq!(gathered_names(&exposed), ["consensus_g"]);

        // The filter keys on the module path, so the prefixed family is
        // matched and hidden even though its gathered name differs.
        let hidden = Registry::new_custom(
            Some("consensus".to_owned()),
            None,
            Some(std::sync::Arc::new(Filter::parse(
                "gather_filter_tests=off",
            ))),
        )
        .unwrap();
        crate::register_int_gauge_with_registry!("g", "h", &hidden; MetricLevel::Warn).unwrap();
        assert_eq!(gathered_names(&hidden), Vec::<String>::new());
    }

    #[test]
    fn directly_registered_collectors_bypass_filter() {
        let reg = registry("off");
        crate::register_int_gauge_with_registry!("g_macro", "h", &reg).unwrap();
        let gauge = prometheus::IntGauge::new("g_direct", "h").unwrap();
        reg.register(Box::new(gauge)).unwrap();

        // Not registered through the macros -> no module/level recorded ->
        // the exposure filter does not apply.
        assert_eq!(gathered_names(&reg), ["g_direct"]);
    }
}

#[cfg(test)]
mod level_macro_tests {
    use super::{IntGauge, MetricLevel, Registry};

    fn registry(filter: &str) -> Registry {
        Registry::new_custom(
            None,
            None,
            Some(std::sync::Arc::new(super::Filter::parse(filter))),
        )
        .unwrap()
    }

    #[test]
    fn hidden_metrics_still_register_and_collect() {
        // At a `warn` threshold, a default (`debug`) metric still registers
        // and collects — it is only hidden from `gather` output.
        let reg = registry("g_default=warn");
        let g: IntGauge = crate::register_int_gauge_with_registry!("g_default", "h", &reg).unwrap();
        // `IntGauge` is a type alias for `core::GenericGauge<AtomicI64>`; its
        // `Debug` impl prints the underlying `GenericGauge` name, not the alias.
        assert_eq!(format!("{g:?}"), "GenericGauge");
        g.set(42);
        assert_eq!(g.get(), 42);
        assert!(reg.gather().is_empty());

        // Even an `off` threshold registers the metric; it only hides it.
        let reg = registry("g_off=off");
        let g: IntGauge = crate::register_int_gauge_with_registry!(
            "g_off", "h", &reg; MetricLevel::Warn
        )
        .unwrap();
        assert_eq!(format!("{g:?}"), "GenericGauge");
        assert!(reg.gather().is_empty());
    }
}
