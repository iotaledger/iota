// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::ArcSwap;
use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{LegacyReportPayload, VersionedMisbehaviorReport};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MisbehaviorCounts(pub(crate) Vec<Vec<u64>>);

impl MisbehaviorCounts {
    pub fn to_report(&self) -> VersionedMisbehaviorReport {
        VersionedMisbehaviorReport::new_v1(LegacyReportPayload {
            faulty_blocks_provable: self.0[0].clone(),
            faulty_blocks_unprovable: self.0[1].clone(),
            missing_proposals: self.0[2].clone(),
            equivocations: self.0[3].clone(),
        })
    }
}

/// Holds all information related to scoring of authorities in the committee.
pub struct MisbehaviorMonitor {
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    pub(crate) current_local_metrics_count: ArcSwap<MisbehaviorCounts>,
}

impl MisbehaviorMonitor {
    pub fn new(protocol_config: &ProtocolConfig, committee_size: usize) -> Self {
        let current_local_metrics_count = ArcSwap::new(Arc::new(MisbehaviorCounts(vec![
                vec![0; committee_size];
                4
            ])));

        Self {
            current_local_metrics_count,
        }
    }
}
