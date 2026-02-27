// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(feature = "flamegraph-alloc", nightly))]
mod alloc;
mod arena;
mod callgraph;
mod flame;
mod grafana;
mod metric;
mod svg;
mod tracing;
#[cfg(all(feature = "flamegraph-alloc", nightly))]
pub use alloc::{AllocMetrics, CounterAlloc, get_alloc_metrics};

pub use grafana::NestedSetFrame;
pub use svg::{Config as SvgConfig, Svg};
pub use tracing::FlameSub;
