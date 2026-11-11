// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Post-consensus validation of transactions that carry a lease: the first
//! one kept holds the lease, and every other transaction of the same sender
//! with the same lease is dropped.

use std::{collections::HashMap, sync::Arc};

use iota_sdk_types::{
    Address, ObjectId, ObjectReference, Transaction, TransactionDigest, TransactionExpiration,
    ValidWindow,
};
use iota_types::{
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair},
    error::IotaError,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, VerifiedTransaction},
    utils::to_sender_signed_transaction,
};

use crate::{
    authority::{
        AuthorityState,
        authority_per_epoch_store::{
            AuthorityPerEpochStore, consensus_quarantine::ConsensusCommitOutput,
        },
        lease::{LeaseHolder, LeaseKey},
        test_authority_builder::TestAuthorityBuilder,
    },
    consensus_handler::VerifiedSequencedConsensusTransaction,
    post_consensus_validation::{self, PostConsensusDecision},
};

const LEASE: [u8; 32] = [1; 32];
const OTHER_LEASE: [u8; 32] = [2; 32];

/// A sender with its key and the coins it owns.
struct Sender {
    address: Address,
    key: AccountPrivateKey,
    coins: Vec<ObjectReference>,
}

/// An authority with two senders, each owning three coins.
struct Setup {
    authority: Arc<AuthorityState>,
    epoch_store: Arc<AuthorityPerEpochStore>,
    alice: Sender,
    bob: Sender,
}

impl Setup {
    async fn new() -> Self {
        let owners: Vec<(Address, AccountPrivateKey)> = vec![get_key_pair(), get_key_pair()];
        let coin_ids: Vec<Vec<ObjectId>> = owners
            .iter()
            .map(|_| (0..3).map(|_| ObjectId::random()).collect())
            .collect();
        let objects: Vec<Object> = owners
            .iter()
            .zip(&coin_ids)
            .flat_map(|((address, _), ids)| {
                ids.iter()
                    .map(|id| Object::with_id_owner_for_testing(*id, *address))
            })
            .collect();
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(&objects)
            .build()
            .await;
        let epoch_store = authority.epoch_store_for_testing().clone();

        let mut senders = owners
            .into_iter()
            .zip(coin_ids)
            .map(|((address, key), ids)| {
                let coins = ids
                    .iter()
                    .map(|id| authority.get_object(id).unwrap().object_ref())
                    .collect();
                Sender {
                    address,
                    key,
                    coins,
                }
            });
        let alice = senders.next().unwrap();
        let bob = senders.next().unwrap();
        drop(senders);

        Self {
            authority,
            epoch_store,
            alice,
            bob,
        }
    }

    /// A transfer by `sender` paid with its coin `coin`, valid in the current
    /// epoch and the next one, that carries `lease` if it is some.
    fn transfer(
        &self,
        sender: &Sender,
        coin: usize,
        lease: Option<[u8; 32]>,
    ) -> VerifiedTransaction {
        let rgp = self.authority.reference_gas_price_for_testing().unwrap();
        let epoch = self.epoch_store.epoch();
        let mut data = Transaction::new_transfer_iota(
            dbg_addr(2),
            sender.address,
            None,
            sender.coins[coin],
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        *data.expiration_mut_for_testing() = TransactionExpiration::ValidWindow(ValidWindow {
            first_valid: epoch,
            last_valid: epoch + 1,
            genesis_hash: self.epoch_store.get_chain_identifier().digest(),
            note: 0,
            lease,
        });
        self.epoch_store
            .verify_transaction(to_sender_signed_transaction(data, &sender.key))
            .unwrap()
    }

    /// Validates `transactions` as consensus commit `commit_index`. Returns
    /// the digests kept, the transactions dropped with their errors, and the
    /// leases taken.
    async fn validate(
        &self,
        commit_index: u64,
        transactions: &[&VerifiedTransaction],
    ) -> (
        Vec<TransactionDigest>,
        Vec<(TransactionDigest, IotaError)>,
        HashMap<LeaseKey, LeaseHolder>,
    ) {
        let mut sequenced: Vec<_> = transactions
            .iter()
            .map(|transaction| sequenced((*transaction).clone()))
            .collect();
        let (dropped, _locks, leases, _digests) =
            post_consensus_validation::validate_and_resolve_conflicts_with_leases(
                &self.authority,
                &self.epoch_store,
                commit_index,
                &Default::default(),
                &mut sequenced,
            )
            .await
            .unwrap();

        let kept = sequenced
            .iter()
            .filter_map(|transaction| transaction.0.transaction.user_transaction_digest())
            .collect();
        let dropped = dropped
            .into_iter()
            .map(|(digest, error, decision)| {
                assert_eq!(decision, PostConsensusDecision::Dropped);
                (digest, error)
            })
            .collect();
        (kept, dropped, leases)
    }

    /// Records `leases` as taken in an earlier consensus commit of this
    /// epoch that is not flushed yet.
    fn take_in_an_earlier_commit(&self, leases: HashMap<LeaseKey, LeaseHolder>) {
        let mut output = ConsensusCommitOutput::default();
        output.set_leases(leases);
        output.set_default_commit_stats_for_testing();
        self.epoch_store.push_consensus_output_for_tests(output);
    }
}

fn sequenced(transaction: VerifiedTransaction) -> VerifiedSequencedConsensusTransaction {
    VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(
            iota_types::attestation::AttestedTransaction::new(transaction.into(), None),
        )),
        tracking_id: Default::default(),
    })
}

fn lease_conflict(sender: &Sender, lease: [u8; 32], holder: &VerifiedTransaction) -> IotaError {
    IotaError::LeaseConflict {
        sender: sender.address,
        lease,
        holder: *holder.digest(),
        last_valid: holder
            .data()
            .transaction()
            .expiration()
            .as_valid_window()
            .last_valid,
    }
}

#[tokio::test]
async fn the_first_transaction_with_a_lease_is_kept_and_the_second_is_dropped() {
    let setup = Setup::new().await;
    let first = setup.transfer(&setup.alice, 0, Some(LEASE));
    let second = setup.transfer(&setup.alice, 1, Some(LEASE));

    let (kept, dropped, leases) = setup.validate(1, &[&first, &second]).await;

    assert_eq!(kept, vec![*first.digest()]);
    assert_eq!(
        dropped,
        vec![(
            *second.digest(),
            lease_conflict(&setup.alice, LEASE, &first)
        )]
    );
    assert_eq!(
        leases,
        HashMap::from([(
            (setup.alice.address, LEASE),
            LeaseHolder {
                transaction: *first.digest(),
                last_valid: setup.epoch_store.epoch() + 1,
            }
        )])
    );
}

#[tokio::test]
async fn the_same_lease_of_another_sender_is_kept() {
    let setup = Setup::new().await;
    let of_alice = setup.transfer(&setup.alice, 0, Some(LEASE));
    let of_bob = setup.transfer(&setup.bob, 0, Some(LEASE));

    let (kept, dropped, leases) = setup.validate(1, &[&of_alice, &of_bob]).await;

    assert_eq!(kept, vec![*of_alice.digest(), *of_bob.digest()]);
    assert!(dropped.is_empty());
    assert_eq!(leases.len(), 2);
}

#[tokio::test]
async fn another_lease_of_the_same_sender_is_kept() {
    let setup = Setup::new().await;
    let first = setup.transfer(&setup.alice, 0, Some(LEASE));
    let second = setup.transfer(&setup.alice, 1, Some(OTHER_LEASE));
    let without_lease = setup.transfer(&setup.alice, 2, None);

    let (kept, dropped, leases) = setup.validate(1, &[&first, &second, &without_lease]).await;

    assert_eq!(
        kept,
        vec![*first.digest(), *second.digest(), *without_lease.digest()]
    );
    assert!(dropped.is_empty());
    assert_eq!(leases.len(), 2);
}

#[tokio::test]
async fn a_lease_taken_in_an_earlier_commit_drops_a_later_transaction() {
    let setup = Setup::new().await;
    let first = setup.transfer(&setup.alice, 0, Some(LEASE));
    let second = setup.transfer(&setup.alice, 1, Some(LEASE));

    let (_, _, leases) = setup.validate(1, &[&first]).await;
    setup.take_in_an_earlier_commit(leases);
    let (kept, dropped, leases) = setup.validate(2, &[&second]).await;

    assert!(kept.is_empty());
    assert_eq!(
        dropped,
        vec![(
            *second.digest(),
            lease_conflict(&setup.alice, LEASE, &first)
        )]
    );
    assert!(leases.is_empty());
}

#[tokio::test]
async fn a_transaction_is_not_dropped_for_the_lease_it_holds_itself() {
    let setup = Setup::new().await;
    let holder = setup.transfer(&setup.alice, 0, Some(LEASE));

    let (_, _, leases) = setup.validate(1, &[&holder]).await;
    setup.take_in_an_earlier_commit(leases);
    let (kept, dropped, _) = setup.validate(2, &[&holder]).await;

    assert_eq!(kept, vec![*holder.digest()]);
    assert!(dropped.is_empty());
}

#[tokio::test]
async fn a_transaction_dropped_for_another_reason_takes_no_lease() {
    let setup = Setup::new().await;
    // Both pay with the same coin, so the second one loses the coin's lock.
    let takes_the_coin = setup.transfer(&setup.alice, 0, None);
    let loses_the_coin = setup.transfer(&setup.alice, 0, Some(LEASE));
    let with_the_same_lease = setup.transfer(&setup.alice, 1, Some(LEASE));

    let (kept, dropped, leases) = setup
        .validate(1, &[&takes_the_coin, &loses_the_coin, &with_the_same_lease])
        .await;

    assert_eq!(
        kept,
        vec![*takes_the_coin.digest(), *with_the_same_lease.digest()]
    );
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, *loses_the_coin.digest());
    assert!(matches!(dropped[0].1, IotaError::ObjectLockConflict { .. }));
    assert_eq!(
        leases[&(setup.alice.address, LEASE)].transaction,
        *with_the_same_lease.digest()
    );
}
