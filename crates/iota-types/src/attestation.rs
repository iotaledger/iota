// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, ObjectReference, TransactionDigest, UserSignature};
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
    /// The gas vector: per-resource demand computed from the attestor's
    /// dry-run resource profile, replacing the single `computation_units`
    /// number. Produced and accepted only when the `attestation_gas_vector`
    /// feature flag is on (old nodes cannot decode an unknown variant, so
    /// production and acceptance are gated together).
    V2 {
        /// Predicted lane-time in reference-hardware nanoseconds: the
        /// dry-run's resource-profile counters priced by the protocol
        /// config's calibrated coefficients. Every validator can recompute
        /// the same value from the same counters, which is what makes
        /// attested-vs-actual divergence checkable.
        cpu_time: u64,
        /// Bytes moved through the shared memory/store path during the
        /// dry-run, as the weighted sum over the profile's moved-bytes
        /// counters (reads at their per-operation byte-equivalent, hash
        /// input at its per-byte weight, plain moved bytes at 1).
        moved_bytes: u64,
        /// Write-cost bytes from the dry-run effects (object bytes + event
        /// bytes + the per-deletion constant). Recorded now; consumed by the
        /// write budget when it ships.
        write_bytes: u64,
        /// Same role as in `V1`: the misbehavior-detection evidence base.
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
    pub fn payload(&self) -> &AttestationData {
        match self {
            Attestation::Validator { payload, .. } | Attestation::Explicit { payload, .. } => {
                payload
            }
        }
    }

    /// The V1 scheduling estimate. `None` for a V2 payload: the gas vector
    /// is not a unit count, and pretending otherwise would mix units in the
    /// congestion tracker — V2 consumers use [`Self::declared_cpu_time`]
    /// once admission is time-denominated.
    pub fn computation_units(&self) -> Option<u64> {
        match self.payload() {
            AttestationData::V1 {
                computation_units, ..
            } => Some(*computation_units),
            AttestationData::V2 { .. } => None,
        }
    }

    /// Attested lane-time in reference-hardware nanoseconds (V2 only).
    pub fn declared_cpu_time(&self) -> Option<u64> {
        match self.payload() {
            AttestationData::V1 { .. } => None,
            AttestationData::V2 { cpu_time, .. } => Some(*cpu_time),
        }
    }

    /// Attested weighted moved bytes (V2 only).
    pub fn declared_moved_bytes(&self) -> Option<u64> {
        match self.payload() {
            AttestationData::V1 { .. } => None,
            AttestationData::V2 { moved_bytes, .. } => Some(*moved_bytes),
        }
    }

    /// Attested write-cost bytes (V2 only).
    pub fn declared_write_bytes(&self) -> Option<u64> {
        match self.payload() {
            AttestationData::V1 { .. } => None,
            AttestationData::V2 { write_bytes, .. } => Some(*write_bytes),
        }
    }

    /// The run-time-resolved object versions the attestor read — present in
    /// every payload version.
    pub fn object_versions(&self) -> &[ObjectReference] {
        match self.payload() {
            AttestationData::V1 {
                object_versions, ..
            }
            | AttestationData::V2 {
                object_versions, ..
            } => object_versions,
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

    fn make_attestation_data_v2() -> AttestationData {
        AttestationData::V2 {
            cpu_time: 1_500_000,
            moved_bytes: 64 * 1024,
            write_bytes: 2_048,
            object_versions: vec![random_object_ref()],
        }
    }

    #[test]
    fn attestation_data_v2_bcs_round_trip() {
        let data = make_attestation_data_v2();
        let encoded = bcs::to_bytes(&data).unwrap();
        let decoded: AttestationData = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn v2_accessors_and_v1_unit_partition() {
        let v1 = Attestation::Validator {
            payload: make_attestation_data(),
            attestor_index: 0,
        };
        assert_eq!(v1.computation_units(), Some(1_000_000));
        assert_eq!(v1.declared_cpu_time(), None);
        assert_eq!(v1.object_versions().len(), 1);

        let v2 = Attestation::Validator {
            payload: make_attestation_data_v2(),
            attestor_index: 0,
        };
        assert_eq!(v2.computation_units(), None);
        assert_eq!(v2.declared_cpu_time(), Some(1_500_000));
        assert_eq!(v2.declared_moved_bytes(), Some(64 * 1024));
        assert_eq!(v2.declared_write_bytes(), Some(2_048));
        assert_eq!(v2.object_versions().len(), 1);
    }

    #[test]
    fn v1_byte_layout_is_unchanged_by_the_new_variant() {
        // Adding V2 must not disturb V1's wire format: variant index 0 plus
        // the same fields. A golden prefix guards the variant tag.
        let data = AttestationData::V1 {
            computation_units: 7,
            object_versions: vec![],
        };
        let encoded = bcs::to_bytes(&data).unwrap();
        assert_eq!(encoded[0], 0, "V1 must keep BCS variant index 0");
        let v2 = make_attestation_data_v2();
        assert_eq!(bcs::to_bytes(&v2).unwrap()[0], 1, "V2 is variant index 1");
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
