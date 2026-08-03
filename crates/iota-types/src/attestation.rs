// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

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

/// State of an object version recorded in an attestation, relative to the epoch
/// executing the attested transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttestedObjectVersionState {
    /// Still the object's current version, so the transaction executed against
    /// exactly this state.
    Current,
    /// Superseded by a transaction that executed in the current epoch. Retained
    /// by every validator, so it can be re-run at.
    SupersededInCurrentEpoch,
    /// Superseded in an earlier epoch, or its supersession is unresolvable.
    Stale,
}

/// Resolves the state of the object versions recorded in an attestation.
pub trait AttestedObjectVersionReader: Send + Sync {
    fn attested_object_version_state(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> AttestedObjectVersionState;
}

/// What the execution layer needs to judge an attestation when the attested
/// transaction fails Move authentication at execution.
pub struct AttestationVerdictContext<'a> {
    pub object_versions: Vec<ObjectReference>,
    /// Consulted only on the authentication-failure path. `None` leaves every
    /// failure to re-run, for callers with no object-version history to read.
    pub version_age_reader: Option<&'a dyn AttestedObjectVersionReader>,
}

impl AttestationVerdictContext<'_> {
    /// Whether authentication should be re-run at the recorded versions.
    ///
    /// False when nothing drifted, because authentication just failed against
    /// exactly the recorded state and a re-run would reproduce it, proving the
    /// attestor's claim false. Also false when a drifted version is too old to
    /// judge. Both cases are `InvalidAttestation`.
    ///
    /// Only the versions the re-run reloads are considered, given by
    /// `reauthenticated_object_ids`: an attestation also records the versions
    /// the transaction body read, and those cannot decide whether
    /// authentication would have succeeded.
    pub fn should_reauthenticate(&self, reauthenticated_object_ids: &BTreeSet<ObjectId>) -> bool {
        let Some(version_age_reader) = self.version_age_reader else {
            return true;
        };
        let mut drifted = false;
        for object_ref in &self.object_versions {
            if !reauthenticated_object_ids.contains(object_ref.object_id()) {
                continue;
            }
            match version_age_reader
                .attested_object_version_state(object_ref.object_id(), object_ref.version())
            {
                AttestedObjectVersionState::Current => {}
                AttestedObjectVersionState::SupersededInCurrentEpoch => drifted = true,
                AttestedObjectVersionState::Stale => return false,
            }
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

    /// Reports whatever state a test assigned to an object, treating anything
    /// unmentioned as `Stale`.
    struct FakeVersionStates(std::collections::BTreeMap<ObjectId, AttestedObjectVersionState>);

    impl AttestedObjectVersionReader for FakeVersionStates {
        fn attested_object_version_state(
            &self,
            object_id: &ObjectId,
            _version: Version,
        ) -> AttestedObjectVersionState {
            self.0
                .get(object_id)
                .copied()
                .unwrap_or(AttestedObjectVersionState::Stale)
        }
    }

    fn verdict_context<'a>(
        object_versions: &[ObjectReference],
        reader: &'a FakeVersionStates,
    ) -> AttestationVerdictContext<'a> {
        AttestationVerdictContext {
            object_versions: object_versions.to_vec(),
            version_age_reader: Some(reader),
        }
    }

    /// Authentication failed against exactly the recorded state, so re-running
    /// would only reproduce it: the attestor vouched for a transaction that
    /// fails at the versions it saw.
    #[test]
    fn no_drift_skips_reauthentication() {
        let account = random_object_ref();
        let reader =
            FakeVersionStates([(account.object_id, AttestedObjectVersionState::Current)].into());
        let ids = BTreeSet::from([account.object_id]);

        assert!(!verdict_context(&[account], &reader).should_reauthenticate(&ids));
    }

    /// The account moved on after an honest attestation, so the recorded state
    /// still has to be checked before anyone is charged.
    #[test]
    fn drift_within_the_epoch_reauthenticates() {
        let account = random_object_ref();
        let reader = FakeVersionStates(
            [(
                account.object_id,
                AttestedObjectVersionState::SupersededInCurrentEpoch,
            )]
            .into(),
        );
        let ids = BTreeSet::from([account.object_id]);

        assert!(verdict_context(&[account], &reader).should_reauthenticate(&ids));
    }

    /// A version superseded before the retained window cannot be re-run at, and
    /// an attestation is never taken on trust without it.
    #[test]
    fn stale_version_skips_reauthentication() {
        let account = random_object_ref();
        let reader =
            FakeVersionStates([(account.object_id, AttestedObjectVersionState::Stale)].into());
        let ids = BTreeSet::from([account.object_id]);

        assert!(!verdict_context(&[account], &reader).should_reauthenticate(&ids));
    }

    /// An attestation also records the versions the transaction body read.
    /// Those cannot change whether authentication would have succeeded, so a
    /// stale one must not decide the verdict.
    #[test]
    fn versions_the_reauthentication_does_not_read_are_ignored() {
        let account = random_object_ref();
        let body_object = random_object_ref();
        let reader = FakeVersionStates(
            [
                (
                    account.object_id,
                    AttestedObjectVersionState::SupersededInCurrentEpoch,
                ),
                (body_object.object_id, AttestedObjectVersionState::Stale),
            ]
            .into(),
        );
        let ids = BTreeSet::from([account.object_id]);

        assert!(
            verdict_context(&[account, body_object], &reader).should_reauthenticate(&ids),
            "a stale body-side version must not suppress the re-run"
        );
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
