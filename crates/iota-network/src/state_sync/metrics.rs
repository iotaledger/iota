// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus_filtered::{
    Histogram, IntCounter, IntGauge, MetricLevel, Registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};
use tap::Pipe;

#[derive(Clone)]
pub(super) struct Metrics(Option<Arc<Inner>>);

impl std::fmt::Debug for Metrics {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Metrics").finish()
    }
}

impl Metrics {
    pub fn enabled(registry: &Registry) -> Self {
        Metrics(Some(Inner::new(registry)))
    }

    pub fn disabled() -> Self {
        Metrics(None)
    }

    pub fn set_highest_known_checkpoint(&self, sequence_number: CheckpointSequenceNumber) {
        if let Some(inner) = &self.0 {
            inner.highest_known_checkpoint.set(sequence_number as i64);
        }
    }

    pub fn set_highest_verified_checkpoint(&self, sequence_number: CheckpointSequenceNumber) {
        if let Some(inner) = &self.0 {
            inner
                .highest_verified_checkpoint
                .set(sequence_number as i64);
        }
    }

    pub fn set_highest_synced_checkpoint(&self, sequence_number: CheckpointSequenceNumber) {
        if let Some(inner) = &self.0 {
            inner.highest_synced_checkpoint.set(sequence_number as i64);
        }
    }

    /// Counts a checkpoint from the archive whose results were not applied, so
    /// its transactions fall back to being executed. Expected around epoch
    /// boundaries the node has not reached yet; a steadily rising count means
    /// applying is not taking effect.
    pub fn checkpoint_from_archive_left_to_the_executor(&self) {
        if let Some(inner) = &self.0 {
            inner.checkpoints_from_archive_left_to_the_executor.inc();
        }
    }

    pub fn update_checkpoints_synced_from_checkpoint_archive(&self) {
        if let Some(inner) = &self.0 {
            inner.checkpoints_synced_from_checkpoint_archive.inc();
        }
    }

    pub fn checkpoint_summary_age_metrics(&self) -> Option<&Histogram> {
        if let Some(inner) = &self.0 {
            return Some(&inner.checkpoint_summary_age);
        }
        None
    }
}

struct Inner {
    highest_known_checkpoint: IntGauge,
    highest_verified_checkpoint: IntGauge,
    highest_synced_checkpoint: IntGauge,
    checkpoints_synced_from_checkpoint_archive: IntCounter,
    checkpoints_from_archive_left_to_the_executor: IntCounter,
    checkpoint_summary_age: Histogram,
}

impl Inner {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Self {
            highest_known_checkpoint: register_int_gauge_with_registry!(
                "highest_known_checkpoint",
                "Highest known checkpoint",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),

            highest_verified_checkpoint: register_int_gauge_with_registry!(
                "highest_verified_checkpoint",
                "Highest verified checkpoint",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),

            highest_synced_checkpoint: register_int_gauge_with_registry!(
                "highest_synced_checkpoint",
                "Highest synced checkpoint",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),

            checkpoints_synced_from_checkpoint_archive: register_int_counter_with_registry!(
                "checkpoints_synced_from_checkpoint_archive",
                "Checkpoints synced from checkpoint archive",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),

            checkpoints_from_archive_left_to_the_executor: register_int_counter_with_registry!(
                "checkpoints_from_archive_left_to_the_executor",
                "Checkpoints from the archive whose results were not applied, leaving their \
                 transactions to be executed",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),

            checkpoint_summary_age: register_histogram_with_registry!(
                "checkpoint_summary_age",
                "Age of checkpoints summaries when they arrive and are verified.",
                iota_metrics::LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
        }
        .pipe(Arc::new)
    }
}
