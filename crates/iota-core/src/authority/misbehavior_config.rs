// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{LegacyReportPayload, VersionedMisbehaviorReport};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::consensus_types::consensus_output_api::ConsensusOutputMisbehaviors;

/// Single source of truth for the misbehavior schema and report wire format.
///
/// Each variant ties together (a) the version reported in `ProtocolConfig`,
/// (b) the ordered list of `Misbehaviors` tracked locally, and (c) the
/// `VersionedMisbehaviorReport` variant accepted from peers. Adding a new
/// variant forces the schema and the acceptance check to be updated in one
/// place; the compiler enforces that every match is exhaustive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MisbehaviorSchemaVersion {
    V1,
}

impl MisbehaviorSchemaVersion {
    pub fn from_protocol(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.misbehavior_monitor_version_as_option() {
            None | Some(1) => Self::V1,
            Some(version) => panic!("Unsupported misbehavior schema version {version}"),
        }
    }

    /// Ordered list of misbehavior categories tracked under this version. The
    /// index of each variant determines its row in `MisbehaviorCounts`.
    pub fn reported_misbehaviors(&self) -> &'static [Misbehaviors] {
        match self {
            Self::V1 => &[
                Misbehaviors::FaultyBlocksProvable,
                Misbehaviors::FaultyBlocksUnprovable,
                Misbehaviors::MissingProposals,
                Misbehaviors::Equivocations,
            ],
        }
    }

    pub fn num_metrics(&self) -> usize {
        self.reported_misbehaviors().len()
    }

    /// Returns `true` if the given report's wire format matches this version.
    pub fn accepts_report(&self, report: &VersionedMisbehaviorReport) -> bool {
        match self {
            Self::V1 => matches!(report, VersionedMisbehaviorReport::V1(..)),
        }
    }
}

/// A single misbehavior category tracked by the monitor.
///
/// This enum is **append-only**: once a variant is added it must never be
/// removed or reordered, because existing encoded data (e.g.,
/// `LegacyReportPayload`) relies on stable positional indices. New variants
/// must be introduced via a new `MisbehaviorSchemaVersion`.
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

/// A two-dimensional matrix of raw misbehavior counts.
///
/// `MisbehaviorCounts[i][j]` holds the count of misbehavior category `i`
/// (indexed by the schema's `reported_misbehaviors`) observed for authority
/// `j`. The inner dimension equals the committee size; the outer equals the
/// number of tracked misbehavior categories.
///
/// This is the domain type used inside `iota-core`. For wire/storage encoding
/// see `LegacyReportPayload` / `VersionedMisbehaviorReport` in `iota-types`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MisbehaviorCounts(pub(crate) Vec<Vec<u64>>);

impl MisbehaviorCounts {
    pub(crate) fn new(reported_misbehaviors: &[Misbehaviors], committee_size: usize) -> Self {
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
        reported_misbehaviors: &[Misbehaviors],
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
    ///
    /// Rows are matched to wire fields by `Misbehaviors` variant rather than
    /// positional index, so reordering the schema cannot silently swap
    /// categories — adding/removing a variant is caught at compile time.
    pub fn to_report(&self, version: MisbehaviorSchemaVersion) -> VersionedMisbehaviorReport {
        match version {
            MisbehaviorSchemaVersion::V1 => {
                let mut faulty_blocks_provable = Vec::new();
                let mut faulty_blocks_unprovable = Vec::new();
                let mut missing_proposals = Vec::new();
                let mut equivocations = Vec::new();
                for (row, misbehavior) in self.0.iter().zip(version.reported_misbehaviors().iter())
                {
                    match misbehavior {
                        Misbehaviors::FaultyBlocksProvable => faulty_blocks_provable = row.clone(),
                        Misbehaviors::FaultyBlocksUnprovable => {
                            faulty_blocks_unprovable = row.clone()
                        }
                        Misbehaviors::MissingProposals => missing_proposals = row.clone(),
                        Misbehaviors::Equivocations => equivocations = row.clone(),
                    }
                }
                let payload = LegacyReportPayload {
                    faulty_blocks_provable,
                    faulty_blocks_unprovable,
                    missing_proposals,
                    equivocations,
                };
                VersionedMisbehaviorReport::V1(payload, OnceCell::new())
            }
        }
    }

    /// Builds `MisbehaviorCounts` from a peer's report. Row order follows the
    /// schema's `reported_misbehaviors`; the `match` on `Misbehaviors` enforces
    /// that every locally tracked variant maps to a known V1 wire field.
    pub fn from_report(
        report: &VersionedMisbehaviorReport,
        version: MisbehaviorSchemaVersion,
    ) -> Self {
        debug_assert!(
            version.accepts_report(report),
            "from_report called with a report whose wire-format version does not match the schema; \
             callers must validate via MisbehaviorSchemaVersion::accepts_report first"
        );
        match report {
            VersionedMisbehaviorReport::V1(payload, _) => Self(
                version
                    .reported_misbehaviors()
                    .iter()
                    .map(|misbehavior| match misbehavior {
                        Misbehaviors::FaultyBlocksProvable => {
                            payload.faulty_blocks_provable.clone()
                        }
                        Misbehaviors::FaultyBlocksUnprovable => {
                            payload.faulty_blocks_unprovable.clone()
                        }
                        Misbehaviors::MissingProposals => payload.missing_proposals.clone(),
                        Misbehaviors::Equivocations => payload.equivocations.clone(),
                    })
                    .collect(),
            ),
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
