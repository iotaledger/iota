// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reads the values of a registry, for the tests of the crates that register
//! metrics.

use std::collections::BTreeSet;

use prometheus_filtered::{Registry, prometheus::proto::Metric};

/// A histogram series: the sample count and sum.
#[derive(Debug, Default)]
pub struct HistogramTotals {
    pub count: u64,
    pub sum: f64,
}

/// Reads series of a registry by name and labels. With a prefix, `name` means
/// `{prefix}_{name}`.
#[derive(Clone)]
pub struct Reader {
    registry: Registry,
    prefix: Option<String>,
}

impl Reader {
    /// A reader of `registry`, with no prefix.
    pub fn new(registry: &Registry) -> Self {
        Self {
            registry: registry.clone(),
            prefix: None,
        }
    }

    /// Makes each name that this reader gets mean `{prefix}_{name}`.
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(prefix.to_owned());
        self
    }

    fn full_name(&self, name: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}_{name}"),
            None => name.to_owned(),
        }
    }

    /// The series of the family `name` whose labels include `labels`.
    fn series(&self, name: &str, labels: &[(&str, &str)]) -> Vec<Metric> {
        let name = self.full_name(name);
        self.registry
            .gather()
            .into_iter()
            .filter(|family| family.name() == name)
            .flat_map(|family| family.get_metric().to_vec())
            .filter(|metric| {
                labels.iter().all(|(key, value)| {
                    metric
                        .get_label()
                        .iter()
                        .any(|label| label.name() == *key && label.value() == *value)
                })
            })
            .collect()
    }

    /// The names of all families the registry exposes, as they are.
    pub fn family_names(&self) -> BTreeSet<String> {
        self.registry
            .gather()
            .into_iter()
            .map(|family| family.name().to_owned())
            .collect()
    }

    /// Whether the registry exposes a family `name`.
    pub fn has_family(&self, name: &str) -> bool {
        let name = self.full_name(name);
        self.registry
            .gather()
            .iter()
            .any(|family| family.name() == name)
    }

    /// The sum of the counters or gauges whose labels include `labels`. Zero
    /// when no series matches.
    pub fn value(&self, name: &str, labels: &[(&str, &str)]) -> f64 {
        self.series(name, labels)
            .iter()
            .map(|metric| {
                if metric.counter.is_some() {
                    metric.get_counter().value()
                } else {
                    metric.get_gauge().value()
                }
            })
            .sum()
    }

    /// The sum of the histograms whose labels include `labels`. Empty when no
    /// series matches.
    pub fn histogram_totals(&self, name: &str, labels: &[(&str, &str)]) -> HistogramTotals {
        let mut total = HistogramTotals::default();
        for metric in self.series(name, labels) {
            let histogram = metric.get_histogram();
            total.count += histogram.sample_count();
            total.sum += histogram.sample_sum();
        }
        total
    }
}
