// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use arc_swap::ArcSwap;
use iota_types::{
    base_types::AuthorityName,
    messages_consensus::{MisbehaviorObservations, VersionedMisbehaviorReport},
};

use crate::{
    authority::authority_per_epoch_store::misbehavior::{
        MisbehaviorReportVersion, merge_max, observations_from_consensus_output, zero_observations,
    },
    consensus_types::consensus_output_api::ConsensusOutputMisbehaviorCounts,
};

/// Tracks local misbehavior observations for all authorities in the committee
/// and manages outgoing report generation and rate-limiting.
///
/// The monitor accumulates counts from blocks produced by consensus and exposes
/// them as `MisbehaviorReport` transactions submitted to consensus at
/// checkpoint boundaries.
pub struct MisbehaviorMonitor {
    authority: AuthorityName,
    report_version: MisbehaviorReportVersion,
    committee_size: usize,
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    current_local_observations: ArcSwap<MisbehaviorObservations>,
    // Single-writer: the three rate-limit fields below are only mutated by
    // SubmitCheckpointToConsensus::checkpoint_created. Don't add additional writers without
    // revisiting the atomicity story — `last_report_summary` and `last_report_checkpoint_seq`
    // form a logical tuple but are stored as independent Relaxed atomics, safe today only
    // because reads and writes happen from the single CheckpointBuilder task.
    // `has_sent_end_of_epoch_report` is an independent epoch-once flag.
    //
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
    pub fn new(
        authority: AuthorityName,
        report_version: MisbehaviorReportVersion,
        committee_size: usize,
    ) -> Self {
        let current_local_observations =
            ArcSwap::new(Arc::new(zero_observations(report_version, committee_size)));

        Self {
            authority,
            report_version,
            committee_size,
            current_local_observations,
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

    pub fn generate_report(&self, generation: u64) -> VersionedMisbehaviorReport {
        match self.current_local_observations.load().as_ref() {
            MisbehaviorObservations::V1(o) => {
                VersionedMisbehaviorReport::new_v1(self.authority, generation, o.clone())
            }
        }
    }

    pub fn update_from_consensus_output(&self, counts: ConsensusOutputMisbehaviorCounts) {
        let new_counts =
            observations_from_consensus_output(counts, self.report_version, self.committee_size);
        // Defensive merge: counts reported within an epoch are expected to be
        // monotonic, but folding in via element-wise max guarantees the local
        // view never goes backwards even if upstream produces a transient dip.
        // RCU keeps the load+merge+store atomic against concurrent updaters.
        self.current_local_observations
            .rcu(|current| Arc::new(merge_max(current, &new_counts)));
    }
}
