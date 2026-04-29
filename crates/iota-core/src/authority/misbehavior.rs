// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{ReportPayload, ReportPayloadV1, VersionedMisbehaviorReport};
use serde::{Deserialize, Serialize};

use crate::consensus_types::consensus_output_api::ConsensusOutputMisbehavior;

/// Single source of truth for the misbehavior schema and report wire format.
///
/// Each variant ties together (a) the version reported in `ProtocolConfig`,
/// (b) the ordered list of `Misbehavior` tracked locally, and (c) the
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
    pub fn reported_misbehaviors(&self) -> &'static [Misbehavior] {
        match self {
            Self::V1 => &[
                Misbehavior::FaultyBlocksProvable,
                Misbehavior::FaultyBlocksUnprovable,
                Misbehavior::MissingProposals,
                Misbehavior::Equivocations,
            ],
        }
    }

    pub fn num_metrics(&self) -> usize {
        self.reported_misbehaviors().len()
    }

    /// Returns `true` if the given report's wire format matches this version.
    pub fn accepts_report(&self, report: &VersionedMisbehaviorReport) -> bool {
        match self {
            Self::V1 => matches!(report.payload, ReportPayload::V1(_)),
        }
    }
}

/// A single misbehavior category tracked by the monitor.
///
/// This enum is **append-only**: once a variant is added it must never be
/// removed or reordered, because existing encoded data (e.g.,
/// `ReportPayloadV1`) relies on stable positional indices. New variants
/// must be introduced via a new `MisbehaviorSchemaVersion`.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Misbehavior {
    FaultyBlocksProvable,
    FaultyBlocksUnprovable,
    MissingProposals,
    Equivocations,
}

impl From<&ConsensusOutputMisbehavior> for Misbehavior {
    fn from(output_misbehavior: &ConsensusOutputMisbehavior) -> Self {
        match output_misbehavior {
            ConsensusOutputMisbehavior::FaultyBlocksProvable => Self::FaultyBlocksProvable,
            ConsensusOutputMisbehavior::FaultyBlocksUnprovable => Self::FaultyBlocksUnprovable,
            ConsensusOutputMisbehavior::MissingProposals => Self::MissingProposals,
            ConsensusOutputMisbehavior::Equivocations => Self::Equivocations,
        }
    }
}

/// In-memory representation of a misbehavior report's payload.
///
/// Tagged by schema version so each version's representation can be a
/// dedicated named-field struct — the compiler enforces that every operation
/// touching all metrics handles every category. Field order in each
/// `MisbehaviorCountsVN` mirrors the corresponding wire format
/// (`ReportPayloadVN`) for human readability but is not load-bearing here;
/// the wire-format struct's field order, however, *is* part of the protocol.
///
/// For wire/storage encoding see `VersionedMisbehaviorReport` in `iota-types`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MisbehaviorCounts {
    V1(MisbehaviorCountsV1),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MisbehaviorCountsV1 {
    pub faulty_blocks_provable: Vec<u64>,
    pub faulty_blocks_unprovable: Vec<u64>,
    pub missing_proposals: Vec<u64>,
    pub equivocations: Vec<u64>,
}

impl MisbehaviorCountsV1 {
    pub fn zeros(committee_size: usize) -> Self {
        Self {
            faulty_blocks_provable: vec![0u64; committee_size],
            faulty_blocks_unprovable: vec![0u64; committee_size],
            missing_proposals: vec![0u64; committee_size],
            equivocations: vec![0u64; committee_size],
        }
    }

    /// Looks up the per-authority count vector for a given misbehavior
    /// category. Exhaustive `match` ensures every variant is wired up.
    pub fn metric(&self, m: Misbehavior) -> &[u64] {
        match m {
            Misbehavior::FaultyBlocksProvable => &self.faulty_blocks_provable,
            Misbehavior::FaultyBlocksUnprovable => &self.faulty_blocks_unprovable,
            Misbehavior::MissingProposals => &self.missing_proposals,
            Misbehavior::Equivocations => &self.equivocations,
        }
    }

    /// Element-wise maximum merge across all four metrics. Adding a metric to
    /// the V1 struct will produce a missing-field error here, forcing the new
    /// metric to be considered.
    pub fn merge_max(&self, other: &Self) -> Self {
        fn elem_max(a: &[u64], b: &[u64]) -> Vec<u64> {
            a.iter().zip(b.iter()).map(|(x, y)| *x.max(y)).collect()
        }
        Self {
            faulty_blocks_provable: elem_max(
                &self.faulty_blocks_provable,
                &other.faulty_blocks_provable,
            ),
            faulty_blocks_unprovable: elem_max(
                &self.faulty_blocks_unprovable,
                &other.faulty_blocks_unprovable,
            ),
            missing_proposals: elem_max(&self.missing_proposals, &other.missing_proposals),
            equivocations: elem_max(&self.equivocations, &other.equivocations),
        }
    }
}

/// Pure field-rename: V1 in-memory ↔ V1 wire format.
impl From<&MisbehaviorCountsV1> for ReportPayloadV1 {
    fn from(c: &MisbehaviorCountsV1) -> Self {
        Self {
            faulty_blocks_provable: c.faulty_blocks_provable.clone(),
            faulty_blocks_unprovable: c.faulty_blocks_unprovable.clone(),
            missing_proposals: c.missing_proposals.clone(),
            equivocations: c.equivocations.clone(),
        }
    }
}

impl From<&ReportPayloadV1> for MisbehaviorCountsV1 {
    fn from(p: &ReportPayloadV1) -> Self {
        Self {
            faulty_blocks_provable: p.faulty_blocks_provable.clone(),
            faulty_blocks_unprovable: p.faulty_blocks_unprovable.clone(),
            missing_proposals: p.missing_proposals.clone(),
            equivocations: p.equivocations.clone(),
        }
    }
}

impl MisbehaviorCounts {
    pub(crate) fn new(version: MisbehaviorSchemaVersion, committee_size: usize) -> Self {
        match version {
            MisbehaviorSchemaVersion::V1 => Self::V1(MisbehaviorCountsV1::zeros(committee_size)),
        }
    }

    /// Builds counts from a consensus-output payload, projecting it onto the
    /// locally tracked schema. Categories the local schema tracks but consensus
    /// did not report are zero-filled; categories consensus reported but the
    /// local schema does not track are silently ignored.
    pub(crate) fn from_consensus_output(
        output_misbehavior_counts: Vec<(ConsensusOutputMisbehavior, Vec<u64>)>,
        version: MisbehaviorSchemaVersion,
        committee_size: usize,
    ) -> Self {
        match version {
            MisbehaviorSchemaVersion::V1 => {
                let mut counts = MisbehaviorCountsV1::zeros(committee_size);
                for (output_misbehavior, row) in output_misbehavior_counts {
                    match Misbehavior::from(&output_misbehavior) {
                        Misbehavior::FaultyBlocksProvable => counts.faulty_blocks_provable = row,
                        Misbehavior::FaultyBlocksUnprovable => {
                            counts.faulty_blocks_unprovable = row
                        }
                        Misbehavior::MissingProposals => counts.missing_proposals = row,
                        Misbehavior::Equivocations => counts.equivocations = row,
                    }
                }
                Self::V1(counts)
            }
        }
    }

    /// Converts the local counts into the wire/storage representation that is
    /// broadcast to peers as a `MisbehaviorReport` transaction.
    pub fn to_report(&self, version: MisbehaviorSchemaVersion) -> VersionedMisbehaviorReport {
        match (version, self) {
            (MisbehaviorSchemaVersion::V1, Self::V1(c)) => {
                VersionedMisbehaviorReport::new_v1(ReportPayloadV1::from(c))
            }
        }
    }

    /// Builds `MisbehaviorCounts` from a peer's report.
    pub fn from_report(
        report: &VersionedMisbehaviorReport,
        version: MisbehaviorSchemaVersion,
    ) -> Self {
        debug_assert!(
            version.accepts_report(report),
            "from_report called with a report whose wire-format version does not match the schema; \
             callers must validate via MisbehaviorSchemaVersion::accepts_report first"
        );
        match (version, &report.payload) {
            (MisbehaviorSchemaVersion::V1, ReportPayload::V1(payload)) => {
                Self::V1(MisbehaviorCountsV1::from(payload))
            }
        }
    }

    /// Element-wise maximum merge. Cross-version merges become a deliberate
    /// design decision when V2 lands (currently impossible — single variant).
    pub fn merge_max(&self, other: &MisbehaviorCounts) -> Self {
        match (self, other) {
            (Self::V1(a), Self::V1(b)) => Self::V1(a.merge_max(b)),
        }
    }
}
