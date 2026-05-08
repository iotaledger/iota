// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

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
    current_local_observations: ArcSwap<MisbehaviorObservations>,
    rate_limit: Mutex<ReportRateLimitState>,
}

/// Mutable state used to rate-limit outgoing misbehavior reports. Grouped
/// behind a single `Mutex` so reads and writes are always consistent.
pub(crate) struct ReportRateLimitState {
    /// Summary of the last submitted report (sum of all metrics across
    /// authorities). Monotonic within an epoch — used to skip submissions
    /// when nothing has changed.
    pub last_report_summary: u64,
    /// Checkpoint sequence number at which the last report was submitted.
    /// Used with `MIN_CHECKPOINTS_BETWEEN_REPORTS` for rate-limiting.
    pub last_report_checkpoint_seq: u64,
    /// Whether the end-of-epoch report has already been sent (at most once
    /// per epoch).
    pub has_sent_end_of_epoch_report: bool,
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
            rate_limit: Mutex::new(ReportRateLimitState {
                last_report_summary: 0,
                last_report_checkpoint_seq: 0,
                has_sent_end_of_epoch_report: false,
            }),
        }
    }

    pub(crate) fn rate_limit(&self) -> std::sync::MutexGuard<'_, ReportRateLimitState> {
        self.rate_limit
            .lock()
            .expect("rate limit lock should not be poisoned")
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
