// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use arc_swap::ArcSwap;
use iota_types::messages_consensus::VersionedMisbehaviorReport;

use crate::{
    authority::authority_per_epoch_store::misbehavior_config::{
        MisbehaviorCounts, MisbehaviorSchemaVersion,
    },
    consensus_types::consensus_output_api::ConsensusOutputMisbehaviors,
};

/// Tracks local misbehavior observations for all authorities in the committee
/// and manages outgoing report generation and rate-limiting.
///
/// The monitor accumulates counts from blocks produced by consensus and exposes
/// them as `MisbehaviorReport` transactions submitted to consensus at
/// checkpoint boundaries.
pub struct MisbehaviorMonitor {
    schema_version: MisbehaviorSchemaVersion,
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    current_local_counts: ArcSwap<MisbehaviorCounts>,
    // Summary of the last MisbehaviorReport this node submitted, defined as the sum of all
    // metrics across authorities. Since reported counts are monotonically non-decreasing within
    // an epoch, the summary is also monotonic. Used to skip submitting reports when nothing has
    // changed since the last submission (rate limiting).
    last_report_summary: AtomicU64,
    // Sequence number of the last checkpoint at which this node submitted a report. Used together
    // with `MIN_CHECKPOINTS_BETWEEN_REPORTS` to rate-limit submissions.
    last_report_checkpoint_seq: AtomicU64,
    // Whether this node has already submitted a report close to the epoch end. Ensures the
    // end-of-epoch report is sent at most once per epoch.
    has_sent_end_of_epoch_report: AtomicBool,
}

impl MisbehaviorMonitor {
    pub fn new(schema_version: MisbehaviorSchemaVersion, committee_size: usize) -> Self {
        let current_local_counts = ArcSwap::new(Arc::new(MisbehaviorCounts::new(
            schema_version.reported_misbehaviors(),
            committee_size,
        )));

        Self {
            schema_version,
            current_local_counts,
            last_report_summary: AtomicU64::new(0),
            last_report_checkpoint_seq: AtomicU64::new(0),
            has_sent_end_of_epoch_report: AtomicBool::new(false),
        }
    }

    pub(crate) fn last_report_summary(&self) -> u64 {
        self.last_report_summary.load(Ordering::Relaxed)
    }

    pub(crate) fn store_last_report_summary(&self, summary: u64) {
        self.last_report_summary.store(summary, Ordering::Relaxed)
    }

    pub(crate) fn last_report_checkpoint_seq(&self) -> u64 {
        self.last_report_checkpoint_seq.load(Ordering::Relaxed)
    }

    pub(crate) fn store_last_report_checkpoint_seq(&self, seq: u64) {
        self.last_report_checkpoint_seq
            .store(seq, Ordering::Relaxed)
    }

    pub(crate) fn has_sent_end_of_epoch_report(&self) -> bool {
        self.has_sent_end_of_epoch_report.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_end_of_epoch_report_sent(&self) {
        self.has_sent_end_of_epoch_report
            .store(true, Ordering::Relaxed);
    }

    pub fn generate_report(&self) -> VersionedMisbehaviorReport {
        self.current_local_counts
            .load()
            .to_report(self.schema_version)
    }

    pub fn update_from_consensus_output(
        &self,
        output_misbehavior_counts: Vec<(ConsensusOutputMisbehaviors, Vec<u64>)>,
    ) {
        let new_counts = MisbehaviorCounts::from_consensus_output(
            output_misbehavior_counts,
            self.schema_version.reported_misbehaviors(),
        );
        self.current_local_counts.store(Arc::new(new_counts));
    }
}
