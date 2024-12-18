// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_metrics::histogram::Histogram as IotaHistogram;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus::{
    Histogram, IntGauge, Registry, register_histogram_with_registry,
    register_int_gauge_with_registry,
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

    pub fn checkpoint_summary_age_metrics(&self) -> Option<(&Histogram, &IotaHistogram)> {
        if let Some(inner) = &self.0 {
            return Some((
                &inner.checkpoint_summary_age,
                &inner.checkpoint_summary_age_ms,
            ));
        }
        None
    }
}

struct Inner {
    highest_known_checkpoint: IntGauge,
    highest_verified_checkpoint: IntGauge,
    highest_synced_checkpoint: IntGauge,
    checkpoint_summary_age: Histogram,
    // TODO: delete once users are migrated to non-Iota histogram.
    checkpoint_summary_age_ms: IotaHistogram,
}

impl Inner {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Self {
            highest_known_checkpoint: register_int_gauge_with_registry!(
                "highest_known_checkpoint",
                "Highest known checkpoint",
                registry
            )
            .unwrap(),

            highest_verified_checkpoint: register_int_gauge_with_registry!(
                "highest_verified_checkpoint",
                "Highest verified checkpoint",
                registry
            )
            .unwrap(),

            highest_synced_checkpoint: register_int_gauge_with_registry!(
                "highest_synced_checkpoint",
                "Highest synced checkpoint",
                registry
            )
            .unwrap(),

            checkpoint_summary_age: register_histogram_with_registry!(
                "checkpoint_summary_age",
                "Age of checkpoints summaries when they arrive and are verified.",
                iota_metrics::LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            checkpoint_summary_age_ms: IotaHistogram::new_in_registry(
                "checkpoint_summary_age_ms",
                "Age of checkpoints summaries when they arrive and are verified.",
                registry,
            ),
        }
        .pipe(Arc::new)
    }
}
