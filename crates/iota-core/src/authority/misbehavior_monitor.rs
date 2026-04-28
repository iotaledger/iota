// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::ArcSwap;
use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{LegacyReportPayload, VersionedMisbehaviorReport};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

/// A single misbehavior category tracked by the monitor.
///
/// This enum is **append-only**: once a variant is added it must never be
/// removed or reordered, because existing encoded data (e.g.,
/// `LegacyReportPayload`) relies on stable positional indices. New variants
/// must be introduced via a new `ReportedMisbehaviors` version.
#[derive(PartialEq)]
pub enum Misbehaviors {
    FaultyBlocksProvable,
    FaultyBlocksUnprovable,
    MissingProposals,
    Equivocations,
}

pub enum ReportedMisbehaviors {
    V1(Vec<Misbehaviors>),
}

impl ReportedMisbehaviors {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Misbehaviors> {
        match self {
            Self::V1(misbehaviors) => misbehaviors.iter(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MisbehaviorCounts(pub(crate) Vec<Vec<u64>>);

impl MisbehaviorCounts {
    pub(crate) fn new(reported_misbehaviors: &ReportedMisbehaviors, committee_size: usize) -> Self {
        Self(
            reported_misbehaviors
                .iter()
                .map(|_| vec![0u64; committee_size])
                .collect(),
        )
    }

    /// Converts the local counts into the wire/storage representation that is
    /// broadcast to peers as a `MisbehaviorReport` transaction.
    pub fn to_report_v1(&self) -> VersionedMisbehaviorReport {
        let payload = LegacyReportPayload {
            faulty_blocks_provable: self.0[0].clone(),
            faulty_blocks_unprovable: self.0[1].clone(),
            missing_proposals: self.0[2].clone(),
            equivocations: self.0[3].clone(),
        };
        VersionedMisbehaviorReport::V1(payload, OnceCell::new())
    }
}

impl From<&VersionedMisbehaviorReport> for MisbehaviorCounts {
    fn from(report: &VersionedMisbehaviorReport) -> Self {
        match report {
            VersionedMisbehaviorReport::V1(payload, _) => Self(vec![
                payload.faulty_blocks_provable.clone(),
                payload.faulty_blocks_unprovable.clone(),
                payload.missing_proposals.clone(),
                payload.equivocations.clone(),
            ]),
        }
    }
}

pub(crate) enum MisbehaviorMonitorVersion {
    V1(ReportedMisbehaviors),
}

impl MisbehaviorMonitorVersion {
    pub(crate) fn load_from_configs(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.misbehavior_monitor_version_as_option() {
            None | Some(1) => Self::V1(ReportedMisbehaviors::V1(vec![
                Misbehaviors::FaultyBlocksProvable,
                Misbehaviors::FaultyBlocksUnprovable,
                Misbehaviors::MissingProposals,
                Misbehaviors::Equivocations,
            ])),
            Some(version) => panic!("Unsupported misbehavior monitor version {version}"),
        }
    }

    pub(crate) fn reported_misbehaviors(&self) -> &ReportedMisbehaviors {
        match self {
            Self::V1(reported_misbehaviors) => reported_misbehaviors,
        }
    }
}

/// Holds all information related to scoring of authorities in the committee.
pub struct MisbehaviorMonitor {
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    pub(crate) current_local_metrics_count: ArcSwap<MisbehaviorCounts>,
    pub(crate) version: MisbehaviorMonitorVersion,
}

impl MisbehaviorMonitor {
    pub fn new(protocol_config: &ProtocolConfig, committee_size: usize) -> Self {
        // Local metrics count are always initialized as zero.
        let version = MisbehaviorMonitorVersion::load_from_configs(protocol_config);

        let current_local_metrics_count = ArcSwap::new(Arc::new(MisbehaviorCounts::new(
            version.reported_misbehaviors(),
            committee_size,
        )));

        Self {
            current_local_metrics_count,
            version,
        }
    }

    pub fn generate_report(&self) -> VersionedMisbehaviorReport {
        match &self.version {
            MisbehaviorMonitorVersion::V1(_) => {
                self.current_local_metrics_count.load().to_report_v1()
            }
        }
    }
}
