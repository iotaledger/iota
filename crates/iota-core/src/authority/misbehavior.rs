// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{
    MisbehaviorObservations, MisbehaviorObservationsV1, VersionedMisbehaviorReport,
};
use tracing::warn;

use crate::consensus_types::consensus_output_api::ConsensusOutputMisbehavior;

/// Selects which `VersionedMisbehaviorReport` variant peers may submit for
/// the current epoch. Loaded once from `ProtocolConfig` and threaded through
/// `MisbehaviorMonitor` / `ReportAggregator` / `Scorer` as a `Copy` token.
///
/// The schema itself (which categories exist and their layout) lives in
/// `MisbehaviorObservationsV1`; this enum only versions the wire format and
/// gates acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MisbehaviorReportVersion {
    V1,
}

impl MisbehaviorReportVersion {
    pub fn from_protocol(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.misbehavior_monitor_version_as_option() {
            None | Some(1) => Self::V1,
            Some(version) => panic!("Unsupported misbehavior report version {version}"),
        }
    }

    /// Returns `true` if the given report's wire format matches this version.
    pub fn accepts_report(&self, report: &VersionedMisbehaviorReport) -> bool {
        match self {
            Self::V1 => matches!(report.payload, MisbehaviorObservations::V1(_)),
        }
    }
}

/// A single misbehavior category. Used as a name token: constructed from
/// `ConsensusOutputMisbehavior` in `observations_from_consensus_output` for
/// the dedup/missing-category warning loop. Variants are not serialized — the
/// wire format uses named-field `MisbehaviorObservationsV1` (and future
/// `MisbehaviorObservationsVN`) structs. The `Scorer` also stores parameters
/// per named field rather than by enum index. Reordering or renaming variants
/// is therefore safe at the type level.
#[derive(PartialEq, Eq, Clone, Copy, Debug, Hash)]
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

/// Per-version zero observations (all metrics, all authorities = 0).
pub(crate) fn zero_observations(
    version: MisbehaviorReportVersion,
    committee_size: usize,
) -> MisbehaviorObservations {
    match version {
        MisbehaviorReportVersion::V1 => MisbehaviorObservations::V1(MisbehaviorObservationsV1 {
            faulty_blocks_provable: vec![0u64; committee_size],
            faulty_blocks_unprovable: vec![0u64; committee_size],
            missing_proposals: vec![0u64; committee_size],
            equivocations: vec![0u64; committee_size],
        }),
    }
}

/// Element-wise maximum merge across all metrics. Cross-version merges become
/// a deliberate design decision when V2 lands (currently impossible — single
/// variant). Adding a metric to `MisbehaviorObservationsV1` will surface as a
/// missing-field error here, forcing the new metric to be considered.
pub(crate) fn merge_max(
    a: &MisbehaviorObservations,
    b: &MisbehaviorObservations,
) -> MisbehaviorObservations {
    fn elem_max(a: &[u64], b: &[u64]) -> Vec<u64> {
        a.iter().zip(b.iter()).map(|(x, y)| *x.max(y)).collect()
    }
    match (a, b) {
        (MisbehaviorObservations::V1(x), MisbehaviorObservations::V1(y)) => {
            MisbehaviorObservations::V1(MisbehaviorObservationsV1 {
                faulty_blocks_provable: elem_max(
                    &x.faulty_blocks_provable,
                    &y.faulty_blocks_provable,
                ),
                faulty_blocks_unprovable: elem_max(
                    &x.faulty_blocks_unprovable,
                    &y.faulty_blocks_unprovable,
                ),
                missing_proposals: elem_max(&x.missing_proposals, &y.missing_proposals),
                equivocations: elem_max(&x.equivocations, &y.equivocations),
            })
        }
    }
}

/// Builds observations from a consensus-output payload, projecting onto the
/// locally tracked schema. Categories the local schema tracks but consensus
/// did not report are zero-filled; categories consensus reported but the local
/// schema does not track are dropped. Both projections are logged so that
/// schema/protocol drift is visible in operator output.
pub(crate) fn observations_from_consensus_output(
    output_misbehavior_counts: Vec<(ConsensusOutputMisbehavior, Vec<u64>)>,
    version: MisbehaviorReportVersion,
    committee_size: usize,
) -> MisbehaviorObservations {
    match version {
        MisbehaviorReportVersion::V1 => {
            let mut counts = MisbehaviorObservationsV1 {
                faulty_blocks_provable: vec![0u64; committee_size],
                faulty_blocks_unprovable: vec![0u64; committee_size],
                missing_proposals: vec![0u64; committee_size],
                equivocations: vec![0u64; committee_size],
            };
            let mut seen = HashSet::new();
            for (output_misbehavior, row) in output_misbehavior_counts {
                if row.len() != committee_size {
                    warn!(
                        "consensus output row for {output_misbehavior:?} has length {}, \
                         expected committee_size {committee_size}; dropping row",
                        row.len()
                    );
                    continue;
                }
                let category = Misbehavior::from(&output_misbehavior);
                if !seen.insert(category) {
                    warn!(
                        "consensus output contained duplicate row for {category:?}; \
                         overwriting earlier value"
                    );
                }
                match category {
                    Misbehavior::FaultyBlocksProvable => counts.faulty_blocks_provable = row,
                    Misbehavior::FaultyBlocksUnprovable => counts.faulty_blocks_unprovable = row,
                    Misbehavior::MissingProposals => counts.missing_proposals = row,
                    Misbehavior::Equivocations => counts.equivocations = row,
                }
            }
            // V1 schema's expected categories. Compiler-checked: adding a
            // `Misbehavior` variant or a `MisbehaviorObservationsV1` field
            // will surface here as a missing case.
            const V1_EXPECTED: [Misbehavior; 4] = [
                Misbehavior::FaultyBlocksProvable,
                Misbehavior::FaultyBlocksUnprovable,
                Misbehavior::MissingProposals,
                Misbehavior::Equivocations,
            ];
            for expected in V1_EXPECTED {
                if !seen.contains(&expected) {
                    warn!(
                        "consensus output omitted misbehavior category {expected:?}; \
                         zero-filling locally"
                    );
                }
            }
            MisbehaviorObservations::V1(counts)
        }
    }
}
