// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{
    MisbehaviorObservations, MisbehaviorObservationsV1, VersionedMisbehaviorReport,
};
use tracing::error;

use crate::consensus_types::consensus_output_api::ConsensusOutputMisbehaviorCounts;

/// Selects which `VersionedMisbehaviorReport` variant peers may submit for
/// the current epoch. Loaded once from `ProtocolConfig` and threaded through
/// `MisbehaviorMonitor` / `ReportAggregator` / `Scoreboard` as a `Copy` token.
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
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => Self::V1,
            Some(version) => panic!("Unsupported scorer version {version}"),
        }
    }

    /// Returns `true` if the given report's wire format matches this version.
    pub fn accepts_report(&self, report: &VersionedMisbehaviorReport) -> bool {
        match self {
            Self::V1 => matches!(report.payload, MisbehaviorObservations::V1(_)),
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

/// Maps consensus-output counts onto the locally tracked schema. Per-metric
/// vectors are projected onto `committee_size`: empty (= "not wired by
/// consensus yet") is silently zero-filled, correct length is used as-is,
/// any other length is malformed and is logged + zero-filled.
pub(crate) fn observations_from_consensus_output(
    counts: ConsensusOutputMisbehaviorCounts,
    version: MisbehaviorReportVersion,
    committee_size: usize,
) -> MisbehaviorObservations {
    let project = |row: Vec<u64>, name: &'static str| -> Vec<u64> {
        if row.is_empty() {
            vec![0u64; committee_size]
        } else if row.len() == committee_size {
            row
        } else {
            // A non-empty wrong-length row means consensus and iota-core
            // disagree on committee size — this is a programmer error, not
            // adversarial input (consensus is local code). Halt in debug /
            // tests; in production, log loudly and zero-fill so the node
            // keeps making progress (zero-fill is harmless: merge_max with
            // zeros leaves prior state untouched).
            debug_assert_eq!(
                row.len(),
                committee_size,
                "consensus output row for {name} has wrong length — consensus and \
                 iota-core disagree on committee size"
            );
            error!(
                name,
                actual = row.len(),
                expected = committee_size,
                "consensus output row has wrong length; consensus and iota-core \
                 disagree on committee size — zero-filling and continuing"
            );
            vec![0u64; committee_size]
        }
    };
    match version {
        MisbehaviorReportVersion::V1 => MisbehaviorObservations::V1(MisbehaviorObservationsV1 {
            faulty_blocks_provable: project(
                counts.faulty_blocks_provable,
                "faulty_blocks_provable",
            ),
            faulty_blocks_unprovable: project(
                counts.faulty_blocks_unprovable,
                "faulty_blocks_unprovable",
            ),
            missing_proposals: project(counts.missing_proposals, "missing_proposals"),
            equivocations: project(counts.equivocations, "equivocations"),
        }),
    }
}
