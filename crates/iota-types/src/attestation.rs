// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
// TODO: change the import once the AuthorityIndex refactor is ready
use starfish_config::AuthorityIndex;

use crate::{
    base_types::{IotaAddress, ObjectRef},
    digests::TransactionDigest,
    signature::GenericSignature,
    transaction::Transaction,
};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Attestation {
    Validator {
        payload: AttestationData,
        /// Index of the attesting validator in the current epoch's committee
        attestor_index: AuthorityIndex,
    },
    Explicit {
        payload: AttestationData,
        attestor_address: IotaAddress,
        /// Signs over `hash(transaction.digest() || BCS(payload) ||
        /// attestor_address)`, binding the attestation to both the
        /// specific transaction and the attestor's identity.
        signature: Box<GenericSignature>,
    },
}

/// The attested content carried by all [`Attestation`] variants.
///
/// Versioned to allow new fields to be introduced without breaking existing
/// match arms. Both `Validator` and `Explicit` share the same `AttestationData`
/// so any extension applies uniformly across attestation types.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttestationData {
    V1 {
        /// Expected computation cost, in NANOS, used by the sequencer to
        /// improve shared-object scheduling before execution.
        estimated_computation_cost: u64,
        /// Shared-object versions observed by the attestor during the dry-run.
        /// Used to determine whether a discrepancy between the attested
        /// estimate and the actual execution outcome is misbehavior or is
        /// explained by a legitimate state change between attestation time and
        /// execution time.
        object_versions: Vec<ObjectRef>,
    },
}

/// A user transaction bundled with its attestation. This is the inner payload
/// of `ConsensusTransactionKind::UserTransactionV2`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttestedTransaction {
    pub transaction: Transaction,
    pub attestation: Attestation,
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
    use bcs;

    use super::*;
    use crate::{
        base_types::{IotaAddress, random_object_ref},
        crypto::Ed25519IotaSignature,
        utils::create_fake_transaction,
    };

    fn make_attestation_data() -> AttestationData {
        AttestationData::V1 {
            estimated_computation_cost: 1_000_000,
            object_versions: vec![random_object_ref()],
        }
    }

    #[test]
    fn attestation_data_bcs_round_trip() {
        let data = make_attestation_data();
        let encoded = bcs::to_bytes(&data).unwrap();
        let decoded: AttestationData = bcs::from_bytes(&encoded).unwrap();
        let AttestationData::V1 {
            estimated_computation_cost,
            object_versions,
        } = decoded;
        assert_eq!(estimated_computation_cost, 1_000_000);
        assert_eq!(object_versions.len(), 1);
    }

    #[test]
    fn attestation_validator_bcs_round_trip() {
        let attestation = Attestation::Validator {
            payload: make_attestation_data(),
            attestor_index: AuthorityIndex::new_for_test(3),
        };
        let encoded = bcs::to_bytes(&attestation).unwrap();
        let decoded: Attestation = bcs::from_bytes(&encoded).unwrap();
        let Attestation::Validator { attestor_index, .. } = decoded else {
            panic!("unexpected variant");
        };
        assert_eq!(attestor_index, AuthorityIndex::new_for_test(3));
    }

    #[test]
    fn attestation_explicit_bcs_round_trip() {
        let address = IotaAddress::random();
        let attestation = Attestation::Explicit {
            payload: make_attestation_data(),
            attestor_address: address,
            signature: Box::new(GenericSignature::Signature(
                Ed25519IotaSignature::default().into(),
            )),
        };
        let encoded = bcs::to_bytes(&attestation).unwrap();
        let decoded: Attestation = bcs::from_bytes(&encoded).unwrap();
        let Attestation::Explicit {
            attestor_address, ..
        } = decoded
        else {
            panic!("unexpected variant");
        };
        assert_eq!(attestor_address, address);
    }

    #[test]
    fn attested_transaction_bcs_round_trip() {
        let tx = create_fake_transaction();
        let digest = *tx.digest();
        let attested = AttestedTransaction::new(
            tx,
            Attestation::Validator {
                payload: make_attestation_data(),
                attestor_index: AuthorityIndex::new_for_test(0),
            },
        );
        let encoded = bcs::to_bytes(&attested).unwrap();
        let decoded: AttestedTransaction = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(*decoded.digest(), digest);
    }
}
