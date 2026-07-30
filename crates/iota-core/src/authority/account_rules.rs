// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Account rules applied by the sequencer.
//!
//! An address is an *explicit* account once an object with that id exists (the
//! object is created by a `ClaimAccount` transaction whose sender is the
//! claimed address); it is *implicit* otherwise. The rules below keep the two
//! states consistent for every validator without reading local execution
//! state:
//!
//! - a `ClaimAccount` for an address that is already explicit is removed (the
//!   duplicate-claim guard: it prevents a second object with the same id from
//!   ever being minted);
//! - a plain-signed transaction whose sender or gas owner is already explicit
//!   is removed (an explicit account must authenticate with a
//!   `MoveAuthenticator`);
//! - a transaction whose `MoveAuthenticator` references an account whose claim
//!   was cancelled earlier in the same commit is removed (the referenced object
//!   can never exist at the declared version).
//!
//! The rules run inside the scheduling pass, per transaction and *before* the
//! transaction's congestion scheduling decision. Checking before scheduling
//! matters: a transaction doomed by the account rules must never occupy
//! scheduling capacity, otherwise flooding the sequencer with doomed
//! transactions (duplicate claims for an already-explicit address, plain-signed
//! transactions racing one's own claim) would create artificial congestion and
//! defer or cancel legitimate transactions. Decisions are never speculative:
//! the pass order is deterministic, and when a transaction is checked, the
//! scheduling fate of every earlier transaction — including every earlier
//! claim — is already settled.
//!
//! "Earlier in the commit" therefore means earlier in the scheduling pass. A
//! dropped transaction never reaches the version-assignment walk, so no
//! version-chain decision can disagree with the pass order.
//!
//! Removed transactions are dropped deterministically and surfaced to clients
//! through the dropped-transaction status cache. Once owned-object-only
//! transactions can execute in cancelled mode, these drops should become
//! cancellations with failure effects instead, so that gas is charged and the
//! outcome is checkpoint-recorded.

use std::collections::HashSet;

use iota_sdk_types::{Address, ObjectId, TransactionKind};
#[cfg(test)]
use iota_types::executable_transaction::VerifiedExecutableTransaction;
use iota_types::{
    error::{IotaError, IotaResult},
    transaction::{SenderSignedData, TransactionDataAPI},
};

use crate::{authority::AuthorityPerEpochStore, execution_cache::ObjectCacheRead};

/// Returns the address a `ClaimAccount` transaction claims — its sender, which
/// is also the id of the account object the claim creates. `None` for every
/// other transaction kind.
pub(crate) fn claimed_account_address(data: &SenderSignedData) -> Option<ObjectId> {
    match data.transaction_data().kind() {
        TransactionKind::ClaimAccount(_) => Some(data.transaction_data().sender().into()),
        _ => None,
    }
}

/// Returns the addresses this transaction authorizes with a plain signature:
/// the sender and the gas owner, minus any address authenticated by a
/// `MoveAuthenticator`.
fn plain_signed_addresses(data: &SenderSignedData) -> Vec<Address> {
    let transaction_data = data.transaction_data();
    let mut addresses = vec![transaction_data.sender()];
    let gas_owner = transaction_data.gas_owner();
    if gas_owner != transaction_data.sender() {
        addresses.push(gas_owner);
    }
    let authenticated: HashSet<Address> = authenticated_account_addresses(data)
        .map(Address::from)
        .collect();
    addresses.retain(|address| !authenticated.contains(address));
    addresses
}

/// Returns the ids of the accounts this transaction authenticates through
/// `MoveAuthenticator` signatures.
fn authenticated_account_addresses(data: &SenderSignedData) -> impl Iterator<Item = ObjectId> + '_ {
    data.move_authenticators()
        .into_iter()
        .filter_map(|authenticator| authenticator.address().ok().map(ObjectId::from))
}

/// Account-rules state threaded through one commit's scheduling pass: the
/// claims the sequencer has scheduled or cancelled at earlier positions of
/// the pass.
#[derive(Default)]
pub(crate) struct AccountRulesState {
    /// Addresses claimed by a scheduled claim at an earlier position of this
    /// commit; the version-assignment walk stages exactly these claims.
    commit_claims: HashSet<ObjectId>,
    /// Addresses whose claim was cancelled by congestion in this commit. A
    /// cancelled claim stages nothing: the address stays implicit and
    /// claimable.
    cancelled_claims: HashSet<ObjectId>,
}

impl AccountRulesState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Checks one transaction against the account rules, at its position of
    /// the scheduling pass. Must be called *before* the transaction's
    /// congestion scheduling decision, so that a violating transaction never
    /// takes scheduling capacity. Returns the rejection error if the
    /// transaction must be dropped.
    pub(crate) fn check_transaction(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
        data: &SenderSignedData,
    ) -> IotaResult<Option<IotaError>> {
        if !epoch_store.protocol_config().enable_claim_registry() {
            return Ok(None);
        }
        if data.transaction_data().is_system_tx() {
            return Ok(None);
        }

        let claim_address = claimed_account_address(data);
        if let Some(address) = claim_address {
            // The duplicate-claim guard: it prevents a second object with the
            // same id from ever being minted. The first scheduled claim wins.
            if self.resolve_explicit(epoch_store, cache_reader, &address)? {
                return Ok(Some(IotaError::AccountAlreadyExplicit {
                    address: address.into(),
                }));
            }
        }

        for address in plain_signed_addresses(data) {
            let account_id = ObjectId::from(address);
            // The claim's own sender is decided by the duplicate-claim guard
            // above, not by the plain-signature rule.
            if claim_address == Some(account_id) {
                continue;
            }
            if self.resolve_explicit(epoch_store, cache_reader, &account_id)? {
                return Ok(Some(IotaError::PlainSignatureForExplicitAccount {
                    address,
                }));
            }
        }

        for account_id in authenticated_account_addresses(data) {
            if self.cancelled_claims.contains(&account_id)
                && !self.resolve_explicit(epoch_store, cache_reader, &account_id)?
            {
                // The account object the authenticator references can never
                // come to exist: its claim was cancelled at an earlier
                // position of this commit and no other claim covers the
                // address.
                return Ok(Some(IotaError::DependencyOnCancelledClaim {
                    address: account_id.into(),
                }));
            }
        }

        Ok(None)
    }

    /// Records the claim of a transaction the sequencer scheduled. No-op for
    /// transactions that are not claims.
    pub(crate) fn record_scheduled(&mut self, data: &SenderSignedData) {
        if let Some(address) = claimed_account_address(data) {
            self.commit_claims.insert(address);
        }
    }

    /// Records the claim of a transaction the sequencer cancelled. No-op for
    /// transactions that are not claims.
    pub(crate) fn record_cancelled(&mut self, data: &SenderSignedData) {
        if let Some(address) = claimed_account_address(data) {
            self.cancelled_claims.insert(address);
        }
    }

    /// Answers "is the account at `address` explicit, as of this position of
    /// the scheduling pass?" identically on every validator.
    ///
    /// Consults, in order: claims scheduled earlier in this commit, claim
    /// entries of the current epoch (quarantine, then the epoch table), and
    /// the object store. The store branch is uniform because the resolution is
    /// only consulted for signature-derivable addresses: an object can exist
    /// there only through a claim, and every current-epoch claim is caught by
    /// the first two steps, so a store hit is always a claim settled in a
    /// previous epoch. Bare `next_shared_object_versions` entries are never
    /// consulted — any transaction can seed one with an arbitrary id.
    fn resolve_explicit(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
        address: &ObjectId,
    ) -> IotaResult<bool> {
        if self.commit_claims.contains(address) {
            return Ok(true);
        }
        if epoch_store.get_claimed_account(address)?.is_some() {
            return Ok(true);
        }
        Ok(cache_reader.get_object(address).is_some())
    }
}

/// Builds a `ClaimAccount` transaction for a random sender, returning the
/// claimed address together with the transaction. The gas object version
/// steers the transaction's lamport version.
#[cfg(test)]
pub(crate) fn generate_claim_account_tx_with_gas_version(
    gas_object_version: u64,
) -> (ObjectId, VerifiedExecutableTransaction) {
    use iota_sdk_types::{
        ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim,
        crypto::{Ed25519PublicKey, PublicKey, PublicKeyExt},
    };
    use iota_types::{
        base_types::{ObjectRef, SequenceNumber},
        crypto::{AccountKeyPair, KeypairTraits, get_key_pair},
        digests::ObjectDigest,
        executable_transaction::{CertificateProof, ExecutableTransaction},
        transaction::{SenderSignedData, TransactionData},
    };

    let (sender, keypair): (Address, AccountKeyPair) = get_key_pair();
    let claim = SmartAccountClaim {
        public_key: PublicKey::Ed25519(
            Ed25519PublicKey::from_bytes(keypair.public().as_ref()).unwrap(),
        ),
        // Leftover of an earlier draft of the transaction kind; ignored.
        claim_registry_initial_shared_version: 0,
        fields: vec![],
        build_kind: SmartAccountBuildKind::Mutable,
    };
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));
    let tx_data = TransactionData::new(
        kind,
        sender,
        ObjectRef::new(
            ObjectId::random(),
            SequenceNumber::from_u64(gas_object_version),
            ObjectDigest::random(),
        ),
        10_000_000,
        1,
    );
    let tx = SenderSignedData::new(tx_data, vec![]);
    (
        sender.into(),
        VerifiedExecutableTransaction::new_unchecked(ExecutableTransaction::new_from_data_and_sig(
            tx,
            CertificateProof::new_system(0),
        )),
    )
}

#[cfg(test)]
mod tests {
    use iota_types::{
        base_types::{ObjectRef, SequenceNumber, TransactionDigest},
        crypto::{AccountKeyPair, get_key_pair},
        digests::ObjectDigest,
        move_authenticator::MoveAuthenticator,
        object::Object,
        signature::GenericSignature,
        transaction::{CallArg, SenderSignedData, SharedObjectRef, TransactionData},
    };

    use super::*;
    use crate::authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder};

    fn make_data(tx_data: TransactionData, signatures: Vec<GenericSignature>) -> SenderSignedData {
        SenderSignedData::new(tx_data, signatures)
    }

    fn random_gas() -> ObjectRef {
        ObjectRef::new(
            ObjectId::random(),
            SequenceNumber::from_u64(3),
            ObjectDigest::random(),
        )
    }

    /// A transaction whose sender authorizes with a plain signature (no
    /// `MoveAuthenticator` among the signatures).
    fn generate_plain_signed_tx(sender: Address) -> SenderSignedData {
        let tx_data = TransactionData::new(
            TransactionKind::Programmable(
                iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder::new()
                    .finish(),
            ),
            sender,
            random_gas(),
            10_000_000,
            1,
        );
        make_data(tx_data, vec![])
    }

    /// A transaction whose sender is authenticated by a `MoveAuthenticator`
    /// referencing the account object `account` at `version`.
    fn generate_move_authenticator_tx(
        account: ObjectId,
        version: SequenceNumber,
    ) -> SenderSignedData {
        let tx_data = TransactionData::new(
            TransactionKind::Programmable(
                iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder::new()
                    .finish(),
            ),
            account.into(),
            random_gas(),
            10_000_000,
            1,
        );
        let authenticator = MoveAuthenticator::new_v1(
            vec![],
            vec![],
            CallArg::Shared(SharedObjectRef::new(account, version, false)),
        );
        make_data(
            tx_data,
            vec![GenericSignature::MoveAuthenticator(authenticator)],
        )
    }

    fn claim_data() -> (ObjectId, SenderSignedData) {
        let (account, tx) = generate_claim_account_tx_with_gas_version(3);
        (account, tx.data().clone())
    }

    fn check(
        state: &AccountRulesState,
        authority: &AuthorityState,
        data: &SenderSignedData,
    ) -> Option<IotaError> {
        state
            .check_transaction(
                &authority.epoch_store_for_testing(),
                authority.get_object_cache_reader().as_ref(),
                data,
            )
            .unwrap()
    }

    #[tokio::test]
    async fn test_plain_signed_after_claim_in_same_commit_is_dropped() {
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, claim) = claim_data();
        let plain = generate_plain_signed_tx(account.into());

        assert!(check(&state, &authority, &claim).is_none());
        state.record_scheduled(&claim);

        assert!(matches!(
            check(&state, &authority, &plain),
            Some(IotaError::PlainSignatureForExplicitAccount { address }) if address == account.into()
        ));
    }

    #[tokio::test]
    async fn test_plain_signed_before_claim_in_same_commit_proceeds() {
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, claim) = claim_data();
        let plain = generate_plain_signed_tx(account.into());

        // The plain-signed transaction is checked at an earlier position of
        // the pass, before the claim is scheduled: it proceeds as implicit.
        assert!(check(&state, &authority, &plain).is_none());

        assert!(check(&state, &authority, &claim).is_none());
        state.record_scheduled(&claim);
    }

    #[tokio::test]
    async fn test_duplicate_claim_in_same_commit_is_dropped() {
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, first_claim) = claim_data();
        // A second claim for the same address with a distinct gas coin.
        let second_claim = {
            let mut data = first_claim.transaction_data().clone();
            iota_types::transaction::TransactionDataAPI::gas_data_mut(&mut data).objects =
                vec![random_gas()];
            make_data(data, vec![])
        };

        assert!(check(&state, &authority, &first_claim).is_none());
        state.record_scheduled(&first_claim);

        assert!(matches!(
            check(&state, &authority, &second_claim),
            Some(IotaError::AccountAlreadyExplicit { address }) if address == account.into()
        ));
    }

    #[tokio::test]
    async fn test_cancelled_claim_releases_address_and_propagates() {
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, claim) = claim_data();
        let plain = generate_plain_signed_tx(account.into());
        let authenticated = generate_move_authenticator_tx(account, SequenceNumber::from_u64(5));

        // The claim passes the account rules but is cancelled by the
        // congestion scheduling decision.
        assert!(check(&state, &authority, &claim).is_none());
        state.record_cancelled(&claim);

        // A plain-signed transaction for the same address proceeds as
        // implicit: the cancelled claim staged nothing.
        assert!(check(&state, &authority, &plain).is_none());
        // A MoveAuthenticator use of the account is dropped: the referenced
        // object can never come to exist.
        assert!(matches!(
            check(&state, &authority, &authenticated),
            Some(IotaError::DependencyOnCancelledClaim { address }) if address == account.into()
        ));
    }

    #[tokio::test]
    async fn test_claim_entry_from_earlier_commit_drops_plain_and_claim() {
        let authority = TestAuthorityBuilder::new().build().await;
        let state = AccountRulesState::new();
        let (account, claim) = claim_data();
        authority
            .epoch_store_for_testing()
            .insert_claimed_account_for_testing(
                account,
                TransactionDigest::random(),
                SequenceNumber::from_u64(4),
            );

        let plain = generate_plain_signed_tx(account.into());
        assert!(matches!(
            check(&state, &authority, &plain),
            Some(IotaError::PlainSignatureForExplicitAccount { .. })
        ));
        assert!(matches!(
            check(&state, &authority, &claim),
            Some(IotaError::AccountAlreadyExplicit { .. })
        ));
    }

    #[tokio::test]
    async fn test_settled_account_object_in_store_drops_plain() {
        let (sender, _): (Address, AccountKeyPair) = get_key_pair();
        let account_object = Object::with_id_owner_for_testing(sender.into(), Address::ZERO);
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(std::slice::from_ref(&account_object))
            .build()
            .await;
        let state = AccountRulesState::new();

        let plain = generate_plain_signed_tx(sender);
        assert!(matches!(
            check(&state, &authority, &plain),
            Some(IotaError::PlainSignatureForExplicitAccount { address }) if address == sender
        ));
    }
}
