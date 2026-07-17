// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Drop-in replacement for the `prometheus` crate with optional per-metric
//! filtering.
//!
//! Replace `use prometheus::*` with `use prometheus_filtered::*` to control
//! which metrics are exposed. The active filter is built from up to three
//! directive layers: the node config's, the `METRICS_FILTER` environment
//! variable's, and an optional runtime override set.
//! A metric is decided by the highest-precedence layer (runtime > env >
//! config); a layer's directives leave the metrics they do not match to the
//! layers below.
//!
//! Filter syntax: comma-separated `pattern=LEVEL` directives, where `LEVEL`
//! is one of `off`, `warn`, `info`, `debug`, `trace`. A bare `LEVEL` token
//! (no `pattern=`) matches every metric — as an env or runtime layer it is
//! a full override of the layers below. A pattern matches if it is a
//! prefix of the metric name OR is a component/prefix of the calling module
//! path (e.g. `traffic_controller` matches
//! `iota_core::traffic_controller::metrics`). When several directives of one
//! layer match the same metric, the most specific one (longest pattern) wins,
//! regardless of order; among directives with the same pattern, the last one
//! wins.
//!
//! Examples:
//! - `METRICS_FILTER=off,authority=warn`
//! - `METRICS_FILTER=authority=off`
//!
//! The directives act as **exposure**
//! thresholds deciding which metrics [`Registry::gather`] includes in its
//! output (`off` exposes none of the matched metrics). Metrics matched by no
//! layer are exposed unconditionally, so with no filter configured the
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

/// Filter holds the directives in three layers — the node config's, the
/// `METRICS_FILTER` env var's, and an optional runtime override.
#[derive(Default)]
pub struct Filter {
    /// The node config's directives, immutable after construction.
    config: Layer,
    /// The `METRICS_FILTER` env var's directives, immutable after
    /// construction.
    env: Layer,
    /// Runtime override layer; consulted first while set.
    runtime: RwLock<Option<Arc<Layer>>>,
}

#[derive(Default)]
struct Layer {
    directives: Vec<FilterDirective>,
    raw: String,
}

impl Layer {
    fn parse(s: &str) -> Self {
        let directives: Vec<FilterDirective> = directive_parts(s)
            .filter_map(|part| {
                parse_directive(part)
                    .map_err(|err| warn!("dropping prometheus filter directive: {err}"))
                    .ok()
            })
            .collect();
        Self {
            raw: render_directives(&directives),
            directives,
        }
    }
}

/// Splits a `METRICS_FILTER`-style string into its non-empty, trimmed
/// directive segments.
pub fn directive_parts(s: &str) -> impl Iterator<Item = &str> + '_ {
    s.split(',').map(str::trim).filter(|part| !part.is_empty())
}

/// Splits one directive into its `(pattern, level)` parts; a directive
/// without `=` is a bare level with an empty (global catch-all) pattern.
pub fn split_directive(part: &str) -> (&str, &str) {
    match part.rfind('=') {
        Some(eq) => (part[..eq].trim(), part[eq + 1..].trim()),
        None => ("", part.trim()),
    }
}

/// Returns an error describing the offending directive if `part` is not a
/// valid `pattern=LEVEL` directive.
pub fn validate_directive(part: &str) -> std::result::Result<(), String> {
    parse_directive(part).map(|_| ())
}

/// Parses one `pattern=LEVEL` directive.
fn parse_directive(part: &str) -> std::result::Result<FilterDirective, String> {
    let (pattern, value) = split_directive(part);
    let threshold = match value {
        "off" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        "trace" => 4,
        other => {
            return Err(format!(
                "invalid level {other:?} in directive {part:?}: expected one of \
                 off/warn/info/debug/trace"
            ));
        }
    };
    Ok(FilterDirective {
        pattern: pattern.to_owned(),
        threshold,
    })
}

/// Renders directives back into their canonical `pattern=LEVEL` string, the
/// inverse of [`parse_directive`].
fn render_directives(directives: &[FilterDirective]) -> String {
    fn token(threshold: u8) -> &'static str {
        match threshold {
            0 => "off",
            1 => "warn",
            2 => "info",
            3 => "debug",
            _ => "trace",
        }
    }
    directives
        .iter()
        .map(|dir| {
            if dir.pattern.is_empty() {
                token(dir.threshold).to_owned()
            } else {
                format!("{}={}", dir.pattern, token(dir.threshold))
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Evaluates `directives` for a metric, returning the most specific matching
/// directive's threshold, or `None` when no directive matches.
///
/// A directive matches when its pattern is:
/// 1. Empty — global default.
/// 2. A metric name prefix — `name.starts_with(pattern)`.
/// 3. A module path prefix — `module.starts_with(pattern)`.
/// 4. An exact module component — `module` contains `"::{pattern}"`.
///
/// Among matching directives, the longest pattern wins (so a directive for a
/// submodule overrides one for its parent, and any pattern overrides the bare
/// global level); among equal-length patterns, the last one wins.
fn threshold_for(directives: &[FilterDirective], name: &str, module: &str) -> Option<u8> {
    let mut best: Option<(usize, u8)> = None;
    for dir in directives {
        let matches = dir.pattern.is_empty()
            || name.starts_with(dir.pattern.as_str())
            || module.starts_with(dir.pattern.as_str())
            || module.contains(&format!("::{}", dir.pattern));
        if matches && best.is_none_or(|(len, _)| dir.pattern.len() >= len) {
            best = Some((dir.pattern.len(), dir.threshold));
        }
    }
    best.map(|(_, threshold)| threshold)
}

impl Filter {
    /// Parses a config-layer directive string, ignoring the `METRICS_FILTER`
    /// env var.
    pub fn parse(s: &str) -> Self {
        Self::from_layers(s, None)
    }

    /// Builds a filter with an empty config layer and the env layer read
    /// from the `METRICS_FILTER` env variable (permissive when unset).
    pub fn from_env() -> Self {
        let env = std::env::var("METRICS_FILTER").ok();
        Self::from_layers("", env.as_deref())
    }

    /// Builds a filter from the node config's directive string and the
    /// `METRICS_FILTER` env var's.
    pub fn from_layers(config: &str, env: Option<&str>) -> Self {
        Self {
            config: Layer::parse(config),
            env: Layer::parse(env.unwrap_or("")),
            runtime: RwLock::new(None),
        }
    }

    /// Returns a snapshot of the runtime override layer, if one is set.
    fn runtime_layer(&self) -> Option<Arc<Layer>> {
        self.runtime.read().unwrap().clone()
    }

    /// Layered evaluation, starting from runtime -> env -> config. Returns the
    /// threshold for a metric, or the default threshold if no layer has a
    /// matching directive.
    fn threshold(&self, runtime: Option<&Layer>, name: &str, module: &str) -> u8 {
        runtime
            .and_then(|layer| threshold_for(&layer.directives, name, module))
            .or_else(|| threshold_for(&self.env.directives, name, module))
            .or_else(|| threshold_for(&self.config.directives, name, module))
            .unwrap_or(DEFAULT_THRESHOLD)
    }

    /// Returns `true` if a registered metric named `name` in `module` at
    /// verbosity `level` should be exposed when gathering, per the layered
    /// directives currently in effect.
    #[inline]
    pub fn is_exposed(&self, name: &str, module: &str, level: MetricLevel) -> bool {
        self.threshold(self.runtime_layer().as_deref(), name, module) >= level.verbosity()
    }

    /// Returns the canonical string of the config layer's directives.
    pub fn config_filter_string(&self) -> &str {
        &self.config.raw
    }

    /// Returns the canonical string of the env (`METRICS_FILTER`) layer's
    /// directives.
    pub fn env_filter_string(&self) -> &str {
        &self.env.raw
    }

    /// Returns the canonical string of the runtime override layer, or `None`
    /// when no override is set.
    pub fn runtime_filter_string(&self) -> Option<String> {
        self.runtime
            .read()
            .unwrap()
            .as_ref()
            .map(|layer| layer.raw.clone())
    }

    /// Sets the runtime override layer. Rejects the whole update if
    /// any directive is invalid.
    pub fn set_runtime_filter(&self, s: &str) -> std::result::Result<(), String> {
        let directives = directive_parts(s)
            .map(parse_directive)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        *self.runtime.write().unwrap() = Some(Arc::new(Layer {
            raw: render_directives(&directives),
            directives,
        }));
        Ok(())
    }

    /// Drops the runtime override layer.
    pub fn reset_runtime_filter(&self) {
        *self.runtime.write().unwrap() = None;
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// A `prometheus::Collector` that can be boxed and cloned, so the registry can
/// keep a handle to re-`register`/`unregister` it as the filter changes.
pub trait CloneableCollector: prometheus::core::Collector {
    fn clone_box(&self) -> Box<dyn CloneableCollector>;
}

impl<T: prometheus::core::Collector + Clone + 'static> CloneableCollector for T {
    fn clone_box(&self) -> Box<dyn CloneableCollector> {
        Box::new(self.clone())
    }
}

/// A metric registered through the wrapper macros, retained so the filter can
/// toggle its membership in the inner registry at runtime.
struct RecordedMetric {
    module: String,
    level: MetricLevel,
    collector: Box<dyn CloneableCollector>,
}

/// Wraps `prometheus::Registry` with an embedded `Filter` so that
/// `register_*_with_registry!` macros can decide whether a metric is exposed.
///
/// Metrics registered through the wrapper macros are recorded with their module
/// path, level, and a collector handle. Exposure is enforced by membership in
/// the inner registry: a metric that the filter disables is `unregister`ed and
/// re-`register`ed if the filter later enables it.
#[derive(Clone)]
pub struct Registry {
    inner: prometheus::Registry,
    filter: Arc<Filter>,
    /// Name prefix passed to [`Registry::new_custom`]; gathered family names
    /// include it.
    prefix: Option<String>,
    /// Gathered family name → recorded metric, for metrics registered via the
    /// wrapper macros.
    registered: Arc<RwLock<HashMap<String, RecordedMetric>>>,
}

impl Registry {
    /// Creates a registry whose filter honours the `METRICS_FILTER` env var
    /// (permissive when unset).
    pub fn new() -> Self {
        Self {
            inner: prometheus::Registry::new(),
            filter: Arc::new(Filter::from_env()),
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
            filter: filter.unwrap_or_else(|| Arc::new(Filter::from_env())),
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

    fn exposed_name(&self, name: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}_{name}"),
            None => name.to_owned(),
        }
    }

    /// Used by the wrapper macros: on a successful registration, retains a
    /// handle to the metric with its module path and level, and immediately
    /// `unregister`s it from the inner registry if the current filter disables
    /// it.
    #[inline]
    pub fn record_collector<C>(
        &self,
        name: &str,
        module: &str,
        level: MetricLevel,
        result: prometheus::Result<C>,
    ) -> prometheus::Result<C>
    where
        C: prometheus::core::Collector + Clone + 'static,
    {
        let Ok(collector) = &result else {
            return result;
        };
        let collector: Box<dyn CloneableCollector> = Box::new(collector.clone());
        let exposed_name = self.exposed_name(name);

        let mut registered = self.registered.write().unwrap();
        if registered.contains_key(&exposed_name) {
            // The inner registry rejects a duplicate metric name, but only for names
            // it still holds — a name the filter disabled has been `unregister`ed, so
            // a second registration of it would slip through. This re-checks the
            // recorded set to keep rejecting duplicates regardless of filter state.
            // Check test `duplicate_name_is_rejected_even_when_filtered_out`
            let _ = self.inner.unregister(collector);
            return Err(prometheus::Error::AlreadyReg);
        }
        if !self.filter.is_exposed(&exposed_name, module, level) {
            let _ = self.inner.unregister(collector.clone_box());
        }
        registered.insert(
            exposed_name,
            RecordedMetric {
                module: module.to_owned(),
                level,
                collector,
            },
        );
        result
    }

    /// Re-evaluates every recorded metric against the current filter. Call
    /// after changing the filter's runtime override.
    pub fn reconcile(&self) {
        // Snapshot the runtime layer once so the whole pass sees a consistent
        // filter and avoids re-locking it per metric.
        let runtime = self.filter.runtime_layer();
        let registered = self.registered.read().unwrap();
        for (name, recorded) in registered.iter() {
            let want = self
                .filter
                .threshold(runtime.as_deref(), name, &recorded.module)
                >= recorded.level.verbosity();
            if want {
                let _ = self.register(recorded.collector.clone_box());
            } else {
                let _ = self.unregister(recorded.collector.clone_box());
            }
        }
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

    /// Gathers the registry's metric families.
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.inner.gather()
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
/// once from `METRICS_FILTER`.
fn default_filter() -> &'static Arc<Filter> {
    static INSTANCE: OnceLock<Arc<Filter>> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Filter::from_env()))
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_int_counter_with_registry!(
                    name,
                    $help,
                    ($registry).inner()
                ),
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_int_counter_vec_with_registry!(
                    name,
                    $help,
                    $labels,
                    ($registry).inner()
                ),
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_int_gauge_with_registry!(
                    name,
                    $help,
                    ($registry).inner()
                ),
            )
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_int_gauge_vec_with_registry!(
                    name,
                    $help,
                    $labels,
                    ($registry).inner()
                ),
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_with_registry!(
                    name,
                    $help,
                    ($registry).inner()
                ),
            )
            .map($crate::Histogram::new_some)
    }};
    ($name:expr, $help:expr, $buckets:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_with_registry!(
                    name,
                    $help,
                    $buckets,
                    ($registry).inner()
                ),
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_vec_with_registry!(
                    name,
                    $help,
                    $labels,
                    ($registry).inner()
                ),
            )
            .map($crate::HistogramVec::new_some)
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_vec_with_registry!(
                    name,
                    $help,
                    $labels,
                    $buckets,
                    ($registry).inner()
                ),
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_gauge_vec_with_registry!(
                    name,
                    $help,
                    $labels,
                    ($registry).inner()
                ),
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
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_gauge_with_registry!(
                    name,
                    $help,
                    ($registry).inner()
                ),
            )
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
        $crate::default_registry()
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_counter!(name, $help),
            )
            .map($crate::core::GenericCounter::new_some)
    }};
}

/// register_counter_with_registry!(name, help, registry)
#[macro_export]
macro_rules! register_counter_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {
        $crate::register_counter_with_registry!(
            $name, $help, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_counter_with_registry!(
                    name,
                    $help,
                    ($registry).inner()
                ),
            )
            .map($crate::core::GenericCounter::new_some)
    }};
}

/// register_counter_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_counter_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {
        $crate::register_counter_vec_with_registry!(
            $name, $help, $labels, $registry; $crate::MetricLevel::Debug
        )
    };
    ($name:expr, $help:expr, $labels:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        ($registry)
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_counter_vec_with_registry!(
                    name,
                    $help,
                    $labels,
                    ($registry).inner()
                ),
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
        $crate::default_registry()
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_counter_vec!(name, $help, $labels),
            )
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
        $crate::default_registry()
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_vec!(opts, $labels),
            )
            .map($crate::HistogramVec::new_some)
    }};
    ($name:expr, $help:expr, $labels:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::default_registry()
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_vec!(name, $help, $labels),
            )
            .map($crate::HistogramVec::new_some)
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::default_registry()
            .record_collector(
                name,
                module,
                $level,
                $crate::prometheus::register_histogram_vec!(name, $help, $labels, $buckets),
            )
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

        // the longer matching prefix shadows the shorter one
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

        // a pattern that is a prefix of the full module path (not only a `::`
        // component) matches.
        let filter = super::Filter::parse("iota_core=off");
        assert!(!filter.is_exposed("certs_total", "iota_core::authority", Debug));
        assert!(filter.is_exposed("certs_total", "starfish::core", Debug));
    }

    #[test]
    fn more_specific_pattern_wins_regardless_of_order() {
        use super::MetricLevel::{Info, Warn};
        // A blanket module directive does not shadow a more specific one,
        // whichever is written first ...
        for input in [
            "iota_core::authority=warn,iota_core=off",
            "iota_core=off,iota_core::authority=warn",
        ] {
            let filter = super::Filter::parse(input);
            assert!(
                filter.is_exposed("x", "iota_core::authority", Warn),
                "{input}"
            );
            assert!(
                !filter.is_exposed("x", "iota_core::checkpoints", Warn),
                "{input}"
            );
        }
        // ... and a trailing bare level does not cancel earlier specific
        // directives.
        let filter = super::Filter::parse("authority=off,info");
        assert!(!filter.is_exposed("authority", "m", Warn));
        assert!(filter.is_exposed("certs_total", "m", Info));
        assert!(!filter.is_exposed("certs_total", "m", Debug));
        // Among directives with the same (here: empty) pattern the last one
        // wins.
        assert!(super::Filter::parse("off,trace").is_exposed("authority", "m", Debug));
    }

    #[test]
    fn env_layer_overrides_config_only_where_it_matches() {
        use super::MetricLevel::{Info, Trace, Warn};
        // Where an env directive matches, it beats the config layer even if
        // the config directive is more specific ...
        let filter = super::Filter::from_layers(
            "iota_core::authority=off,starfish=warn",
            Some("iota_core=info"),
        );
        assert!(filter.is_exposed("x", "iota_core::authority", Info));
        assert!(!filter.is_exposed("x", "iota_core::authority", Debug));
        // ... where it does not, the config layer still applies.
        assert!(filter.is_exposed("x", "starfish::core", Warn));
        assert!(!filter.is_exposed("x", "starfish::core", Info));

        // A bare env level matches everything: a full override.
        let filter = super::Filter::from_layers("info,iota_core=warn", Some("trace"));
        assert!(filter.is_exposed("x", "iota_core::authority", Trace));
        assert!(filter.is_exposed("x", "m", Trace));

        // A blank env var contributes no directives, so the config layer
        // still applies.
        let filter = super::Filter::from_layers("off", Some(" "));
        assert!(!filter.is_exposed("x", "m", Warn));
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
    fn invalid_directives_are_dropped() {
        use super::MetricLevel::Trace;
        // An unrecognised value leaves the directive out, falling back to the
        // permissive default. Only the RUST_LOG-style level names are
        // accepted; the former `on`/`true`/`1` and `false`/`0` aliases are
        // invalid too.
        for level in ["maybe", "on", "true", "1", "false", "0"] {
            let filter = super::Filter::parse(&format!("authority={level}"));
            assert!(
                filter.is_exposed("authority", "m", Trace),
                "{level} should be dropped as invalid, leaving the default"
            );
        }
        // a bare token without `=LEVEL` is parsed as a global value and, being
        // invalid, dropped — it does NOT enable/disable the `authority` subsystem.
        assert!(super::Filter::parse("authority").is_exposed("authority", "m", Trace));
        // a valid directive alongside an invalid one still takes effect.
        let filter = super::Filter::parse("authority=off,bogus=nope");
        assert!(!filter.is_exposed("authority", "m", Debug));
    }

    #[test]
    fn whitespace_is_trimmed() {
        let filter = super::Filter::parse("  authority = off ,  authority_aggregator = trace  ");
        assert!(!filter.is_exposed("authority", "m", Debug));
        assert!(filter.is_exposed("authority_aggregator", "m", Debug));
    }

    #[test]
    fn from_env_is_permissive_when_unset() {
        use super::{Arc, Filter, MetricLevel, Registry};

        // A set env var would add an env layer, so `Filter::from_env` is
        // only exercised when it is unset; `Filter::from_layers` covers the
        // set case.
        if std::env::var_os("METRICS_FILTER").is_some() {
            return;
        }

        // No env, no config -> everything is exposed.
        assert!(Filter::from_env().is_exposed("anything", "m", MetricLevel::Trace));
        assert!(Filter::from_env().is_exposed("anything", "m", Debug));

        // Registries built to share a filter see the same decisions.
        let filter = Arc::new(Filter::parse("off,authority=trace"));
        assert!(filter.is_exposed("authority", "m", MetricLevel::Debug));
        assert!(!filter.is_exposed("consensus", "m", MetricLevel::Debug));
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
    fn off_directive_hides_but_metric_keeps_collecting() {
        let reg = registry("g_hidden=off");
        let g = crate::register_int_gauge_with_registry!("g_hidden", "h", &reg; MetricLevel::Warn)
            .unwrap();
        // The metric handle is live and keeps collecting through the caller's
        // copy, even though the filter unregistered it from the inner registry,
        // so it is absent from gather output.
        assert_eq!(format!("{g:?}"), "GenericGauge");
        g.set(9);
        assert_eq!(g.get(), 9);
        assert_eq!(gathered_names(&reg), Vec::<String>::new());
    }

    #[test]
    fn duplicate_name_is_rejected_even_when_filtered_out() {
        // The `off` directive unregisters the metric from the inner registry,
        // but a second registration of the same name must still be rejected.
        let reg = registry("g_dup=off");
        // The first registration is registered then un-registered because `g_dup=off`.
        crate::register_int_gauge_with_registry!("g_dup", "h", &reg; MetricLevel::Warn).unwrap();
        // The second registration is rejected because the metrics with the same name
        // exists, it is just not in the registry map due to `g_dup=off`, so the
        // second metric shouldn't exist.
        let err = crate::register_int_gauge_with_registry!("g_dup", "h", &reg; MetricLevel::Warn)
            .unwrap_err();
        assert!(
            matches!(err, prometheus::Error::AlreadyReg),
            "unexpected error: {err:?}"
        );
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
mod runtime_filter_tests {
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

    // The node drives this via `RegistryService`; here we exercise the same
    // two steps directly: set the runtime override, then reconcile membership.
    fn set_runtime(registry: &Registry, s: &str) {
        registry.filter().set_runtime_filter(s).unwrap();
        registry.reconcile();
    }

    fn reset_runtime(registry: &Registry) {
        registry.filter().reset_runtime_filter();
        registry.reconcile();
    }

    #[test]
    fn raising_runtime_level_exposes_collected_metrics() {
        // A `warn` startup threshold hides the debug metric …
        let reg = registry("runtime_filter_tests=warn");
        crate::register_int_gauge_with_registry!("g_warn", "h", &reg; MetricLevel::Warn).unwrap();
        let g_debug = crate::register_int_gauge_with_registry!("g_debug", "h", &reg).unwrap();
        g_debug.set(7);
        assert_eq!(gathered_names(&reg), ["g_warn"]);

        // … so raising the exposure level at runtime reveals it, with the
        // values it collected while hidden.
        set_runtime(&reg, "runtime_filter_tests=debug");
        assert_eq!(gathered_names(&reg), ["g_debug", "g_warn"]);
        let family = reg
            .gather()
            .into_iter()
            .find(|f| f.name() == "g_debug")
            .unwrap();
        assert_eq!(family.get_metric()[0].get_gauge().value() as i64, 7);

        // The same holds for a startup `off` directive: a runtime directive
        // matching the metric exposes it with its collected value.
        let reg = registry("g_hidden=off");
        let g = crate::register_int_gauge_with_registry!("g_hidden", "h", &reg; MetricLevel::Warn)
            .unwrap();
        g.set(9);
        assert_eq!(gathered_names(&reg), Vec::<String>::new());
        set_runtime(&reg, "g_hidden=warn");
        assert_eq!(gathered_names(&reg), ["g_hidden"]);
        let family = &reg.gather()[0];
        assert_eq!(family.get_metric()[0].get_gauge().value() as i64, 9);
    }

    #[test]
    fn runtime_layer_falls_back_to_startup_where_it_does_not_match() {
        let reg = registry("g_a=off");
        crate::register_int_gauge_with_registry!("g_a", "h", &reg; MetricLevel::Warn).unwrap();
        crate::register_int_gauge_with_registry!("g_b", "h", &reg; MetricLevel::Warn).unwrap();
        assert_eq!(gathered_names(&reg), ["g_b"]);

        // The override hides g_b; g_a is matched by no override directive
        // and keeps its startup exposure (hidden).
        set_runtime(&reg, "g_b=off");
        assert_eq!(gathered_names(&reg), Vec::<String>::new());

        // An empty override matches nothing, leaving the startup directives
        // fully in effect.
        set_runtime(&reg, "");
        assert_eq!(gathered_names(&reg), ["g_b"]);

        // A bare level matches everything: a full temporary override.
        set_runtime(&reg, "trace");
        assert_eq!(gathered_names(&reg), ["g_a", "g_b"]);

        reset_runtime(&reg);
        assert_eq!(gathered_names(&reg), ["g_b"]);
    }

    #[test]
    fn filter_reports_its_layers() {
        let filter = Filter::from_layers("authority=off", Some("checkpoints=warn"));
        assert_eq!(filter.config_filter_string(), "authority=off");
        assert_eq!(filter.env_filter_string(), "checkpoints=warn");
        assert_eq!(filter.runtime_filter_string(), None);
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));

        filter.set_runtime_filter("authority=warn").unwrap();
        assert_eq!(
            filter.runtime_filter_string().as_deref(),
            Some("authority=warn")
        );
        assert!(filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Debug));

        filter.reset_runtime_filter();
        assert_eq!(filter.runtime_filter_string(), None);
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));
    }

    #[test]
    fn filter_string_is_canonical_and_round_trips() {
        // Invalid startup directives are dropped, and the reported config
        // string reflects the directives actually in effect — so it can
        // always be POSTed back through the strict runtime setter.
        let filter = Filter::parse("foo=bogus, typed_store=warn ,info");
        assert_eq!(filter.config_filter_string(), "typed_store=warn,info");
        filter
            .set_runtime_filter(filter.config_filter_string())
            .unwrap();
        assert_eq!(
            filter.runtime_filter_string().as_deref(),
            Some("typed_store=warn,info")
        );
    }

    #[test]
    fn set_runtime_filter_rejects_invalid_directives() {
        let filter = Filter::parse("authority=off");
        let err = filter
            .set_runtime_filter("authority=warn,bogus=nope")
            .unwrap_err();
        assert!(err.contains("bogus=nope"), "unexpected error: {err}");
        // The failed update leaves the runtime layer unset.
        assert_eq!(filter.runtime_filter_string(), None);
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));
    }
}
