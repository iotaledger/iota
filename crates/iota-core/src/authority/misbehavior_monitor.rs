// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_types::scoring_metrics::VersionedScoringMetrics;

/// Holds all information related to scoring of authorities in the committee.
pub struct MisbehaviorMonitor {
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    pub(crate) current_local_metrics_count: Arc<VersionedScoringMetrics>,
}

impl MisbehaviorMonitor {
    pub fn new(voting_power: Vec<u64>, protocol_config: &ProtocolConfig) -> Self {
        let committee_size = voting_power.len();
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                // Local metrics count are always initialized as zero.
                let current_local_metrics_count = Arc::new(VersionedScoringMetrics::new(
                    committee_size,
                    protocol_config,
                ));

                Self {
                    current_local_metrics_count,
                }
            }
            _ => panic!("Unsupported scorer version"),
        }
    }
}
