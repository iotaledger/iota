// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{LegacyReportPayload, VersionedMisbehaviorReport};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::consensus_types::consensus_output_api::ConsensusOutputMisbehaviors;

/// Identifies which set of misbehavior categories and report format is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    V1,
}

/// Shared misbehavior schema loaded once from `ProtocolConfig` and passed to
/// all misbehavior-related components (`MisbehaviorMonitor`,
/// `ReportAggregator`, `Scorer`). This is the single source of truth for which
/// metrics are tracked.
#[derive(Clone)]
pub struct MisbehaviorConfig {
    version: Version,
    reported_misbehaviors: ReportedMisbehaviors,
}

impl MisbehaviorConfig {
    pub fn from_protocol(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.misbehavior_monitor_version_as_option() {
            None | Some(1) => Self {
                version: Version::V1,
                reported_misbehaviors: ReportedMisbehaviors(vec![
                    Misbehaviors::FaultyBlocksProvable,
                    Misbehaviors::FaultyBlocksUnprovable,
                    Misbehaviors::MissingProposals,
                    Misbehaviors::Equivocations,
                ]),
            },
            Some(version) => panic!("Unsupported misbehavior config version {version}"),
        }
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn reported_misbehaviors(&self) -> &ReportedMisbehaviors {
        &self.reported_misbehaviors
    }

    pub fn num_metrics(&self) -> usize {
        self.reported_misbehaviors.0.len()
    }

    /// Returns `true` if the given report's wire format matches the expected
    /// version.
    pub fn accepts_report(&self, report: &VersionedMisbehaviorReport) -> bool {
        match self.version {
            Version::V1 => matches!(report, VersionedMisbehaviorReport::V1(..)),
        }
    }
}

/// A single misbehavior category tracked by the monitor.
///
/// This enum is **append-only**: once a variant is added it must never be
/// removed or reordered, because existing encoded data (e.g.,
/// `LegacyReportPayload`) relies on stable positional indices. New variants
/// must be introduced via a new `ReportedMisbehaviors` version.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Misbehaviors {
    FaultyBlocksProvable,
    FaultyBlocksUnprovable,
    MissingProposals,
    Equivocations,
}

impl From<&ConsensusOutputMisbehaviors> for Misbehaviors {
    fn from(output_misbehavior: &ConsensusOutputMisbehaviors) -> Self {
        match output_misbehavior {
            ConsensusOutputMisbehaviors::FaultyBlocksProvable => Self::FaultyBlocksProvable,
            ConsensusOutputMisbehaviors::FaultyBlocksUnprovable => Self::FaultyBlocksUnprovable,
            ConsensusOutputMisbehaviors::MissingProposals => Self::MissingProposals,
            ConsensusOutputMisbehaviors::Equivocations => Self::Equivocations,
        }
    }
}

/// The versioned, ordered list of misbehavior categories actively tracked in
/// the current epoch. The index of each variant determines which row of
/// `MisbehaviorCounts` and which field of `ReportPayload` it corresponds to, so
/// order must remain stable within a version.
#[derive(Clone)]
pub struct ReportedMisbehaviors(Vec<Misbehaviors>);

impl ReportedMisbehaviors {
    pub fn iter(&self) -> impl Iterator<Item = &Misbehaviors> {
        self.0.iter()
    }
}

/// A two-dimensional matrix of raw misbehavior counts.
///
/// `MisbehaviorCounts[i][j]` holds the count of misbehavior category `i`
/// (indexed by `ReportedMisbehaviors`) observed for authority `j`.  The inner
/// dimension therefore equals the committee size and the outer dimension equals
/// the number of tracked misbehavior categories.
///
/// This is the domain type used inside `iota-core`. For wire/storage encoding
/// see `ReportPayload` / `VersionedMisbehaviorReport` in `iota-types`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MisbehaviorCounts(pub(crate) Vec<Vec<u64>>);

impl MisbehaviorCounts {
    pub(crate) fn new(reported_misbehaviors: &ReportedMisbehaviors, committee_size: usize) -> Self {
        // Local metrics count are always initialized as zero.
        Self(
            reported_misbehaviors
                .iter()
                .map(|_| vec![0u64; committee_size])
                .collect(),
        )
    }

    pub(crate) fn from_consensus_output(
        mut output_misbehavior_counts: Vec<(ConsensusOutputMisbehaviors, Vec<u64>)>,
        reported_misbehaviors: &ReportedMisbehaviors,
    ) -> Self {
        Self(
            reported_misbehaviors
                .iter()
                .map(|misbehavior| {
                    output_misbehavior_counts
                        .iter()
                        .position(|(output_misbehavior, _)| {
                            Misbehaviors::from(output_misbehavior) == *misbehavior
                        })
                        .map(|i| output_misbehavior_counts.swap_remove(i).1)
                        .unwrap_or_default()
                })
                .collect(),
        )
    }

    /// Converts the local counts into the wire/storage representation that is
    /// broadcast to peers as a `MisbehaviorReport` transaction.
    pub fn to_report(&self, version: Version) -> VersionedMisbehaviorReport {
        match version {
            Version::V1 => {
                let payload = LegacyReportPayload {
                    faulty_blocks_provable: self.0[0].clone(),
                    faulty_blocks_unprovable: self.0[1].clone(),
                    missing_proposals: self.0[2].clone(),
                    equivocations: self.0[3].clone(),
                };
                VersionedMisbehaviorReport::V1(payload, OnceCell::new())
            }
        }
    }

    pub(crate) fn get_value(&self, metric_index: usize, authority: usize) -> u64 {
        self.0[metric_index][authority]
    }

    /// Returns a new `MisbehaviorCounts` where each cell is the element-wise
    /// maximum of `self` and `other`. This implements a monotone update:
    /// counts can only increase, so a later report never reduces a previously
    /// observed count.
    pub fn merge_max(&self, other: &MisbehaviorCounts) -> Self {
        let updated: Vec<Vec<u64>> = self
            .0
            .iter()
            .zip(other.0.iter())
            .map(|(current, incoming)| {
                current
                    .iter()
                    .zip(incoming.iter())
                    .map(|(c, i)| *c.max(i))
                    .collect()
            })
            .collect();
        Self(updated)
    }

    pub fn get_metric(&self, index: usize) -> &[u64] {
        &self.0[index]
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

pub fn verify_legacy_payload(legacy_payload: &LegacyReportPayload, committee_size: usize) -> bool {
    // This version of reports are valid as long as they contain the counts for all
    // authorities. Future versions may contain proofs that need verification.
    // However, since the validity of a proof is deeply coupled with the protocol
    // version and the consensus mechanism being used, we cannot verify it here. In
    // the future, reports should be unwrapped (or translated) to a type verifiable
    // by the consensus crate, which means that the verification logic will probably
    // move out of this crate.
    if (legacy_payload.faulty_blocks_provable.len() != committee_size)
        | (legacy_payload.faulty_blocks_unprovable.len() != committee_size)
        | (legacy_payload.equivocations.len() != committee_size)
        | (legacy_payload.missing_proposals.len() != committee_size)
    {
        return false;
    }
    true
}
