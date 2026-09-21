// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::hash::HashFunction;
use iota_sdk_crypto::{Signer, simple::SimpleKeypair};
use iota_sdk_types::{Address, ObjectReference, SimpleSignature, TransactionDigest, UserSignature};
use serde::{Deserialize, Serialize};

use crate::{crypto::DefaultHash, transaction::TransactionEnvelope};

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
    pub transaction: TransactionEnvelope,
    pub attestation: Attestation,
}

/// Digest an explicit attestor signs:
/// `hash(tx_digest || BCS(payload) || attestor_address)`.
pub fn explicit_attestation_digest(
    tx_digest: &TransactionDigest,
    payload: &AttestationData,
    attestor_address: Address,
) -> [u8; 32] {
    let mut hasher = DefaultHash::default();
    hasher.update(tx_digest.bytes());
    hasher
        .update(bcs::to_bytes(payload).expect("BCS serialization of AttestationData cannot fail"));
    hasher.update(AsRef::<[u8]>::as_ref(&attestor_address));
    hasher.finalize().digest
}

impl Attestation {
    /// An attestation by the block-proposing validator at `attestor_index`.
    pub fn new_validator(payload: AttestationData, attestor_index: AuthorityIndex) -> Self {
        Self::Validator {
            payload,
            attestor_index,
        }
    }

    /// An attestation by the registered attestor `attestor_address`, signed
    /// with `keypair` for `tx_digest`.
    pub fn new_explicit(
        tx_digest: &TransactionDigest,
        payload: AttestationData,
        attestor_address: Address,
        keypair: &SimpleKeypair,
    ) -> Self {
        let digest = explicit_attestation_digest(tx_digest, &payload, attestor_address);
        let signature: SimpleSignature = keypair.sign(&digest);
        Self::Explicit {
            payload,
            attestor_address,
            signature: Box::new(UserSignature::Simple(signature)),
        }
    }

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
    use iota_sdk_crypto::ed25519::Ed25519PrivateKey;
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    use crate::{
        base_types::random_object_ref,
        crypto::{get_key_pair_from_rng, zero_ed25519_signature},
        iota_system_state::attestor_registry::{attestor_pubkey_bytes, verify_attestor_signature},
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
            attestor_index: 3,
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
    fn new_explicit_signs_the_documented_digest() {
        let keypair = SimpleKeypair::from(
            get_key_pair_from_rng::<Ed25519PrivateKey, _>(&mut StdRng::from_seed([3; 32])).1,
        );
        let attestor_address = Address::random();
        let tx = create_fake_transaction();
        let payload = make_attestation_data();
        let attestation =
            Attestation::new_explicit(tx.digest(), payload.clone(), attestor_address, &keypair);
        let Attestation::Explicit { signature, .. } = &attestation else {
            panic!("expected an explicit attestation");
        };
        let UserSignature::Simple(signature) = signature.as_ref() else {
            panic!("expected a simple signature");
        };
        let registered_key = attestor_pubkey_bytes(&keypair);
        let digest = explicit_attestation_digest(tx.digest(), &payload, attestor_address);
        verify_attestor_signature(&registered_key, signature, &digest).unwrap();
        // The digest binds the attestor address: another address is refuted.
        let other = explicit_attestation_digest(tx.digest(), &payload, Address::random());
        assert!(verify_attestor_signature(&registered_key, signature, &other).is_err());
    }

    #[test]
    fn attested_transaction_bcs_round_trip() {
        let attested = AttestedTransaction::new(
            create_fake_transaction(),
            Attestation::Validator {
                payload: make_attestation_data(),
                attestor_index: 0,
            },
        );
        let encoded = bcs::to_bytes(&attested).unwrap();
        let decoded: AttestedTransaction = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attested);
    }
}
