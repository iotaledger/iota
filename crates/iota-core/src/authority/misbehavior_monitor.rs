// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;

use crate::authority::authority_per_epoch_store::scorer::NodeVersionedScoringMetrics;

/// Holds all information related to scoring of authorities in the committee.
pub struct MisbehaviorMonitor {
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    pub(crate) current_local_metrics_count: NodeVersionedScoringMetrics,
}

impl MisbehaviorMonitor {
    pub fn new(protocol_config: &ProtocolConfig, committee_size: usize) -> Self {
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                // Local metrics count are always initialized as zero.
                let current_local_metrics_count =
                    NodeVersionedScoringMetrics::new(committee_size, protocol_config);

                Self {
                    current_local_metrics_count,
                }
            }
            _ => panic!("Unsupported scorer version"),
        }
    }
}
