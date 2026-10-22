// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{GasCostSummary, ObjectReference, TransactionDigest};
use serde::{Deserialize, Serialize};

use crate::transaction::TransactionEnvelope;

/// Index of a validator in the current epoch's consensus committee. Kept as a
/// plain `u8` so `iota-types` does not depend on `starfish-config`, whose
/// `AuthorityIndex(u8)` is BCS-identical; the value is untrusted until checked
/// against the block author post-consensus.
pub(super) type AuthorityIndex = u8;

/// A pre-consensus claim produced by a trusted actor certifying that a specific
/// transaction has been validated before entering consensus. The attestation is
/// a separate artifact that travels alongside the transaction; the transaction
/// and the user's signature are completely unchanged.
///
/// An enum so that other attestor kinds can be appended later; the variant
/// order is protocol-significant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestation {
    /// Produced by the block-proposing validator and authenticated by the
    /// block signature, so it carries no signature of its own.
    Authority {
        payload: AttestationData,
        /// Index of the attesting validator in the current epoch's committee
        authority_index: AuthorityIndex,
    },
}

/// The attested content carried by every [`Attestation`] variant.
///
/// Versioned to allow new fields to be introduced without breaking existing
/// match arms.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttestationData {
    V1 {
        /// Expected computation units billed by the attestor's dry-run
        /// (`computation_cost / gas_price`). Used by the sequencer to improve
        /// shared-object scheduling before execution.
        computation_units: u64,
        /// Versions of the run-time-resolved objects the attestor read during
        /// the dry-run whose version is NOT already pinned by the signed
        /// `TransactionData`. This covers shared objects,
        /// Move-authenticator account and function-ref field objects,
        /// coin-deny-list references, and dynamic fields / child objects loaded
        /// during execution.
        object_versions: Vec<ObjectReference>,
    },
}

/// A user transaction bundled with its attestation. This is the inner payload
/// of `ConsensusTransactionKind::UserTransactionV2`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestedTransaction {
    pub transaction: TransactionEnvelope,
    pub attestation: Attestation,
}

impl Attestation {
    /// An attestation by the block-proposing validator at `authority_index`.
    pub fn new_authority(payload: AttestationData, authority_index: AuthorityIndex) -> Self {
        Self::Authority {
            payload,
            authority_index,
        }
    }

    pub fn computation_units(&self) -> u64 {
        let Attestation::Authority { payload, .. } = self;
        let AttestationData::V1 {
            computation_units, ..
        } = payload;
        *computation_units
    }

    pub fn object_versions(&self) -> &[ObjectReference] {
        let Attestation::Authority { payload, .. } = self;
        let AttestationData::V1 {
            object_versions, ..
        } = payload;
        object_versions
    }
}

/// The computation cost of `gas_cost_summary` in gas units, independent of
/// the gas price the transaction paid. Rounds down, and is zero when
/// `gas_price` is zero.
pub fn computation_units(gas_cost_summary: &GasCostSummary, gas_price: u64) -> u64 {
    gas_cost_summary
        .computation_cost
        .checked_div(gas_price)
        .unwrap_or(0)
}

/// The verdict on an attested, executed transaction. The variant order is
/// protocol-significant: append, never reorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttestationVerdict {
    /// Not refuted and, whenever the transaction body ran, the claimed
    /// computation units were within tolerance of the executed ones.
    Valid,
    /// Not refuted, but the claimed computation units were not within
    /// tolerance. An honest claim can land here when a shared object or
    /// dynamic field the dry run read moved before execution, so this is not
    /// by itself evidence against the attestor.
    Inaccurate,
    /// Authentication failed at the versions the attestor recorded. `units`
    /// is the computation both failed runs consumed: the one execution
    /// charged and the judge's re-run at the recorded versions.
    Refuted { units: u64 },
}

impl AttestationVerdict {
    /// `refuted_units` carries the units of both failed runs when the failure
    /// refutes the attestation. `executed_units` is `None` when the
    /// transaction body never ran, so the claim cannot be assessed.
    /// `tolerance_percentage` bounds `|attested - executed|` as a percentage
    /// of the executed units; `None` disables the accuracy check.
    pub fn new(
        refuted_units: Option<u64>,
        attested_units: u64,
        executed_units: Option<u64>,
        tolerance_percentage: Option<u64>,
    ) -> Self {
        if let Some(units) = refuted_units {
            return Self::Refuted { units };
        }
        let accurate = match (executed_units, tolerance_percentage) {
            (Some(executed), Some(tolerance)) => {
                attested_units.abs_diff(executed).saturating_mul(100)
                    <= tolerance.saturating_mul(executed)
            }
            _ => true,
        };
        if accurate {
            Self::Valid
        } else {
            Self::Inaccurate
        }
    }

    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// The validator's verdict on an attested, executed transaction, certified in
/// the checkpoint summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttestationRecord {
    /// Index of the attestor in the committee of the checkpoint's epoch.
    pub attestor: AuthorityIndex,
    pub verdict: AttestationVerdict,
}

impl AttestationRecord {
    pub fn new(attestation: &Attestation, verdict: AttestationVerdict) -> Self {
        let Attestation::Authority {
            authority_index, ..
        } = attestation;
        Self {
            attestor: *authority_index,
            verdict,
        }
    }
}

/// A record other than `Valid`, certified in the checkpoint summary at its
/// transaction's position in the checkpoint contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttestationFault {
    pub position: u32,
    pub record: AttestationRecord,
}

impl AttestationFault {
    /// The records other than `Valid`, each at its position in `records`.
    ///
    /// # Panics
    ///
    /// If a position does not fit `u32`.
    pub fn from_records(records: &[Option<AttestationRecord>]) -> Vec<Self> {
        records
            .iter()
            .enumerate()
            .filter_map(|(position, record)| {
                let record = (*record).filter(|record| !record.verdict.is_valid())?;
                Some(Self {
                    position: u32::try_from(position).expect("checkpoint positions fit u32"),
                    record,
                })
            })
            .collect()
    }
}

/// Per-attestor totals of refuted attestations for the epoch so far,
/// certified in every checkpoint summary. An attestor without refuted
/// attestations has no entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationTotals {
    /// Committee index to the sum of the squared units of its `Refuted`
    /// verdicts since the epoch started, each first capped at one max-gas
    /// authentication run; saturating.
    pub refuted_units_squared: BTreeMap<AuthorityIndex, u64>,
}

impl AttestationTotals {
    /// These totals plus the `Refuted` records of one more checkpoint, each
    /// record's units capped at `max_auth_units`, one max-gas authentication
    /// run.
    pub fn with_records(
        mut self,
        records: &[Option<AttestationRecord>],
        max_auth_units: u64,
    ) -> Self {
        for record in records.iter().flatten() {
            if let AttestationVerdict::Refuted { units } = record.verdict {
                let sum = self
                    .refuted_units_squared
                    .entry(record.attestor)
                    .or_default();
                *sum = sum.saturating_add(units.min(max_auth_units).saturating_pow(2));
            }
        }
        self
    }
}

impl AttestedTransaction {
    pub fn new(transaction: TransactionEnvelope, attestation: Attestation) -> Self {
        Self {
            transaction,
            attestation,
        }
    }

    pub fn digest(&self) -> &TransactionDigest {
        self.transaction.digest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{base_types::random_object_ref, utils::create_fake_transaction};

    fn make_attestation_data() -> AttestationData {
        AttestationData::V1 {
            computation_units: 1_000_000,
            object_versions: vec![random_object_ref()],
        }
    }

    #[test]
    fn attestation_data_bcs_round_trip() {
        let data = make_attestation_data();
        let encoded = bcs::to_bytes(&data).unwrap();
        let decoded: AttestationData = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn attestation_authority_bcs_round_trip() {
        let attestation = Attestation::Authority {
            payload: make_attestation_data(),
            authority_index: 3,
        };
        let encoded = bcs::to_bytes(&attestation).unwrap();
        let decoded: Attestation = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attestation);
    }

    /// Both sides of the check are `computation_cost / gas_price`, which the
    /// gas meter rounds up to `gas_rounding_step`, so they are always multiples
    /// of it and the smallest difference they can show is one step.
    #[test]
    fn verdict_accuracy_band() {
        const STEP: u64 = 1_000;
        let judge = |attested, executed, tolerance| {
            AttestationVerdict::new(None, attested, Some(executed), tolerance)
        };

        // Under ten steps a tenth of the executed units is less than one step,
        // so only the same value is accurate.
        assert_eq!(
            judge(3 * STEP, 3 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(4 * STEP, 3 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );
        assert_eq!(
            judge(2 * STEP, 3 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );

        // At ten steps one step of drift is exactly the allowance.
        assert_eq!(
            judge(11 * STEP, 10 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(9 * STEP, 10 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(12 * STEP, 10 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );

        // Ten steps either way on a hundred-step transaction.
        assert_eq!(
            judge(110 * STEP, 100 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(111 * STEP, 100 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );

        // No tolerance configured disables the check.
        assert_eq!(judge(50 * STEP, STEP, None), AttestationVerdict::Valid);
        // A body that never ran cannot be assessed.
        assert_eq!(
            AttestationVerdict::new(None, 50 * STEP, None, Some(10)),
            AttestationVerdict::Valid
        );
        // A refutation wins over accuracy and keeps the units of both runs.
        assert_eq!(
            AttestationVerdict::new(Some(7 * STEP), 3 * STEP, Some(3 * STEP), Some(10)),
            AttestationVerdict::Refuted { units: 7 * STEP }
        );
    }

    #[test]
    fn totals_sum_the_capped_squared_units_of_refuted_records() {
        let record = |attestor, verdict| Some(AttestationRecord { attestor, verdict });
        let refuted = |units| AttestationVerdict::Refuted { units };
        let totals = AttestationTotals::default().with_records(
            &[
                record(2, refuted(3)),
                record(1, AttestationVerdict::Valid),
                record(1, AttestationVerdict::Inaccurate),
                None,
                // Capped at the max-gas run of 10 units.
                record(2, refuted(25)),
            ],
            10,
        );
        assert_eq!(totals.refuted_units_squared, BTreeMap::from([(2, 9 + 100)]));

        // A later checkpoint adds to the running sums.
        let totals = totals.with_records(&[record(0, refuted(1)), record(2, refuted(2))], 10);
        assert_eq!(
            totals.refuted_units_squared,
            BTreeMap::from([(0, 1), (2, 9 + 100 + 4)])
        );
        assert_eq!(
            bcs::to_bytes(&AttestationTotals::default()).unwrap(),
            vec![0],
            "no faults cost one byte"
        );
    }

    #[test]
    fn attested_transaction_bcs_round_trip() {
        let attested = AttestedTransaction::new(
            create_fake_transaction(),
            Attestation::Authority {
                payload: make_attestation_data(),
                authority_index: 0,
            },
        );
        let encoded = bcs::to_bytes(&attested).unwrap();
        let decoded: AttestedTransaction = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attested);
    }
}
