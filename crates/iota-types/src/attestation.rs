// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{ObjectReference, TransactionDigest};
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
/// of `ConsensusTransactionKind::UserTransactionV1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestedTransaction {
    pub transaction: TransactionEnvelope,
    /// `None` until validators attest transactions. Reserved now so attaching
    /// an attestation later leaves the encoding of an unattested transaction
    /// unchanged.
    pub attestation: Option<Attestation>,
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

impl AttestedTransaction {
    pub fn new(transaction: TransactionEnvelope, attestation: Option<Attestation>) -> Self {
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
    use crate::base_types::random_object_ref;

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
}
