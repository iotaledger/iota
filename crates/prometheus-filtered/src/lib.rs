// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Drop-in replacement for the `prometheus` crate with optional per-metric
//! filtering.
//!
//! Replace `use prometheus::*` with `use prometheus_filtered::*` to control
//! which metrics are exposed. The active filter is one directive set, built
//! by merging its inputs in precedence order — the node config's directives,
//! the `METRICS_FILTER` environment variable's, and an optional runtime
//! override: a higher-precedence directive replaces every directive whose
//! pattern it prefixes, and the directives it does not shadow keep their
//! effect (see [`Filter`]).
//!
//! Filter syntax: comma-separated `pattern=LEVEL` directives, where `LEVEL`
//! is one of `off`, `warn`, `info`, `debug`, `trace`. A bare `LEVEL` token
//! (no `pattern=`) matches every metric — as an env or runtime directive it
//! replaces the whole filter. A pattern matches if it is a
//! prefix of the metric name OR is a component/prefix of the calling module
//! path (e.g. `traffic_controller` matches
//! `iota_core::traffic_controller::metrics`). When several directives
//! match the same metric, the most specific one (longest pattern) wins,
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
// ---------------------------------------------------------------------------
// prometheus re-exports
// ---------------------------------------------------------------------------

// Filtering is enforced by registry membership (see `Registry::register_filtered`
// and `Registry::reconcile`), so the metric types need no wrapping: re-export
// prometheus's own types and generic primitives directly.
pub use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramTimer, HistogramVec, IntCounter,
    IntCounterVec, IntGauge, IntGaugeVec, core,
};
// Re-export the prometheus items callers reach for through this crate.
pub use prometheus::{
    DEFAULT_BUCKETS, Encoder, Error, HistogramOpts, Opts, PROTOBUF_FORMAT, ProtobufEncoder, Result,
    TextEncoder, exponential_buckets, gather, histogram_opts, linear_buckets, opts, proto,
};
use tracing::warn;

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

/// Filter holds two directive sets: the immutable startup directives (the
/// node config's with the `METRICS_FILTER` env var's merged over them) and
/// the directives currently in effect — the startup directives, with the
/// runtime override merged over them while one is set.
#[derive(Default)]
pub struct Filter {
    /// The startup directives; what [`Filter::reset_runtime_filter`] restores.
    startup: Arc<DirectiveSet>,
    /// The directives consulted by [`Filter::is_exposed`].
    runtime: RwLock<Arc<DirectiveSet>>,
}

/// The source strings for one filter input. `directives` is parsed for
/// matching; `display` is what [`Filter::filter_string`] and
/// [`Filter::startup_filter_string`] echo back (e.g. the group-form string a
/// caller expanded before building the filter). Use [`FilterSource::new`]
/// when the two are the same string.
#[derive(Clone, Copy)]
pub struct FilterSource<'a> {
    pub directives: &'a str,
    pub display: &'a str,
}

impl<'a> FilterSource<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            directives: s,
            display: s,
        }
    }

    /// `directives` drive matching; `display` is echoed by the admin
    /// endpoint.
    pub fn with_display(directives: &'a str, display: &'a str) -> Self {
        Self {
            directives,
            display,
        }
    }
}

/// One parsed filter input: the matching directives plus the display
/// directives they are reported as.
#[derive(Default)]
struct DirectiveSet {
    directives: Vec<FilterDirective>,
    display: Vec<FilterDirective>,
}

impl DirectiveSet {
    fn from_source(source: FilterSource<'_>) -> Self {
        Self {
            directives: parse_valid_directives(source.directives),
            display: parse_valid_directives(source.display),
        }
    }

    /// Merges `over` on top of `self`: an `over` directive replaces every
    /// directive whose pattern it prefixes (a bare level replaces
    /// everything), and directives it does not shadow keep their effect —
    /// for overlapping unrelated patterns the usual longest-pattern-wins
    /// matching applies.
    fn merged(&self, over: &Self) -> Self {
        Self {
            directives: merge_directives(&self.directives, &over.directives),
            display: merge_directives(&self.display, &over.display),
        }
    }
}

/// Appends `over` to `base`, dropping the `base` directives shadowed by an
/// `over` directive's pattern prefix.
fn merge_directives(base: &[FilterDirective], over: &[FilterDirective]) -> Vec<FilterDirective> {
    base.iter()
        .filter(|dir| !over.iter().any(|o| dir.pattern.starts_with(&o.pattern)))
        .chain(over.iter())
        .cloned()
        .collect()
}

/// Parses a directive string, dropping invalid directives with a warning.
fn parse_valid_directives(s: &str) -> Vec<FilterDirective> {
    directive_parts(s)
        .filter_map(|part| {
            parse_directive(part)
                .map_err(|err| warn!("dropping prometheus filter directive: {err}"))
                .ok()
        })
        .collect()
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

/// Like [`threshold_for`], but falling back to the permissive default when no
/// directive matches.
fn threshold(directives: &[FilterDirective], name: &str, module: &str) -> u8 {
    threshold_for(directives, name, module).unwrap_or(DEFAULT_THRESHOLD)
}

impl Filter {
    // Parses a single directive string as the config source, ignoring the
    // `METRICS_FILTER` env var. Convenience for
    // `from_sources(FilterSource::new(s), None)`.
    pub fn parse(s: &str) -> Self {
        Self::from_sources(FilterSource::new(s), None)
    }

    /// Builds a filter with an empty config source and the `METRICS_FILTER`
    /// env variable's directives (permissive when unset).
    pub fn from_env() -> Self {
        let env = std::env::var("METRICS_FILTER").ok();
        Self::from_sources(FilterSource::new(""), env.as_deref().map(FilterSource::new))
    }

    /// Builds a filter whose startup directives are the env source merged
    /// over the config source (see [`DirectiveSet::merged`]).
    pub fn from_sources(config: FilterSource<'_>, env: Option<FilterSource<'_>>) -> Self {
        let startup = Arc::new(DirectiveSet::from_source(config).merged(
            &DirectiveSet::from_source(env.unwrap_or(FilterSource::new(""))),
        ));
        Self {
            runtime: RwLock::new(startup.clone()),
            startup,
        }
    }

    /// Returns a snapshot of the directives currently in effect.
    fn runtime(&self) -> Arc<DirectiveSet> {
        self.runtime.read().unwrap().clone()
    }

    /// Returns `true` if a registered metric named `name` in `module` at
    /// verbosity `level` should be exposed when gathering, per the directives
    /// currently in effect.
    #[inline]
    pub fn is_exposed(&self, name: &str, module: &str, level: MetricLevel) -> bool {
        threshold(&self.runtime().directives, name, module) >= level.verbosity()
    }

    /// Returns the display string of the directives currently in effect.
    pub fn filter_string(&self) -> String {
        render_directives(&self.runtime().display)
    }

    /// Returns the startup directives' display string.
    pub fn startup_filter_string(&self) -> String {
        render_directives(&self.startup.display)
    }

    /// Replaces the directives in effect with the runtime override merged
    /// over the startup directives (see [`DirectiveSet::merged`]) — each call
    /// starts from the startup directives again rather than stacking on the
    /// previous override. Rejects the whole update if any directive is
    /// invalid.
    pub fn set_runtime_filter(&self, source: FilterSource<'_>) -> std::result::Result<(), String> {
        let directives = directive_parts(source.directives)
            .map(parse_directive)
            .collect::<std::result::Result<Vec<_>, String>>()?;
        let over = DirectiveSet {
            directives,
            display: parse_valid_directives(source.display),
        };
        *self.runtime.write().unwrap() = Arc::new(self.startup.merged(&over));
        Ok(())
    }

    /// Drops the runtime override, restoring the startup directives.
    pub fn reset_runtime_filter(&self) {
        *self.runtime.write().unwrap() = self.startup.clone();
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

    /// Used by the wrapper macros: Registers `collector` and records a
    /// handle so the filter can toggle its exposure later. The collector joins
    /// the inner registry only if the current filter exposes it.
    #[inline]
    pub fn register_filtered<C>(
        &self,
        name: &str,
        module: &str,
        level: MetricLevel,
        collector: C,
    ) -> prometheus::Result<C>
    where
        C: prometheus::core::Collector + Clone + 'static,
    {
        let exposed_name = self.exposed_name(name);
        let mut registered = self.registered.write().unwrap();
        // Reject duplicate registration of the same metric name.
        if registered.contains_key(&exposed_name) {
            return Err(prometheus::Error::AlreadyReg);
        }

        let cloneable_collector: Box<dyn CloneableCollector> = Box::new(collector.clone());
        if self.filter.is_exposed(&exposed_name, module, level) {
            self.inner.register(cloneable_collector.clone_box())?;
        }
        registered.insert(
            exposed_name,
            RecordedMetric {
                module: module.to_owned(),
                level,
                collector: cloneable_collector,
            },
        );
        Ok(collector)
    }

    /// Re-evaluates every recorded metric against the current filter. Call
    /// after changing the filter's runtime override.
    pub fn reconcile(&self) {
        // Snapshot the directives once so the whole pass sees a consistent
        // filter and avoids re-locking them per metric.
        let runtime = self.filter.runtime();
        let registered = self.registered.read().unwrap();
        for (name, recorded) in registered.iter() {
            let want = threshold(&runtime.directives, name, &recorded.module)
                >= recorded.level.verbosity();
            if want {
                let _ = self.register(recorded.collector.clone_box());
            } else {
                let _ = self.unregister(recorded.collector.clone_box());
            }
        }
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
// The `$registry` must be a `prometheus_filtered::Registry`. Each macro builds
// the prometheus metric unregistered, then hands it to
// [`Registry::register_filtered`], which owns registration and returns the
// metric. The filter never turns a successful construction into an `Err`.
//
// `$crate::prometheus::` names the metric constructors so callers don't need a
// direct `prometheus` crate dependency.
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
        $crate::prometheus::IntCounter::new(name, $help)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::IntCounterVec::new($crate::prometheus::Opts::new(name, $help), $labels)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::IntGauge::new(name, $help)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::IntGaugeVec::new($crate::prometheus::Opts::new(name, $help), $labels)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::Histogram::with_opts($crate::prometheus::HistogramOpts::new(name, $help))
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
    }};
    ($name:expr, $help:expr, $buckets:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::prometheus::Histogram::with_opts(
            $crate::prometheus::HistogramOpts::new(name, $help).buckets($buckets),
        )
        .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::HistogramVec::new(
            $crate::prometheus::HistogramOpts::new(name, $help),
            $labels,
        )
        .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr, $registry:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::prometheus::HistogramVec::new(
            $crate::prometheus::HistogramOpts::new(name, $help).buckets($buckets),
            $labels,
        )
        .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::GaugeVec::new($crate::prometheus::Opts::new(name, $help), $labels)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::Gauge::new(name, $help)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::Counter::new(name, $help).and_then(|metric| {
            $crate::default_registry().register_filtered(name, module, $level, metric)
        })
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
        $crate::prometheus::Counter::new(name, $help)
            .and_then(|metric| ($registry).register_filtered(name, module, $level, metric))
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
        $crate::prometheus::CounterVec::new($crate::prometheus::Opts::new(name, $help), $labels)
            .and_then(|metric| {
                ($registry).register_filtered(name, module, $level, metric)
            })
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
        $crate::prometheus::CounterVec::new($crate::prometheus::Opts::new(name, $help), $labels)
            .and_then(|metric| {
                $crate::default_registry().register_filtered(name, module, $level, metric)
            })
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
        let name = opts.common_opts.name.clone();
        let name: &str = &name;
        let module: &str = module_path!();
        $crate::prometheus::HistogramVec::new(opts, $labels).and_then(|metric| {
            $crate::default_registry().register_filtered(name, module, $level, metric)
        })
    }};
    ($name:expr, $help:expr, $labels:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::prometheus::HistogramVec::new(
            $crate::prometheus::HistogramOpts::new(name, $help),
            $labels,
        )
        .and_then(|metric| {
            $crate::default_registry().register_filtered(name, module, $level, metric)
        })
    }};
    ($name:expr, $help:expr, $labels:expr, $buckets:expr ; $level:expr $(,)?) => {{
        let _n = $name;
        let name: &str = &*_n;
        let module: &str = module_path!();
        $crate::prometheus::HistogramVec::new(
            $crate::prometheus::HistogramOpts::new(name, $help).buckets($buckets),
            $labels,
        )
        .and_then(|metric| {
            $crate::default_registry().register_filtered(name, module, $level, metric)
        })
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
    fn env_directives_merge_over_config_into_one_startup_filter() {
        use super::MetricLevel::{Info, Trace, Warn};
        // An env directive replaces every config directive whose pattern it
        // prefixes, so it beats the config even where the config directive is
        // more specific ...
        let filter = super::Filter::from_sources(
            super::FilterSource::new("iota_core::authority=off,starfish=warn"),
            Some(super::FilterSource::new("iota_core=info")),
        );
        assert!(filter.is_exposed("x", "iota_core::authority", Info));
        assert!(!filter.is_exposed("x", "iota_core::authority", Debug));
        // ... where it does not, the config directives still apply.
        assert!(filter.is_exposed("x", "starfish::core", Warn));
        assert!(!filter.is_exposed("x", "starfish::core", Info));
        // The two sources collapse into a single startup string.
        assert_eq!(
            filter.startup_filter_string(),
            "starfish=warn,iota_core=info"
        );

        // A bare env level prefixes everything: a full override.
        let filter = super::Filter::from_sources(
            super::FilterSource::new("info,iota_core=warn"),
            Some(super::FilterSource::new("trace")),
        );
        assert!(filter.is_exposed("x", "iota_core::authority", Trace));
        assert!(filter.is_exposed("x", "m", Trace));
        assert_eq!(filter.startup_filter_string(), "trace");

        // A blank env var contributes no directives, so the config directives
        // still apply.
        let filter = super::Filter::from_sources(
            super::FilterSource::new("off"),
            Some(super::FilterSource::new(" ")),
        );
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
        ];
        // A set env var would add env directives, so `Filter::from_env` is
        // only exercised when it is unset; `Filter::from_sources` covers the
        // set case.
        if std::env::var_os("METRICS_FILTER").is_none() {
            filters.push(super::Filter::from_env());
        }
        for filter in filters {
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
    use super::{Filter, FilterSource, MetricLevel, Registry};

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
        registry
            .filter()
            .set_runtime_filter(FilterSource::new(s))
            .unwrap();
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
    fn runtime_override_keeps_startup_directives_it_does_not_shadow() {
        let reg = registry("g_a=off");
        crate::register_int_gauge_with_registry!("g_a", "h", &reg; MetricLevel::Warn).unwrap();
        crate::register_int_gauge_with_registry!("g_b", "h", &reg; MetricLevel::Warn).unwrap();
        assert_eq!(gathered_names(&reg), ["g_b"]);

        // The override hides g_b; g_a is shadowed by no override directive
        // and keeps its startup exposure (hidden).
        set_runtime(&reg, "g_b=off");
        assert_eq!(gathered_names(&reg), Vec::<String>::new());

        // An empty override contributes nothing, leaving the startup
        // directives fully in effect — and replaces the previous override
        // rather than accumulating with it.
        set_runtime(&reg, "");
        assert_eq!(gathered_names(&reg), ["g_b"]);

        // A bare level prefixes every pattern: a full temporary override.
        set_runtime(&reg, "trace");
        assert_eq!(gathered_names(&reg), ["g_a", "g_b"]);

        reset_runtime(&reg);
        assert_eq!(gathered_names(&reg), ["g_b"]);
    }

    #[test]
    fn runtime_directives_replace_the_startup_directives_they_shadow() {
        use MetricLevel::{Debug, Trace, Warn};

        // Same pattern: the override directive replaces the startup one.
        let filter = Filter::parse("g_a=off,g_b=warn");
        filter
            .set_runtime_filter(FilterSource::new("g_b=trace"))
            .unwrap();
        assert!(!filter.is_exposed("g_a", "m", Warn));
        assert!(filter.is_exposed("g_b", "m", Trace));
        assert_eq!(filter.filter_string(), "g_a=off,g_b=trace");
        // The startup filter is untouched, ready for reset.
        assert_eq!(filter.startup_filter_string(), "g_a=off,g_b=warn");

        // Pattern prefix: an override directive claims its whole subtree, so
        // a more specific startup directive cannot poke through it.
        let filter = Filter::parse("iota_core=warn,iota_core::authority::sub=off");
        filter
            .set_runtime_filter(FilterSource::new("iota_core::authority=trace"))
            .unwrap();
        assert!(filter.is_exposed("x", "iota_core::authority::sub", Trace));
        assert!(!filter.is_exposed("x", "iota_core::checkpoints", Debug));
        assert_eq!(
            filter.filter_string(),
            "iota_core=warn,iota_core::authority=trace"
        );

        // A bare level shadows everything; more specific directives in the
        // same override still apply on top of it.
        let filter = Filter::parse("g_a=off,g_b=warn");
        filter
            .set_runtime_filter(FilterSource::new("trace,g_c=off"))
            .unwrap();
        assert!(filter.is_exposed("g_a", "m", Trace));
        assert!(filter.is_exposed("g_b", "m", Trace));
        assert!(!filter.is_exposed("g_c", "m", Warn));
        assert_eq!(filter.filter_string(), "trace,g_c=off");

        // Reset restores the startup directives.
        filter.reset_runtime_filter();
        assert_eq!(filter.filter_string(), filter.startup_filter_string());
        assert!(!filter.is_exposed("g_a", "m", Warn));
    }

    #[test]
    fn filter_reports_startup_and_current_strings() {
        // Both directive sets keep the group-form display the caller
        // supplies, while matching uses the expanded directives.
        let filter = Filter::from_sources(
            FilterSource::with_display("iota_core::authority=off", "authority=off"),
            Some(FilterSource::with_display(
                "iota_core::checkpoints=warn",
                "checkpoints=warn",
            )),
        );
        assert_eq!(
            filter.startup_filter_string(),
            "authority=off,checkpoints=warn"
        );
        assert_eq!(filter.filter_string(), filter.startup_filter_string());
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));

        // An override's display shadows the startup display the same way its
        // directives shadow the startup directives.
        filter
            .set_runtime_filter(FilterSource::with_display(
                "iota_core::authority=warn",
                "authority=warn",
            ))
            .unwrap();
        assert_eq!(filter.filter_string(), "checkpoints=warn,authority=warn");
        assert!(filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Debug));

        filter.reset_runtime_filter();
        assert_eq!(filter.filter_string(), filter.startup_filter_string());
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));
    }

    #[test]
    fn filter_string_is_canonical_and_round_trips() {
        // Invalid startup directives are dropped, and the reported startup
        // string reflects the directives actually in effect — so it can
        // always be POSTed back through the strict runtime setter.
        let filter = Filter::parse("foo=bogus, typed_store=warn ,info");
        let startup = filter.startup_filter_string();
        assert_eq!(startup, "typed_store=warn,info");
        filter
            .set_runtime_filter(FilterSource::new(&startup))
            .unwrap();
        assert_eq!(filter.filter_string(), "typed_store=warn,info");
    }

    #[test]
    fn set_runtime_filter_rejects_invalid_directives() {
        let filter = Filter::parse("authority=off");
        let err = filter
            .set_runtime_filter(FilterSource::new("authority=warn,bogus=nope"))
            .unwrap_err();
        assert!(err.contains("bogus=nope"), "unexpected error: {err}");
        // The failed update leaves the startup directives in effect.
        assert_eq!(filter.filter_string(), filter.startup_filter_string());
        assert!(!filter.is_exposed("x", "iota_core::authority", MetricLevel::Warn));
    }
}
