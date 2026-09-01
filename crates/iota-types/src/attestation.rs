// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{Address, ObjectId, ObjectReference, TransactionDigest, Version};
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

/// Resolves whether an object version was superseded (overwritten, deleted or
/// wrapped) by a transaction of the current epoch — such versions are retained
/// and can be re-run at.
pub trait AttestedObjectVersionReader: Send + Sync {
    fn superseded_in_current_epoch(&self, object_id: &ObjectId, version: Version) -> bool;
}

/// What the execution layer needs to judge an attestation when the attested
/// transaction fails Move authentication at execution.
pub struct AttestationVerdictContext<'a> {
    pub object_versions: &'a [ObjectReference],
    pub computation_units: u64,
    pub version_reader: &'a dyn AttestedObjectVersionReader,
}

impl AttestationVerdictContext<'_> {
    /// Whether authentication should be re-run at the recorded versions.
    pub fn should_reauthenticate(&self, executed_versions: &BTreeMap<ObjectId, Version>) -> bool {
        let mut drifted = false;
        for object_ref in self.object_versions {
            let Some(&executed) = executed_versions.get(object_ref.object_id()) else {
                continue;
            };
            let attested = object_ref.version();
            if attested == executed {
                continue;
            }
            if attested > executed
                || !self
                    .version_reader
                    .superseded_in_current_epoch(object_ref.object_id(), attested)
            {
                return false;
            }
            drifted = true;
        }
        drifted
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

    /// Reports the versions a test marked as superseded during the current
    /// epoch; anything else is unresolvable.
    struct FakeSupersededVersions(std::collections::BTreeSet<(ObjectId, Version)>);

    impl AttestedObjectVersionReader for FakeSupersededVersions {
        fn superseded_in_current_epoch(&self, object_id: &ObjectId, version: Version) -> bool {
            self.0.contains(&(*object_id, version))
        }
    }

    fn verdict_context<'a>(
        object_versions: &'a [ObjectReference],
        reader: &'a FakeSupersededVersions,
    ) -> AttestationVerdictContext<'a> {
        AttestationVerdictContext {
            object_versions,
            computation_units: 1_000_000,
            version_reader: reader,
        }
    }

    fn ref_at(version: u64) -> ObjectReference {
        let base = random_object_ref();
        ObjectReference::new(base.object_id, Version::from(version), base.digest)
    }

    /// Authentication failed against exactly the recorded state, so re-running
    /// would only reproduce it: the attestor vouched for a transaction that
    /// fails at the versions it saw.
    #[test]
    fn no_drift_skips_reauthentication() {
        let account = ref_at(3);
        let reader = FakeSupersededVersions(Default::default());
        let executed = BTreeMap::from([(account.object_id, account.version())]);

        assert!(!verdict_context(&[account], &reader).should_reauthenticate(&executed));
    }

    /// The account moved on after an honest attestation, so the recorded state
    /// still has to be checked before anyone is charged.
    #[test]
    fn drift_within_the_epoch_reauthenticates() {
        let account = ref_at(3);
        let reader = FakeSupersededVersions([(account.object_id, account.version())].into());
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(verdict_context(&[account], &reader).should_reauthenticate(&executed));
    }

    /// A drifted version whose supersession is not from the current epoch is
    /// not state an honest attestor can have read this epoch, and an
    /// attestation is never taken on trust without checking it.
    #[test]
    fn stale_version_skips_reauthentication() {
        let account = ref_at(3);
        let reader = FakeSupersededVersions(Default::default());
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(!verdict_context(&[account], &reader).should_reauthenticate(&executed));
    }

    #[test]
    fn version_ahead_of_execution_skips_reauthentication() {
        let account = ref_at(7);
        let reader = FakeSupersededVersions([(account.object_id, account.version())].into());
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(!verdict_context(&[account], &reader).should_reauthenticate(&executed));
    }

    /// An attestation also records the versions the transaction body read.
    /// Those cannot change whether authentication would have succeeded, so a
    /// stale one must not decide the verdict.
    #[test]
    fn versions_the_reauthentication_does_not_read_are_ignored() {
        let account = ref_at(3);
        let body_object = ref_at(9);
        let reader = FakeSupersededVersions([(account.object_id, account.version())].into());
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(
            verdict_context(&[account, body_object], &reader).should_reauthenticate(&executed),
            "a stale body-side version must not suppress the re-run"
        );
    }

    /// One refuted version decides the verdict even when another version
    /// drifted honestly.
    #[test]
    fn any_refuted_version_skips_reauthentication() {
        let account = ref_at(3);
        let input = ref_at(4);
        let reader = FakeSupersededVersions([(account.object_id, account.version())].into());
        let executed = BTreeMap::from([
            (account.object_id, Version::from(5u64)),
            (input.object_id, Version::from(2u64)),
        ]);

        assert!(!verdict_context(&[account, input], &reader).should_reauthenticate(&executed));
    }

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
