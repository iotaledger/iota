// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Grafana Flame Graph panel compatible data structures and traits.

use std::time::Duration;

use serde::Serialize;

use super::{
    callgraph::{CallGraph, FrameLabel, NodeId},
    flame::{Flames, Graph, GraphId, Metadata},
    metric::{FlameMetric, MergeMetrics, SpanMetrics},
};

/// Frame in nested set form suitable for Grafana Flame Graph panel.
/// The type is mainly used with serde to serialize into axum::Json.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct NestedSetFrame<N = f64> {
    #[serde(serialize_with = "serialize_label")]
    pub label: FrameLabel,
    pub level: u32,
    pub value: N,
    #[serde(rename = "self")]
    pub self_: N,
}

fn serialize_label<S: serde::Serializer>(label: &FrameLabel, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(label.name)
}

pub trait NestedSetCollector<N = f64> {
    fn total(&self) -> N;
    fn collect_nested_set(&self) -> Vec<NestedSetFrame<N>>;
}

pub trait Dashboard {
    fn list_nested_sets(&self) -> Vec<(GraphId, f64)>;
    fn get_nested_set(&self, graph_id: &str, running: bool, completed: bool)
    -> Vec<NestedSetFrame>;
    fn get_nested_sets(
        &self,
        label: &'static str,
        running: bool,
        completed: bool,
    ) -> Vec<NestedSetFrame>;
}

trait FromDuration: Default + Sized {
    fn from_duration(total: Duration) -> Self;
}

impl FromDuration for f64 {
    fn from_duration(total: Duration) -> Self {
        // Grafana nested set model mandates the use of ms.
        // We preserve fractional part for f64.
        total.as_nanos() as f64 / 1_000_000.0
    }
}

impl FromDuration for u64 {
    fn from_duration(total: Duration) -> Self {
        total.as_millis() as u64
    }
}

impl CallGraph<FlameMetric> {
    fn nested_set_frame<N: FromDuration>(&self, ix: NodeId, level: u32) -> NestedSetFrame<N> {
        let node = &self.graph[ix];
        let value = N::from_duration(node.value.metrics.running.total);

        // "self" duration is recomputed: self total duration - sum of children total
        // durations
        let children_total = node
            .children
            .iter()
            .copied()
            .map(|ix| self.graph[ix].value.metrics.running.total)
            .sum::<Duration>();
        assert!(node.value.metrics.running.total >= children_total);

        let self_ = N::from_duration(node.value.metrics.running.total - children_total);
        NestedSetFrame {
            label: node.value.label,
            level,
            value,
            self_,
        }
    }
}

impl<N: FromDuration> NestedSetCollector<N> for CallGraph<FlameMetric> {
    fn total(&self) -> N {
        N::from_duration(
            self.root()
                .map(|frame| frame.metrics.running.total)
                .unwrap_or_default(),
        )
    }

    fn collect_nested_set(&self) -> Vec<NestedSetFrame<N>> {
        self.graph.dfs_fold(Vec::new(), |frames, node_id, level| {
            frames.push(self.nested_set_frame(node_id, level as u32))
        })
    }
}

impl<S> Dashboard for Flames<S>
where
    S: Clone + Default + MergeMetrics + SpanMetrics,
    CallGraph<S>: NestedSetCollector,
{
    fn list_nested_sets(&self) -> Vec<(GraphId, f64)> {
        let mut graph_ids: Vec<(GraphId, f64)> = self
            .graphs
            .read()
            .values()
            .map(|Graph { graph_id, mutex }| (*graph_id, mutex.lock().total()))
            .collect();

        graph_ids.extend(
            self.completed
                .read()
                .iter()
                .map(|(graph_id, graph)| (*graph_id, graph.total())),
        );

        graph_ids.sort_by(|(_, m), (_, n)| n.total_cmp(m));
        graph_ids
    }

    fn get_nested_set(
        &self,
        graph_id: &str,
        running: bool,
        completed: bool,
    ) -> Vec<NestedSetFrame> {
        self.get_callgraph(&Metadata::from(graph_id), running, completed)
            .map(|graph| graph.collect_nested_set())
            .unwrap_or_default()
    }

    fn get_nested_sets(
        &self,
        label: &'static str,
        running: bool,
        completed: bool,
    ) -> Vec<NestedSetFrame> {
        let callgraphs = self.get_callgraphs(running, completed);
        let total = callgraphs.values().map(|graph| graph.total()).sum::<f64>();

        std::iter::once(NestedSetFrame {
            label: label.into(),
            level: 0,
            value: total,
            self_: 0.0,
        })
        .chain(
            callgraphs
                .into_iter()
                .flat_map(|(_, flame)| flame.collect_nested_set().into_iter())
                .map(|mut frame| {
                    frame.level += 1;
                    frame
                }),
        )
        .collect()
    }
}
