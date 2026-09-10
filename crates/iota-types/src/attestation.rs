// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, ObjectReference, TransactionDigest};
use serde::{Deserialize, Serialize};
// TODO: change the import once the AuthorityIndex refactor is ready
// See https://github.com/iotaledger/iota-private/issues/404
use starfish_config::AuthorityIndex;

use crate::{signature::UserSignature, transaction::Transaction};

/// A pre-consensus claim produced by a trusted actor certifying that a specific
/// transaction has been validated before entering consensus. The attestation is
/// a separate artifact that travels alongside the transaction; the transaction
/// and the user's signature are completely unchanged.
///
/// Two variants are supported:
/// - [`Attestation::Validator`]: produced by the block-proposing validator.
///   Authenticated implicitly by the block signature — no separate attestor
///   signature is needed.
/// - [`Attestation::Explicit`]: produced by a registered third-party attestor.
///   Requires a signature binding the attestation to the transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestation {
    Validator {
        payload: AttestationData,
        /// Index of the attesting validator in the current epoch's committee
        attestor_index: AuthorityIndex,
    },
    Explicit {
        payload: AttestationData,
        attestor_address: Address,
        /// Signs over `hash(transaction.digest() || BCS(payload) ||
        /// attestor_address)`, binding the attestation to both the
        /// specific transaction and the attestor's identity.
        signature: Box<UserSignature>,
    },
}

/// The attested content carried by all [`Attestation`] variants.
///
/// Versioned to allow new fields to be introduced without breaking existing
/// match arms. Both `Validator` and `Explicit` share the same `AttestationData`
/// so any extension applies uniformly across attestation types.
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
    pub transaction: Transaction,
    pub attestation: Attestation,
}

impl Attestation {
    pub fn computation_units(&self) -> u64 {
        let payload = match self {
            Attestation::Validator { payload, .. } | Attestation::Explicit { payload, .. } => {
                payload
            }
        };
        let AttestationData::V1 {
            computation_units, ..
        } = payload;
        *computation_units
    }

    pub fn object_versions(&self) -> &[ObjectReference] {
        let payload = match self {
            Attestation::Validator { payload, .. } | Attestation::Explicit { payload, .. } => {
                payload
            }
        };
        let AttestationData::V1 {
            object_versions, ..
        } = payload;
        object_versions
    }
}

/// The verdict on an attested, executed transaction. The variant order is
/// protocol-significant: append, never reorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttestationVerdict {
    /// The attestation stood and the claimed computation units were within
    /// tolerance of the executed ones.
    Valid,
    /// The attestation stood but the claimed computation units were not.
    Inaccurate,
    /// Authentication failed at the versions the attestor recorded.
    Refuted,
}

impl AttestationVerdict {
    /// `tolerance_percentage` bounds `|attested - executed|` as a percentage of
    /// the executed units; `None` disables the accuracy check.
    pub fn new(
        refuted: bool,
        attested_units: u64,
        executed_units: u64,
        tolerance_percentage: Option<u64>,
    ) -> Self {
        if refuted {
            return Self::Refuted;
        }
        let accurate = tolerance_percentage.is_none_or(|tolerance| {
            attested_units.abs_diff(executed_units).saturating_mul(100)
                <= tolerance.saturating_mul(executed_units)
        });
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
    pub attestor: AuthorityIndex,
    pub verdict: AttestationVerdict,
}

impl AttestationRecord {
    /// `None` for explicit attestations, which never reach execution.
    pub fn new(attestation: &Attestation, verdict: AttestationVerdict) -> Option<Self> {
        match attestation {
            Attestation::Validator { attestor_index, .. } => Some(Self {
                attestor: *attestor_index,
                verdict,
            }),
            Attestation::Explicit { .. } => None,
        }
    }
}

impl AttestedTransaction {
    pub fn new(transaction: Transaction, attestation: Attestation) -> Self {
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
    use crate::{
        base_types::random_object_ref, crypto::zero_ed25519_signature,
        utils::create_fake_transaction,
    };

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
            attestor_index: AuthorityIndex::new_for_test(3),
        };
        let encoded = bcs::to_bytes(&attestation).unwrap();
        let decoded: Attestation = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attestation);
    }

    #[test]
    fn attestation_explicit_bcs_round_trip() {
        let attestation = Attestation::Explicit {
            payload: make_attestation_data(),
            attestor_address: Address::random(),
            signature: Box::new(UserSignature::Simple(zero_ed25519_signature())),
        };
        let encoded = bcs::to_bytes(&attestation).unwrap();
        let decoded: Attestation = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attestation);
    }

    #[test]
    fn verdict_accuracy_band() {
        let judge = |attested, executed, tolerance| {
            AttestationVerdict::new(false, attested, executed, tolerance)
        };
        assert_eq!(judge(90, 100, Some(10)), AttestationVerdict::Valid);
        assert_eq!(judge(110, 100, Some(10)), AttestationVerdict::Valid);
        assert_eq!(judge(89, 100, Some(10)), AttestationVerdict::Inaccurate);
        assert_eq!(judge(111, 100, Some(10)), AttestationVerdict::Inaccurate);
        // Zero executed units accept only a zero claim.
        assert_eq!(judge(0, 0, Some(10)), AttestationVerdict::Valid);
        assert_eq!(judge(1, 0, Some(10)), AttestationVerdict::Inaccurate);
        // No tolerance configured disables the check.
        assert_eq!(judge(1_000, 1, None), AttestationVerdict::Valid);
        // A refutation wins over accuracy.
        assert_eq!(
            AttestationVerdict::new(true, 100, 100, Some(10)),
            AttestationVerdict::Refuted
        );
    }

    #[test]
    fn attested_transaction_bcs_round_trip() {
        let attested = AttestedTransaction::new(
            create_fake_transaction(),
            Attestation::Validator {
                payload: make_attestation_data(),
                attestor_index: AuthorityIndex::new_for_test(0),
            },
        );
        let encoded = bcs::to_bytes(&attested).unwrap();
        let decoded: AttestedTransaction = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attested);
    }
}
