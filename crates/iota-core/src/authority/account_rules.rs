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
//! The first two rules run inside post-consensus validation, per transaction
//! in consensus order and *before* the transaction acquires its owned-object
//! locks. A dropped transaction never executes, so a lock it took would hold
//! the object's current reference until the epoch ends; taking none keeps a
//! key the account rotated away from locking the account's objects with doomed
//! transactions at no cost. "Earlier in the commit" for these rules means kept
//! earlier in the validation loop: the scheduling pass may still defer or
//! cancel a kept claim, in which case a duplicate dropped against it simply
//! retries in a later commit.
//!
//! The third rule depends on the scheduling pass's own decisions, so it runs
//! there, before the transaction's congestion scheduling decision. With the
//! duplicate-claim guard ahead of it, a cancelled claim was the only claim kept
//! for its address in the commit and the address had neither a claim entry nor
//! an object, so no other claim covers the address: a `MoveAuthenticator`
//! naming it is dropped on the cancellation alone.
//!
//! Neither placement lets a doomed transaction take scheduling capacity, so
//! flooding the sequencer with duplicate claims or plain-signed transactions
//! for an explicit account cannot create artificial congestion. A dropped
//! transaction never reaches the version-assignment walk, so no version-chain
//! decision can disagree with the consensus order.
//!
//! The whole design is enabled only under the P-COOL flow, where every user
//! transaction is sequenced before it can execute: dropping a transaction
//! that may already have executed would be unsound, so in the certificate
//! flow the rules are inert and `ClaimAccount` transactions are not accepted
//! at all.
//!
//! Removed transactions are dropped deterministically and surfaced to clients
//! through the dropped-transaction status cache. The drops are final by
//! design, not pending an upgrade to cancellations with failure effects:
//! charging gas would honor the very authorization the rule rejected. A
//! plain-signed transaction (or duplicate claim) for an explicit account is
//! signed by a key that no longer speaks for the account, so charging its gas
//! would let a stale key drain an account that rotated it away; and a
//! transaction whose authenticator names a never-created account object has
//! no account to authenticate the charge against. The one path where a claim
//! is charged gas is execution-worker congestion cancelling it: there the
//! address is still implicit, so the plain signature authorizes the charge.

use std::collections::HashSet;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, ObjectId, SenderSignedTransaction, TransactionKind};
#[cfg(test)]
use iota_types::executable_transaction::VerifiedExecutableTransaction;
use iota_types::{
    error::{IotaError, IotaResult},
    transaction::TransactionAPI,
};

use crate::{authority::AuthorityPerEpochStore, execution_cache::ObjectCacheRead};

/// Returns the address a `ClaimAccount` transaction claims — its sender, which
/// is also the id of the account object the claim creates. `None` for every
/// other transaction kind.
pub(crate) fn account_address_being_claimed(data: &SenderSignedTransaction) -> Option<ObjectId> {
    match data.transaction().kind() {
        TransactionKind::ClaimAccount(_) => Some(data.transaction().sender().into()),
        _ => None,
    }
}

/// Returns the account addresses this transaction authorizes without a
/// `MoveAuthenticator` — the sender and the gas owner, minus any address a
/// `MoveAuthenticator` covers. These are the plain-signed sides.
fn non_move_authenticated_account_addresses(data: &SenderSignedTransaction) -> Vec<Address> {
    let transaction_data = data.transaction();
    let mut addresses = vec![transaction_data.sender()];
    let gas_owner = transaction_data.gas_owner();
    if gas_owner != transaction_data.sender() {
        addresses.push(gas_owner);
    }
    let authenticated: HashSet<Address> = move_authenticated_account_addresses(data)
        .map(Address::from)
        .collect();
    addresses.retain(|address| !authenticated.contains(address));
    addresses
}

/// Returns the ids of the accounts this transaction authenticates through
/// `MoveAuthenticator` signatures.
fn move_authenticated_account_addresses(
    data: &SenderSignedTransaction,
) -> impl Iterator<Item = ObjectId> + '_ {
    data.move_authenticators()
        .into_iter()
        .map(|authenticator| ObjectId::from(authenticator.address()))
}

/// Account-rules state threaded through one commit's post-consensus
/// validation: the claims kept at earlier positions of the loop.
#[derive(Default)]
pub(crate) struct AccountRulesState {
    /// Addresses claimed by a transaction kept earlier in this commit. The
    /// scheduling pass may still defer or cancel such a claim; a duplicate
    /// dropped against it retries in a later commit.
    kept_claims: HashSet<ObjectId>,
}

impl AccountRulesState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Checks one transaction against the duplicate-claim and plain-signature
    /// rules, at its position of the validation loop. Must be called *before*
    /// the transaction acquires its owned-object locks, so that a dropped
    /// transaction leaves none behind. Returns the rejection error if the
    /// transaction must be dropped.
    pub(crate) fn check_transaction(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        cache_reader: &dyn ObjectCacheRead,
        data: &SenderSignedTransaction,
    ) -> IotaResult<Option<IotaError>> {
        if !epoch_store
            .protocol_config()
            .enable_claim_account_transaction()
        {
            return Ok(None);
        }
        if data.transaction().is_system_tx() {
            return Ok(None);
        }

        let address_being_claimed = account_address_being_claimed(data);
        if let Some(address) = address_being_claimed {
            // The duplicate-claim guard: it prevents a second object with the
            // same id from ever being minted. The first scheduled claim wins.
            if self.resolve_explicit(epoch_store, cache_reader, &address)? {
                return Ok(Some(IotaError::AccountAlreadyExplicit {
                    address: address.into(),
                }));
            }
        }

        for address in non_move_authenticated_account_addresses(data) {
            let account_id = ObjectId::from(address);
            // The claim's own sender is decided by the duplicate-claim guard
            // above, not by the plain-signature rule.
            if address_being_claimed == Some(account_id) {
                continue;
            }
            if self.resolve_explicit(epoch_store, cache_reader, &account_id)? {
                return Ok(Some(IotaError::PlainSignatureForExplicitAccount {
                    address,
                }));
            }
        }

        Ok(None)
    }

    /// Records the claim of a transaction that validation kept. No-op for
    /// transactions that are not claims.
    pub(crate) fn record_kept(&mut self, data: &SenderSignedTransaction) {
        if let Some(address) = account_address_being_claimed(data) {
            self.kept_claims.insert(address);
        }
    }

    /// Answers "is the account at `address` explicit, as of this position of
    /// the validation loop?" identically on every validator.
    ///
    /// Consults, in order: claims kept earlier in this commit, claim
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
        if self.kept_claims.contains(address) {
            return Ok(true);
        }
        if epoch_store.get_claimed_account(address)?.is_some() {
            return Ok(true);
        }
        Ok(cache_reader.get_object(address).is_some())
    }
}

/// Claims the scheduling pass cancelled at earlier positions of one commit.
/// A cancelled claim stages nothing: the address stays implicit and claimable,
/// and no account object comes to exist for it in this commit.
#[derive(Default)]
pub(crate) struct CancelledClaims {
    addresses: HashSet<ObjectId>,
}

impl CancelledClaims {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Records the claim of a transaction the sequencer cancelled. No-op for
    /// transactions that are not claims.
    pub(crate) fn record(&mut self, data: &SenderSignedTransaction) {
        if let Some(address) = account_address_being_claimed(data) {
            self.addresses.insert(address);
        }
    }

    /// Returns the rejection error for a transaction whose `MoveAuthenticator`
    /// names an account whose claim was cancelled earlier in this commit: the
    /// referenced object can never come to exist. Must be called *before* the
    /// transaction's congestion scheduling decision.
    pub(crate) fn check(
        &self,
        config: &ProtocolConfig,
        data: &SenderSignedTransaction,
    ) -> Option<IotaError> {
        if self.addresses.is_empty()
            || !config.enable_claim_account_transaction()
            || data.transaction().is_system_tx()
        {
            return None;
        }
        move_authenticated_account_addresses(data)
            .find(|account_id| self.addresses.contains(account_id))
            .map(|account_id| IotaError::DependencyOnCancelledClaim {
                address: account_id.into(),
            })
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
        ClaimAccountTransaction, ObjectDigest, ObjectReference, SmartAccountBuildKind,
        SmartAccountClaim, Transaction, Version, crypto::PublicKey,
    };
    use iota_types::{
        crypto::{AccountPrivateKey, get_key_pair},
        executable_transaction::{CertificateProof, ExecutableTransaction},
    };

    let (sender, private_key): (Address, AccountPrivateKey) = get_key_pair();
    let claim = SmartAccountClaim::new(
        &PublicKey::Ed25519(private_key.public_key()),
        SmartAccountBuildKind::Mutable,
    );
    let kind =
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));
    let transaction = Transaction::new(
        kind,
        sender,
        ObjectReference::new(
            ObjectId::random(),
            Version::from(gas_object_version),
            ObjectDigest::random(),
        ),
        10_000_000,
        1,
    );
    let tx = SenderSignedTransaction::new(transaction, vec![]);
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
    use iota_sdk_types::{
        MoveAuthenticator, MoveAuthenticatorV1, ObjectDigest, ObjectReference,
        SharedObjectReference, Transaction, TransactionDigest, UserSignature, Version,
    };
    use iota_types::{
        crypto::{AccountPrivateKey, get_key_pair},
        object::Object,
    };

    use super::*;
    use crate::authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder};

    /// The plain-signature and propagation rules only drop transactions
    /// under the P-COOL flow; tests exercising them enable it.
    fn enable_claim_account_transaction() -> impl Drop {
        iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_claim_account_transaction_for_testing(true);
            config
        })
    }

    fn make_data(
        transaction: Transaction,
        signatures: Vec<UserSignature>,
    ) -> SenderSignedTransaction {
        SenderSignedTransaction::new(transaction, signatures)
    }

    fn random_gas() -> ObjectReference {
        ObjectReference::new(ObjectId::random(), Version::from(3), ObjectDigest::random())
    }

    /// A transaction whose sender authorizes with a plain signature (no
    /// `MoveAuthenticator` among the signatures).
    fn generate_plain_signed_tx(sender: Address) -> SenderSignedTransaction {
        let transaction = Transaction::new(
            TransactionKind::Programmable(
                iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder::new()
                    .finish(),
            ),
            sender,
            random_gas(),
            10_000_000,
            1,
        );
        make_data(transaction, vec![])
    }

    /// A transaction whose sender is authenticated by a `MoveAuthenticator`
    /// referencing the account object `account` at `version`.
    fn generate_move_authenticator_tx(
        account: ObjectId,
        version: Version,
    ) -> SenderSignedTransaction {
        let transaction = Transaction::new(
            TransactionKind::Programmable(
                iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder::new()
                    .finish(),
            ),
            account.into(),
            random_gas(),
            10_000_000,
            1,
        );
        let authenticator =
            MoveAuthenticator::V1(MoveAuthenticatorV1::new_with_shared_account_object(
                vec![],
                vec![],
                SharedObjectReference::new(account, version, false),
            ));
        make_data(
            transaction,
            vec![UserSignature::MoveAuthenticator(authenticator)],
        )
    }

    fn claim_data() -> (ObjectId, SenderSignedTransaction) {
        let (account, tx) = generate_claim_account_tx_with_gas_version(3);
        (account, tx.data().clone())
    }

    fn check(
        state: &AccountRulesState,
        authority: &AuthorityState,
        data: &SenderSignedTransaction,
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
        let _protocol_guard = enable_claim_account_transaction();
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, claim) = claim_data();
        let plain = generate_plain_signed_tx(account.into());

        assert!(check(&state, &authority, &claim).is_none());
        state.record_kept(&claim);

        assert!(matches!(
            check(&state, &authority, &plain),
            Some(IotaError::PlainSignatureForExplicitAccount { address }) if address == account.into()
        ));
    }

    #[tokio::test]
    async fn test_plain_signed_before_claim_in_same_commit_proceeds() {
        let _protocol_guard = enable_claim_account_transaction();
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, claim) = claim_data();
        let plain = generate_plain_signed_tx(account.into());

        // The plain-signed transaction is checked at an earlier position of
        // the pass, before the claim is scheduled: it proceeds as implicit.
        assert!(check(&state, &authority, &plain).is_none());

        assert!(check(&state, &authority, &claim).is_none());
        state.record_kept(&claim);
    }

    #[tokio::test]
    async fn test_duplicate_claim_in_same_commit_is_dropped() {
        let _protocol_guard = enable_claim_account_transaction();
        let authority = TestAuthorityBuilder::new().build().await;
        let mut state = AccountRulesState::new();
        let (account, first_claim) = claim_data();
        // A second claim for the same address with a distinct gas coin.
        let second_claim = {
            let mut transaction = first_claim.transaction().clone();
            transaction.gas_data_mut().objects = vec![random_gas()];
            make_data(transaction, vec![])
        };

        assert!(check(&state, &authority, &first_claim).is_none());
        state.record_kept(&first_claim);

        assert!(matches!(
            check(&state, &authority, &second_claim),
            Some(IotaError::AccountAlreadyExplicit { address }) if address == account.into()
        ));
    }

    #[tokio::test]
    async fn test_cancelled_claim_propagates_to_move_authenticator_uses() {
        let _protocol_guard = enable_claim_account_transaction();
        let authority = TestAuthorityBuilder::new().build().await;
        let epoch_store = authority.epoch_store_for_testing();
        let config = epoch_store.protocol_config();
        let mut cancelled = CancelledClaims::new();
        let (account, claim) = claim_data();
        let plain = generate_plain_signed_tx(account.into());
        let authenticated = generate_move_authenticator_tx(account, Version::from(5));

        assert!(cancelled.check(config, &authenticated).is_none());
        // The claim passed validation but is cancelled by the congestion
        // scheduling decision.
        cancelled.record(&claim);

        // A MoveAuthenticator use of the account is dropped: the referenced
        // object can never come to exist.
        assert!(matches!(
            cancelled.check(config, &authenticated),
            Some(IotaError::DependencyOnCancelledClaim { address }) if address == account.into()
        ));
        // A plain-signed transaction for the same address is not this rule's
        // concern: the cancelled claim staged nothing, so the address stays
        // implicit.
        assert!(cancelled.check(config, &plain).is_none());
    }

    #[tokio::test]
    async fn test_claim_entry_from_earlier_commit_drops_plain_and_claim() {
        let _protocol_guard = enable_claim_account_transaction();
        let authority = TestAuthorityBuilder::new().build().await;
        let state = AccountRulesState::new();
        let (account, claim) = claim_data();
        authority
            .epoch_store_for_testing()
            .insert_claimed_account_for_testing(
                account,
                TransactionDigest::random(),
                Version::from(4),
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
        let _protocol_guard = enable_claim_account_transaction();
        let (sender, _): (Address, AccountPrivateKey) = get_key_pair();
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
