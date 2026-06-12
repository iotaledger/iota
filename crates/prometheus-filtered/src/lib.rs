// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Drop-in replacement for the `prometheus` crate with optional per-metric
//! filtering.
//!
//! Replace `use prometheus::*` with `use prometheus_filtered::*` and set
//! `METRICS_FILTER` (or call `Registry::with_config`) to control which metrics
//! are registered.
//!
//! Filter syntax: comma-separated `pattern=on|off` directives, last-match
//! wins.  A bare `off` or `on` sets the global default.  A pattern matches if
//! it is a prefix of the metric name OR is a component/prefix of the calling
//! module path (e.g. `traffic_controller` matches
//! `iota_core::traffic_controller::metrics`).
//!
//! Examples:
//! - `METRICS_FILTER=off,authority=on`
//! - `METRICS_FILTER=authority=off`

use std::sync::Arc;

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

    pub struct GenericCounter<P: Atomic>(pub(super) Option<prometheus::core::GenericCounter<P>>);

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

    pub struct GenericGauge<P: Atomic>(pub(super) Option<prometheus::core::GenericGauge<P>>);

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

    pub struct GenericCounterVec<P: Atomic>(
        pub(super) Option<prometheus::core::GenericCounterVec<P>>,
    );

    impl_generic_metric_traits!(GenericCounterVec);
    impl_generic_metric_vec!(GenericCounterVec, GenericCounter);

    pub struct GenericGaugeVec<P: Atomic>(pub(super) Option<prometheus::core::GenericGaugeVec<P>>);

    impl_generic_metric_traits!(GenericGaugeVec);
    impl_generic_metric_vec!(GenericGaugeVec, GenericGauge);

    pub struct Histogram(pub Option<prometheus::Histogram>);

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

    pub struct HistogramVec(pub Option<prometheus::HistogramVec>);

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

struct FilterDirective {
    /// Empty string means global catch-all.
    pattern: String,
    enabled: bool,
}

/// Parses and evaluates `METRICS_FILTER`-style directives.
///
/// Filter string: comma-separated `pattern=on|off`. Bare `on`/`off` is a
/// global default.  A pattern matches if it is a prefix of the metric name OR
/// is a component/prefix of the module path (e.g. `traffic_controller` matches
/// `iota_core::traffic_controller::metrics`).
#[derive(Default)]
pub struct Filter {
    directives: Vec<FilterDirective>,
}

impl Filter {
    fn parse(s: &str) -> Self {
        let directives = s
            .split(',')
            .filter_map(|part| {
                let part = part.trim();
                if part.is_empty() {
                    None
                } else {
                    let (pattern, enabled) = if let Some(eq) = part.rfind('=') {
                        (part[..eq].trim().to_owned(), part[eq + 1..].trim())
                    } else {
                        (String::new(), part)
                    };
                    match enabled {
                        "on" | "true" | "1" => Some(true),
                        "off" | "false" | "0" => Some(false),
                        other => {
                            warn!("invalid prometheus filter value {other:?} in {part:?}");
                            None
                        }
                    }
                    .map(|enabled| FilterDirective { pattern, enabled })
                }
            })
            .collect();
        Self { directives }
    }

    fn from_env() -> Self {
        std::env::var("METRICS_FILTER")
            .ok()
            .map(|s| Self::parse(&s))
            .unwrap_or_default()
    }

    /// Returns `true` if the metric should be registered (default when no
    /// directives match: `true`).
    ///
    /// Matching order (last wins):
    /// 1. Empty pattern — global default.
    /// 2. `name.starts_with(pattern)` — metric name prefix.
    /// 3. `module.starts_with(pattern)` — module path prefix.
    /// 4. `module` contains `"::{pattern}"` — exact module component.
    #[inline]
    pub fn is_enabled(&self, name: &str, module: &str) -> bool {
        let mut result = true;
        for dir in &self.directives {
            if dir.pattern.is_empty()
                || name.starts_with(dir.pattern.as_str())
                || module.starts_with(dir.pattern.as_str())
                || module.contains(&format!("::{}", dir.pattern))
            {
                result = dir.enabled;
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Wraps `prometheus::Registry` with an embedded `Filter` so that
/// `register_*_with_registry!` macros can decide at construction time whether
/// a metric should be active.
#[derive(Clone)]
pub struct Registry {
    inner: prometheus::Registry,
    filter: Arc<Filter>,
}

impl Registry {
    /// Creates a registry whose filter is read from `METRICS_FILTER` env var.
    pub fn new() -> Self {
        Self {
            inner: prometheus::Registry::new(),
            filter: Arc::new(Filter::from_env()),
        }
    }

    /// Creates a custom-prefixed registry whose filter is read from the
    /// `METRICS_FILTER` env var.
    pub fn new_custom(
        prefix: Option<String>,
        labels: Option<std::collections::HashMap<String, String>>,
    ) -> prometheus::Result<Self> {
        Ok(Self {
            inner: prometheus::Registry::new_custom(prefix, labels)?,
            filter: Arc::new(Filter::from_env()),
        })
    }

    /// Creates a registry combining the env-var filter (appended last, highest
    /// priority) with the supplied config string.
    pub fn with_filter(filter_str: &str) -> Self {
        Self {
            inner: prometheus::Registry::new(),
            filter: Arc::new(Filter::parse(filter_str)),
        }
    }

    /// Used by wrapper macros to decide whether to register a metric.
    #[inline]
    pub fn is_enabled(&self, name: &str, module: &str) -> bool {
        self.filter.is_enabled(name, module)
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

/// Returns the process-wide `Filter` parsed once from `METRICS_FILTER`.
///
/// Shared by [`default_registry`] and the global `register_*!` macros (via
/// [`default_registry`]) so that metrics on the default registry honour the
/// same filtering as those on explicit registries.
pub fn default_filter() -> &'static Arc<Filter> {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<Arc<Filter>> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Filter::from_env()))
}

/// Returns a reference to the global default `Registry`, wrapping the
/// underlying `prometheus::default_registry()`.  Metrics registered here
/// appear in the standard prometheus default gather output.
pub fn default_registry() -> &'static Registry {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<Registry> = OnceLock::new();
    INSTANCE.get_or_init(|| Registry {
        inner: prometheus::default_registry().clone(),
        filter: default_filter().clone(),
    })
}

// ---------------------------------------------------------------------------
// Wrapper macros
// ---------------------------------------------------------------------------
//
// Each macro captures `module_path!()` at the call site so the filter can
// match by subsystem in addition to metric name.
//
// The `$registry` must be a `prometheus_filtered::Registry`.  On success the
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
    ($name:expr, $help:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_int_counter_with_registry!(
                name,
                $help,
                ($registry).inner()
            )
            .map($crate::core::GenericCounter::new_some)
        } else {
            ::std::result::Result::Ok($crate::core::GenericCounter::new_none())
        }
    }};
}

/// register_int_counter_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_int_counter_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_int_counter_vec_with_registry!(
                name,
                $help,
                $labels,
                ($registry).inner()
            )
            .map($crate::IntCounterVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::IntCounterVec::new_none())
        }
    }};
}

/// register_int_gauge_with_registry!(name, help, registry)
#[macro_export]
macro_rules! register_int_gauge_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_int_gauge_with_registry!(name, $help, ($registry).inner())
                .map($crate::core::GenericGauge::new_some)
        } else {
            ::std::result::Result::Ok($crate::core::GenericGauge::new_none())
        }
    }};
}

/// register_int_gauge_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_int_gauge_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_int_gauge_vec_with_registry!(
                name,
                $help,
                $labels,
                ($registry).inner()
            )
            .map($crate::IntGaugeVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::IntGaugeVec::new_none())
        }
    }};
}

/// register_histogram_with_registry!(name, help, registry)
/// register_histogram_with_registry!(name, help, buckets, registry)
#[macro_export]
macro_rules! register_histogram_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_histogram_with_registry!(name, $help, ($registry).inner())
                .map($crate::Histogram::new_some)
        } else {
            ::std::result::Result::Ok($crate::Histogram::new_none())
        }
    }};
    ($name:expr, $help:expr, $buckets:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_histogram_with_registry!(
                name,
                $help,
                $buckets,
                ($registry).inner()
            )
            .map($crate::Histogram::new_some)
        } else {
            ::std::result::Result::Ok($crate::Histogram::new_none())
        }
    }};
}

/// register_histogram_vec_with_registry!(name, help, labels, registry)
/// register_histogram_vec_with_registry!(name, help, labels, buckets, registry)
#[macro_export]
macro_rules! register_histogram_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_histogram_vec_with_registry!(
                name,
                $help,
                $labels,
                ($registry).inner()
            )
            .map($crate::HistogramVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::HistogramVec::new_none())
        }
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_histogram_vec_with_registry!(
                name,
                $help,
                $labels,
                $buckets,
                ($registry).inner()
            )
            .map($crate::HistogramVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::HistogramVec::new_none())
        }
    }};
}

/// register_gauge_vec_with_registry!(name, help, labels, registry)
#[macro_export]
macro_rules! register_gauge_vec_with_registry {
    ($name:expr, $help:expr, $labels:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_gauge_vec_with_registry!(
                name,
                $help,
                $labels,
                ($registry).inner()
            )
            .map($crate::core::GenericGaugeVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::core::GenericGaugeVec::new_none())
        }
    }};
}

/// register_gauge_with_registry!(name, help, registry)
#[macro_export]
macro_rules! register_gauge_with_registry {
    ($name:expr, $help:expr, $registry:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if ($registry).is_enabled(name, module) {
            $crate::prometheus::register_gauge_with_registry!(name, $help, ($registry).inner())
                .map($crate::core::GenericGauge::new_some)
        } else {
            ::std::result::Result::Ok($crate::core::GenericGauge::new_none())
        }
    }};
}

/// register_counter!(name, help)  — global prometheus registry, filtered.
#[macro_export]
macro_rules! register_counter {
    ($name:expr, $help:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if $crate::default_filter().is_enabled(name, module) {
            $crate::prometheus::register_counter!(name, $help)
                .map($crate::core::GenericCounter::new_some)
        } else {
            ::std::result::Result::Ok($crate::core::GenericCounter::new_none())
        }
    }};
}

/// register_counter_vec!(name, help, labels)  — global registry, filtered.
#[macro_export]
macro_rules! register_counter_vec {
    ($name:expr, $help:expr, $labels:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if $crate::default_filter().is_enabled(name, module) {
            $crate::prometheus::register_counter_vec!(name, $help, $labels)
                .map($crate::core::GenericCounterVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::core::GenericCounterVec::new_none())
        }
    }};
}

/// register_histogram_vec!(opts, labels) or (name, help, labels) or (name,
/// help, labels, buckets) — global prometheus registry, filtered.
#[macro_export]
macro_rules! register_histogram_vec {
    ($opts:expr, $labels:expr $(,)?) => {{
        let opts = $opts;
        let name: &str = &opts.common_opts.name;
        let module: &str = module_path!();
        if $crate::default_filter().is_enabled(name, module) {
            $crate::prometheus::register_histogram_vec!(opts, $labels)
                .map($crate::HistogramVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::HistogramVec::new_none())
        }
    }};
    ($name:expr, $help:expr, $labels:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if $crate::default_filter().is_enabled(name, module) {
            $crate::prometheus::register_histogram_vec!(name, $help, $labels)
                .map($crate::HistogramVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::HistogramVec::new_none())
        }
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        if $crate::default_filter().is_enabled(name, module) {
            $crate::prometheus::register_histogram_vec!(name, $help, $labels, $buckets)
                .map($crate::HistogramVec::new_some)
        } else {
            ::std::result::Result::Ok($crate::HistogramVec::new_none())
        }
    }};
}
