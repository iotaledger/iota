// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::ArcSwap;
use iota_types::messages_consensus::VersionedMisbehaviorReport;

use crate::{
    authority::authority_per_epoch_store::misbehavior_config::{
        MisbehaviorConfig, MisbehaviorCounts,
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
    config: MisbehaviorConfig,
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    current_local_counts: ArcSwap<MisbehaviorCounts>,
}

impl MisbehaviorMonitor {
    pub fn new(config: &MisbehaviorConfig, committee_size: usize) -> Self {
        let current_local_counts = ArcSwap::new(Arc::new(MisbehaviorCounts::new(
            config.reported_misbehaviors(),
            committee_size,
        )));

        Self {
            config: config.clone(),
            current_local_counts,
        }
    }

    pub fn generate_report(&self) -> VersionedMisbehaviorReport {
        self.current_local_counts
            .load()
            .to_report(self.config.version())
    }

    pub fn update_from_consensus_output(
        &self,
        output_misbehavior_counts: Vec<(ConsensusOutputMisbehaviors, Vec<u64>)>,
    ) {
        let new_counts = MisbehaviorCounts::from_consensus_output(
            output_misbehavior_counts,
            self.config.reported_misbehaviors(),
        );
        self.current_local_counts.store(Arc::new(new_counts));
    }
}
