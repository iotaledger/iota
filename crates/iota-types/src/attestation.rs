// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
    Validator {
        payload: AttestationData,
        /// Index of the attesting validator in the current epoch's committee
        validator_index: AuthorityIndex,
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
    /// An attestation by the block-proposing validator at `validator_index`.
    pub fn new_validator(payload: AttestationData, validator_index: AuthorityIndex) -> Self {
        Self::Validator {
            payload,
            validator_index,
        }
    }

    pub fn computation_units(&self) -> u64 {
        let Attestation::Validator { payload, .. } = self;
        let AttestationData::V1 {
            computation_units, ..
        } = payload;
        *computation_units
    }

    pub fn object_versions(&self) -> &[ObjectReference] {
        let Attestation::Validator { payload, .. } = self;
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
    /// Authentication failed at the versions the attestor recorded.
    Refuted,
}

impl AttestationVerdict {
    /// `executed_units` is `None` when the transaction body never ran, so the
    /// claim cannot be assessed. `tolerance_percentage` bounds
    /// `|attested - executed|` as a percentage of the executed units; `None`
    /// disables the accuracy check.
    pub fn new(
        refuted: bool,
        attested_units: u64,
        executed_units: Option<u64>,
        tolerance_percentage: Option<u64>,
    ) -> Self {
        if refuted {
            return Self::Refuted;
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
        let Attestation::Validator {
            validator_index, ..
        } = attestation;
        Self {
            attestor: *validator_index,
            verdict,
        }
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
    fn attestation_validator_bcs_round_trip() {
        let attestation = Attestation::Validator {
            payload: make_attestation_data(),
            validator_index: 3,
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
            AttestationVerdict::new(false, attested, Some(executed), tolerance)
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
            AttestationVerdict::new(false, 50 * STEP, None, Some(10)),
            AttestationVerdict::Valid
        );
        // A refutation wins over accuracy.
        assert_eq!(
            AttestationVerdict::new(true, 3 * STEP, Some(3 * STEP), Some(10)),
            AttestationVerdict::Refuted
        );
    }

    #[test]
    fn attested_transaction_bcs_round_trip() {
        let attested = AttestedTransaction::new(
            create_fake_transaction(),
            Attestation::Validator {
                payload: make_attestation_data(),
                validator_index: 0,
            },
        );
        let encoded = bcs::to_bytes(&attested).unwrap();
        let decoded: AttestedTransaction = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attested);
    }
}
