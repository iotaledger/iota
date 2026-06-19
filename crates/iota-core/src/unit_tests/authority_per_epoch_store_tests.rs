// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_protocol_config::PerObjectCongestionControlMode;
use iota_sdk_types::{Address, DenyRuleSet, ObjectId, TransactionDigest, Version};
use iota_types::{
    attestation::{Attestation, AttestationData},
    base_types::AuthorityName,
    committee::Committee,
    crypto::KeypairTraits,
    messages_consensus::TransactionDenyRuleProposal,
    transaction::SenderSignedTransactionAPI,
};
use tokio::time::timeout;
use typed_store::{Map, rocks::DBBatch};

use crate::{
    authority::{
        authority_per_epoch_store::{
            AuthorityPerEpochStore, CongestionControlParameters, compute_deny_rule_update_chunks,
            consensus_quarantine::ConsensusCommitOutput,
        },
        shared_object_congestion_tracker::{
            SequencingResult, SharedObjectCongestionTracker,
            shared_object_test_utils::{TEST_ONLY_GAS_PRICE, build_transaction},
        },
        test_authority_builder::TestAuthorityBuilder,
    },
    execution_scheduler::transaction_manager::VerifiedExecutableAttestedTransaction,
};

/// Records an overload notification through the same path
/// `process_consensus_transaction` uses: buffer it in `ConsensusCommitOutput`
/// and flush via `write_to_batch`. This is the only sanctioned way to populate
/// `authority_overload_notifications` outside the consensus loop.
fn flush_overload_notification(
    store: &AuthorityPerEpochStore,
    authority: AuthorityName,
    percentage: u8,
) {
    let mut output = ConsensusCommitOutput::new(0);
    output.record_overload_notification(authority, percentage);
    output.set_default_commit_stats_for_testing();
    let mut batch: DBBatch = store.db_batch_for_test();
    output.write_to_batch(store, &mut batch).unwrap();
    batch.write().unwrap();
    // The commit flush loop updates the in-memory cache in lockstep with the
    // table write; this helper writes the table directly, so update the cache
    // explicitly to keep the two consistent.
    store.apply_overload_notification_to_cache_for_test(authority, percentage);
}

#[tokio::test]
async fn test_notify_read_executed_transactions_to_checkpoint() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let checkpoint_sequence_1 = 10;
    let checkpoint_sequence_2 = 12;

    let txes_to_be_notified = vec![
        TransactionDigest::random(),
        TransactionDigest::random(),
        TransactionDigest::random(),
    ];

    // Insert only the first transaction already
    store
        .insert_finalized_transactions(
            vec![txes_to_be_notified[0]].as_slice(),
            checkpoint_sequence_1,
            0,
        )
        .expect("Should not fail");

    // Now register to get notified for the addition of some of the above
    // transactions
    let txes_to_be_notified_cloned = txes_to_be_notified.clone();
    let handle = tokio::spawn(async move {
        let notify = store.transactions_executed_in_checkpoint_notify(txes_to_be_notified_cloned);
        notify.await
    });

    // Now insert the rest of the transactions
    let store = authority_state.epoch_store_for_testing();
    store
        .insert_finalized_transactions(&txes_to_be_notified[1..], checkpoint_sequence_2, 0)
        .expect("Should not fail");

    // We should get notified about all the transactions having been executed via
    // checkpoints
    let _ = timeout(Duration::from_secs(5), handle)
        .await
        .expect("Should not timeout")
        .expect("Should not fail");

    // And the transactions should be found into the table
    let result = store
        .multi_get_transaction_checkpoint(txes_to_be_notified.as_slice())
        .expect("Should not fail");
    assert_eq!(result.len(), txes_to_be_notified.len());

    assert_eq!(result[0].unwrap(), checkpoint_sequence_1);
    assert_eq!(result[1].unwrap(), checkpoint_sequence_2);
    assert_eq!(result[2].unwrap(), checkpoint_sequence_2);
}

#[tokio::test]
async fn wait_for_transactions_in_checkpoint_returns_promptly_on_notify() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();

    // Two digests: the first is already in the DB before the wait starts and
    // should resolve via the `get_timestamp` closure; the second is inserted
    // after the wait registers and should resolve via the notification path.
    let preexisting_digest = TransactionDigest::random();
    let pending_digest = TransactionDigest::random();
    let preexisting_seq = 3;
    let preexisting_ts_via_closure = 42;
    let pending_seq = 7;
    let pending_ts = 1_700_000_000_000;

    // Pre-populate the first digest before the wait registers.
    store
        .insert_finalized_transactions(&[preexisting_digest], preexisting_seq, 0)
        .expect("insert_finalized_transactions should succeed");

    let waiter_store = store.clone();
    let waiter_digests = vec![preexisting_digest, pending_digest];
    let waiter = tokio::spawn(async move {
        waiter_store
            .wait_for_transactions_in_checkpoint_with_timeout(
                &waiter_digests,
                Duration::from_secs(30),
                |_seq| preexisting_ts_via_closure,
            )
            .await
    });

    // Give the waiter a moment to register before firing the notification.
    tokio::time::sleep(Duration::from_millis(50)).await;
    store
        .insert_finalized_transactions(&[pending_digest], pending_seq, pending_ts)
        .expect("insert_finalized_transactions should succeed");

    // With the bug, this hangs for the full 30s timeout and trips the outer 2s
    // timeout; with the fix, the call returns in milliseconds.
    let results = timeout(Duration::from_secs(2), waiter)
        .await
        .expect("wait did not return promptly after notification")
        .expect("waiter task panicked")
        .expect("wait_for_transactions_in_checkpoint_with_timeout returned error");

    assert_eq!(results.len(), 2);
    // Already-checkpointed: timestamp resolved by closure.
    assert_eq!(
        results[0],
        Some((preexisting_seq, preexisting_ts_via_closure))
    );
    // Notified during the wait: timestamp comes from the notification payload.
    assert_eq!(results[1], Some((pending_seq, pending_ts)));
}

#[tokio::test]
async fn wait_for_transactions_in_checkpoint_times_out_without_notify() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();

    let digests = vec![TransactionDigest::random(), TransactionDigest::random()];
    let wait_timeout = Duration::from_millis(200);

    let started = Instant::now();
    let results = store
        .wait_for_transactions_in_checkpoint_with_timeout(&digests, wait_timeout, |_seq| 0)
        .await
        .expect("wait_for_transactions_in_checkpoint_with_timeout returned error");
    let elapsed = started.elapsed();

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(Option::is_none));
    assert!(
        elapsed >= wait_timeout,
        "expected to wait at least the full timeout, waited {elapsed:?}"
    );
    assert!(
        elapsed < wait_timeout * 5,
        "wait took unreasonably long: {elapsed:?}"
    );
}

#[tokio::test]
async fn wait_for_checkpoint_inclusion_resolves_across_reconfiguration() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let digest = TransactionDigest::random();
    let seq = 5;
    let ts = 1_700_000_000_000;

    let state = authority_state.clone();
    let waiter = tokio::spawn(async move {
        state
            .wait_for_checkpoint_inclusion(&[digest], Duration::from_secs(30))
            .await
    });
    // Let the waiter register on the current (soon-to-be-old) epoch store.
    tokio::time::sleep(Duration::from_millis(50)).await;

    authority_state.reconfigure_for_testing().await;
    // Let the waiter hop to the new epoch store and register there, so the
    // mapping below is delivered through the notification path (which carries
    // the timestamp; the DB-read path would fall back to a checkpoint summary
    // lookup that has nothing to find in this test).
    tokio::time::sleep(Duration::from_millis(50)).await;

    // The mapping lands in the next epoch's store, as happens for a transaction
    // that is still uncheckpointed when the epoch ends.
    authority_state
        .epoch_store_for_testing()
        .insert_finalized_transactions(&[digest], seq, ts)
        .expect("insert_finalized_transactions should succeed");

    let results = timeout(Duration::from_secs(5), waiter)
        .await
        .expect("wait did not resolve promptly after reconfiguration")
        .expect("waiter task panicked")
        .expect("wait_for_checkpoint_inclusion returned error");
    assert_eq!(results.get(&digest), Some(&(seq, ts)));
}

#[tokio::test]
async fn wait_for_checkpoint_inclusion_recovers_mapping_from_old_epoch_store() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let old_store = authority_state.epoch_store_for_testing().clone();
    let digest = TransactionDigest::random();
    let seq = 5;

    let state = authority_state.clone();
    let waiter = tokio::spawn(async move {
        state
            .wait_for_checkpoint_inclusion(&[digest], Duration::from_secs(30))
            .await
    });
    // Let the waiter register on the current (soon-to-be-old) epoch store.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Write the mapping directly to the old store's table without firing the
    // notification, modeling a notification the waiter raced with at the
    // boundary. The waiter must recover it by re-reading the old store's
    // table after epoch termination.
    let mut batch = old_store.db_batch_for_test();
    batch
        .insert_batch(
            &old_store
                .tables()
                .expect("old epoch store tables should still be open")
                .executed_transactions_to_checkpoint,
            [(digest, seq)],
        )
        .expect("insert_batch should succeed");
    batch.write().expect("batch write should succeed");

    authority_state.reconfigure_for_testing().await;

    let results = timeout(Duration::from_secs(5), waiter)
        .await
        .expect("wait did not resolve promptly after reconfiguration")
        .expect("waiter task panicked")
        .expect("wait_for_checkpoint_inclusion returned error");
    // No checkpoint summary exists in this test, so the timestamp resolves
    // to the 0 fallback.
    assert_eq!(results.get(&digest), Some(&(seq, 0)));
}

/// Persisted overload notifications must round-trip through
/// `ConsensusCommitOutput::record_overload_notification` (flushed via
/// `write_to_batch`) -> `load_overload_notifications`. Re-recording in a
/// subsequent commit overwrites the previous percentage.
#[tokio::test]
async fn test_load_overload_notifications_round_trip() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    assert!(
        store.load_overload_notifications().unwrap().is_empty(),
        "no notifications recorded yet",
    );

    flush_overload_notification(&store, me, 25);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(25),
    );

    // A subsequent commit's recorder call from the same authority overwrites
    // the prior value once flushed.
    flush_overload_notification(&store, me, 75);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&me)
            .copied(),
        Some(75),
    );
}

/// `compute_quorum_load_shedding_percentage` must read the percentile from
/// the supplied map without consulting the DB. With a single-authority test
/// committee the 2f+1 quorum value is just that authority's reported value
/// (or 0 when absent). This is the same code path used to apply in-batch
/// `OverloadNotificationV1` overrides on top of the persisted map before
/// dropping user transactions.
#[tokio::test]
async fn test_compute_quorum_load_shedding_percentage_uses_overlay() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    // Empty overlay -> 0%.
    let empty: HashMap<_, _> = HashMap::new();
    assert_eq!(store.compute_quorum_load_shedding_percentage(&empty), 0);

    // Overlay-only value is reflected without touching the DB.
    let mut overlay = HashMap::new();
    overlay.insert(me, 60);
    assert_eq!(store.compute_quorum_load_shedding_percentage(&overlay), 60);

    // Persist a different value and confirm the overlay wins, mirroring the
    // last-writer-wins behavior of the pre-pass merge step.
    flush_overload_notification(&store, me, 10);
    let mut merged = store.load_overload_notifications().unwrap();
    merged.insert(me, 90);
    assert_eq!(store.compute_quorum_load_shedding_percentage(&merged), 90);

    // Without the overlay, only the persisted value is visible.
    assert_eq!(store.get_quorum_load_shedding_percentage().unwrap(), 10);
}
/// `load_overload_notifications` must return the same notification map
/// whether a given authority's latest reported percentage lives in the
/// persisted `authority_overload_notifications` DBMap or in a still-queued
/// `ConsensusCommitOutput` inside `ConsensusOutputQuarantine`. The "logical
/// state" of overload notifications is the union of (a) everything flushed to
/// disk so far and (b) everything processed but not yet flushed; the read
/// path must surface that union, not just (a).
///
/// This invariance under the disk/queue split is what makes the
/// post-consensus drop decision deterministic: the same set of notifications
/// fed into `compute_quorum_load_shedding_percentage` regardless of where the
/// flush boundary happens to fall when the read is taken.
///
/// The test reads a value through path (b) — a queued, not-yet-flushed commit —
/// and through path (a) — held fully on disk with an empty quarantine, on a
/// second independent authority that models the freshly started / drained
/// state. Both reads must agree, and the derived
/// `get_quorum_load_shedding_percentage` must match. A single authority can't
/// test path (a) cleanly: its queued entry lingers, making the read a tautology
/// of disk overlaid by the identical queued value.
#[tokio::test]
async fn test_load_overload_notifications_invariant_under_disk_queue_split() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let authority_name = store.name;

    // Disk holds an earlier value for the authority.
    flush_overload_notification(&store, authority_name, 30);
    assert_eq!(
        store
            .load_overload_notifications()
            .unwrap()
            .get(&authority_name)
            .copied(),
        Some(30),
    );

    // A later commit has been processed and its output is sitting in the
    // quarantine queue, *not* yet flushed. This is the common steady-state
    // condition while consensus runs ahead of the checkpoint executor —
    // potentially many commits queue up before any of them flush.
    let mut later = ConsensusCommitOutput::default();
    later.record_overload_notification(authority_name, 80);
    later.set_default_commit_stats_for_testing();
    store.push_consensus_output_for_tests(later);

    // The read that drives the post-consensus drop decision must see the
    // queued value, not the stale on-disk one.
    let from_queue = store
        .load_overload_notifications()
        .unwrap()
        .get(&authority_name)
        .copied();
    assert_eq!(
        from_queue,
        Some(80),
        "queued notifications must be visible via load_overload_notifications",
    );

    // The other end of the split: the same logical state held fully on disk
    // with nothing queued, as a freshly started authority sees it once the
    // queued commit has drained. A second, independent authority — empty
    // quarantine — with 80 persisted must read back the same value as the
    // queued case above. (Reusing the first authority would not test this:
    // its queued entry lingers, so the read would be disk(80)
    // overlaid by queue(80).)
    let fresh_state = TestAuthorityBuilder::new().build().await;
    let fresh_store = fresh_state.epoch_store_for_testing();
    let fresh_authority = fresh_store.name;
    flush_overload_notification(&fresh_store, fresh_authority, 80);

    let from_disk = fresh_store
        .load_overload_notifications()
        .unwrap()
        .get(&fresh_authority)
        .copied();
    assert_eq!(
        from_queue, from_disk,
        "load_overload_notifications must be invariant under the disk/queue split",
    );

    // The derived quorum percentage must agree with the disk-only view.
    assert_eq!(
        fresh_store.get_quorum_load_shedding_percentage().unwrap(),
        80,
    );
}

fn deny_proposal(
    authority: AuthorityName,
    generation: u64,
    proposed_rules: DenyRuleSet,
) -> TransactionDenyRuleProposal {
    TransactionDenyRuleProposal {
        authority,
        generation,
        proposed_rules,
    }
}

fn rules_denying_address(address: Address) -> DenyRuleSet {
    DenyRuleSet {
        denied_addresses: [address].into(),
        ..Default::default()
    }
}

/// Records a deny rule proposal through the same path
/// `process_consensus_transaction` will use: buffer it in
/// `ConsensusCommitOutput` and flush via `write_to_batch`. See
/// `flush_overload_notification`.
fn flush_deny_rule_proposal(store: &AuthorityPerEpochStore, proposal: TransactionDenyRuleProposal) {
    let mut output = ConsensusCommitOutput::new(0);
    output.record_deny_rule_proposal(proposal.clone());
    output.set_default_commit_stats_for_testing();
    let mut batch: DBBatch = store.db_batch_for_test();
    output.write_to_batch(store, &mut batch).unwrap();
    batch.write().unwrap();
    store.apply_deny_rule_proposal_to_cache_for_test(proposal);
}

/// Deny list entries activate at f+1 supporting stake, kill switches at 2f+1,
/// and non-members contribute nothing. With the 4-validator equal-stake test
/// committee (2500 each, normalized to 10000): 2 supporters = 5000 clears
/// f+1 (3334) but not 2f+1 (6667); 3 supporters = 7500 clears both.
#[test]
fn compute_active_transaction_deny_rules_applies_stake_thresholds() {
    let (committee, keys) = Committee::new_simple_test_committee();
    let names: Vec<AuthorityName> = keys.iter().map(|key| key.public().into()).collect();
    let active_address = Address::new([1u8; 32]);
    let minority_address = Address::new([2u8; 32]);
    let non_member_address = Address::new([3u8; 32]);
    let active_object = ObjectId::new([4u8; 32]);
    let minority_object = ObjectId::new([5u8; 32]);
    let active_package = ObjectId::new([6u8; 32]);
    let minority_package = ObjectId::new([7u8; 32]);

    let mut proposals: BTreeMap<AuthorityName, TransactionDenyRuleProposal> = BTreeMap::new();
    // Two members support the `active_*` entries; switch support ranges from
    // 3 members (receiving/move authenticator) through 2 (shared object,
    // package publish) and 1 (user transaction) to 0 (package upgrade).
    proposals.insert(
        names[0],
        deny_proposal(
            names[0],
            1,
            DenyRuleSet {
                denied_addresses: [active_address, minority_address].into(),
                denied_objects: [active_object, minority_object].into(),
                denied_packages: [active_package, minority_package].into(),
                shared_object_disabled: true,
                user_transaction_disabled: true,
                package_publish_disabled: true,
                receiving_objects_disabled: true,
                move_authenticator_disabled: true,
                ..Default::default()
            },
        ),
    );
    proposals.insert(
        names[1],
        deny_proposal(
            names[1],
            1,
            DenyRuleSet {
                denied_addresses: [active_address].into(),
                denied_objects: [active_object].into(),
                denied_packages: [active_package].into(),
                shared_object_disabled: true,
                package_publish_disabled: true,
                receiving_objects_disabled: true,
                move_authenticator_disabled: true,
                ..Default::default()
            },
        ),
    );
    proposals.insert(
        names[2],
        deny_proposal(
            names[2],
            1,
            DenyRuleSet {
                receiving_objects_disabled: true,
                move_authenticator_disabled: true,
                ..Default::default()
            },
        ),
    );
    // A non-member's proposal must weigh 0.
    let non_member = AuthorityName::ZERO;
    proposals.insert(
        non_member,
        deny_proposal(
            non_member,
            1,
            DenyRuleSet {
                denied_addresses: [non_member_address, minority_address].into(),
                denied_objects: [minority_object].into(),
                denied_packages: [minority_package].into(),
                user_transaction_disabled: true,
                package_upgrade_disabled: true,
                ..Default::default()
            },
        ),
    );

    let active =
        AuthorityPerEpochStore::compute_active_transaction_deny_rules(&proposals, &committee);
    // Deny lists: 5000 >= f+1 active; 2500 (+0 from the non-member) < f+1.
    assert!(active.denied_addresses.contains(&active_address));
    assert!(!active.denied_addresses.contains(&minority_address));
    assert!(!active.denied_addresses.contains(&non_member_address));
    assert!(active.denied_objects.contains(&active_object));
    assert!(!active.denied_objects.contains(&minority_object));
    assert!(active.denied_packages.contains(&active_package));
    assert!(!active.denied_packages.contains(&minority_package));
    // Kill switches: 7500 >= 2f+1 on; 5000, 2500 (+0), and 0 stay off.
    assert!(active.receiving_objects_disabled);
    assert!(active.move_authenticator_disabled);
    assert!(!active.shared_object_disabled);
    assert!(!active.user_transaction_disabled);
    assert!(!active.package_publish_disabled);
    assert!(!active.package_upgrade_disabled);
}

/// The enforcement union keeps every mirrored on-chain entry and switch
/// active regardless of the proposal-derived aggregate, so rules carry
/// across the epoch boundary before supporters re-announce.
#[test]
fn union_deny_rule_sets_keeps_mirrored_state_active() {
    use crate::authority::authority_per_epoch_store::union_deny_rule_sets;

    // Every list gets a mirrored-only, an aggregate-only, and a shared entry.
    // Each side contributes three of the six switches.
    let shared = Address::new([1u8; 32]);
    let mirrored = DenyRuleSet {
        denied_addresses: [shared, Address::new([2u8; 32])].into(),
        denied_objects: [ObjectId::new([3u8; 32])].into(),
        denied_packages: [ObjectId::new([4u8; 32])].into(),
        package_publish_disabled: true,
        shared_object_disabled: true,
        receiving_objects_disabled: true,
        ..Default::default()
    };
    let aggregate = DenyRuleSet {
        denied_addresses: [shared, Address::new([5u8; 32])].into(),
        denied_objects: [ObjectId::new([6u8; 32])].into(),
        denied_packages: [ObjectId::new([7u8; 32])].into(),
        package_upgrade_disabled: true,
        user_transaction_disabled: true,
        move_authenticator_disabled: true,
        ..Default::default()
    };

    let active = union_deny_rule_sets(aggregate, &mirrored);
    assert_eq!(
        active.denied_addresses,
        [shared, Address::new([2u8; 32]), Address::new([5u8; 32])].into()
    );
    assert_eq!(
        active.denied_objects,
        [ObjectId::new([3u8; 32]), ObjectId::new([6u8; 32])].into()
    );
    assert_eq!(
        active.denied_packages,
        [ObjectId::new([4u8; 32]), ObjectId::new([7u8; 32])].into()
    );
    assert!(active.package_publish_disabled);
    assert!(active.package_upgrade_disabled);
    assert!(active.shared_object_disabled);
    assert!(active.user_transaction_disabled);
    assert!(active.receiving_objects_disabled);
    assert!(active.move_authenticator_disabled);

    // An empty aggregate changes nothing. A fresh epoch starts this way.
    // This also pins the switches the mirror leaves off.
    let active = union_deny_rule_sets(DenyRuleSet::default(), &mirrored);
    assert_eq!(active, mirrored);
}

/// `announced_deny_rule_stake` sums the committee stake behind recorded
/// proposals: empty proposals count as announcements, non-members weigh 0.
#[tokio::test]
async fn announced_deny_rule_stake_counts_recorded_proposals() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    assert_eq!(store.announced_deny_rule_stake(), 0);

    // A non-member's recorded proposal contributes no stake.
    flush_deny_rule_proposal(
        &store,
        deny_proposal(AuthorityName::ZERO, 1, DenyRuleSet::default()),
    );
    assert_eq!(store.announced_deny_rule_stake(), 0);

    // An empty proposal is an announcement: the single-validator test
    // committee holds all stake.
    flush_deny_rule_proposal(&store, deny_proposal(me, 1, DenyRuleSet::default()));
    assert_eq!(
        store.announced_deny_rule_stake(),
        store.committee().total_votes()
    );
}

/// A proposal replaces the recorded one from the same authority only when its
/// generation is strictly newer, both across commits
/// (`should_record_deny_rule_proposal`) and within one commit
/// (`ConsensusCommitOutput::record_deny_rule_proposal`).
#[tokio::test]
async fn deny_rule_proposals_dedup_by_strictly_newer_generation() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;
    let old_rules = rules_denying_address(Address::new([1u8; 32]));
    let new_rules = rules_denying_address(Address::new([2u8; 32]));

    flush_deny_rule_proposal(&store, deny_proposal(me, 5, old_rules.clone()));
    assert!(!store.should_record_deny_rule_proposal(&deny_proposal(me, 3, new_rules.clone())));
    assert!(!store.should_record_deny_rule_proposal(&deny_proposal(me, 5, new_rules.clone())));
    assert!(store.should_record_deny_rule_proposal(&deny_proposal(me, 6, new_rules.clone())));

    // Within one commit the output itself keeps only the newest generation.
    let mut output = ConsensusCommitOutput::default();
    output.record_deny_rule_proposal(deny_proposal(me, 8, new_rules.clone()));
    output.record_deny_rule_proposal(deny_proposal(me, 7, old_rules.clone()));
    output.set_default_commit_stats_for_testing();
    store.push_consensus_output_for_tests(output);

    let recorded = store.load_deny_rule_proposals().unwrap();
    assert_eq!(recorded.get(&me).unwrap().generation, 8);
    assert_eq!(recorded.get(&me).unwrap().proposed_rules, new_rules);

    // The staleness check must see the queued generation (8), not the flushed
    // one (5).
    assert!(!store.should_record_deny_rule_proposal(&deny_proposal(me, 7, new_rules.clone())));
    assert!(!store.should_record_deny_rule_proposal(&deny_proposal(me, 8, new_rules.clone())));
    assert!(store.should_record_deny_rule_proposal(&deny_proposal(me, 9, new_rules.clone())));

    // A newer generation recorded after an older one replaces it.
    let mut output = ConsensusCommitOutput::default();
    output.record_deny_rule_proposal(deny_proposal(me, 9, old_rules));
    output.record_deny_rule_proposal(deny_proposal(me, 10, new_rules.clone()));
    output.set_default_commit_stats_for_testing();
    store.push_consensus_output_for_tests(output);

    let recorded = store.load_deny_rule_proposals().unwrap();
    assert_eq!(recorded.get(&me).unwrap().generation, 10);
    assert_eq!(recorded.get(&me).unwrap().proposed_rules, new_rules);
}

/// `update_active_transaction_deny_rules` recomputes from the recorded
/// proposals and reports whether the active set changed. A newer full-state
/// proposal that omits an entry withdraws that vote, deactivating the entry
/// (drop-to-remove).
#[tokio::test]
async fn update_active_transaction_deny_rules_drops_withdrawn_rules() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    // The single-validator test committee holds all stake, clearing both
    // thresholds alone.
    let me = store.name;
    let address = Address::new([1u8; 32]);

    assert_eq!(
        *store.get_active_transaction_deny_rules(),
        DenyRuleSet::default()
    );
    assert!(!store.update_active_transaction_deny_rules().unwrap());

    flush_deny_rule_proposal(
        &store,
        deny_proposal(
            me,
            1,
            DenyRuleSet {
                denied_addresses: [address].into(),
                user_transaction_disabled: true,
                ..Default::default()
            },
        ),
    );
    assert!(store.update_active_transaction_deny_rules().unwrap());
    let active = store.get_active_transaction_deny_rules();
    assert!(active.denied_addresses.contains(&address));
    assert!(active.user_transaction_disabled);

    // Newer proposal without the switch withdraws it; the address stays.
    flush_deny_rule_proposal(&store, deny_proposal(me, 2, rules_denying_address(address)));
    assert!(store.update_active_transaction_deny_rules().unwrap());
    let active = store.get_active_transaction_deny_rules();
    assert!(active.denied_addresses.contains(&address));
    assert!(!active.user_transaction_disabled);

    // An empty proposal withdraws everything.
    flush_deny_rule_proposal(&store, deny_proposal(me, 3, DenyRuleSet::default()));
    assert!(store.update_active_transaction_deny_rules().unwrap());
    assert_eq!(
        *store.get_active_transaction_deny_rules(),
        DenyRuleSet::default()
    );
    assert!(!store.update_active_transaction_deny_rules().unwrap());
}

/// `load_deny_rule_proposals` must return the same map whether an authority's
/// newest proposal lives in the persisted `deny_rule_proposals` DBMap or in a
/// still-queued `ConsensusCommitOutput`. See
/// `test_load_overload_notifications_invariant_under_disk_queue_split`.
#[tokio::test]
async fn load_deny_rule_proposals_invariant_under_disk_queue_split() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;

    // Disk holds an earlier proposal.
    flush_deny_rule_proposal(
        &store,
        deny_proposal(me, 1, rules_denying_address(Address::new([1u8; 32]))),
    );

    // A later commit's proposal is queued, not yet flushed; the read must
    // surface it over the stale on-disk one.
    let queued_rules = rules_denying_address(Address::new([2u8; 32]));
    let mut later = ConsensusCommitOutput::default();
    later.record_deny_rule_proposal(deny_proposal(me, 2, queued_rules.clone()));
    later.set_default_commit_stats_for_testing();
    store.push_consensus_output_for_tests(later);

    let from_queue = store.load_deny_rule_proposals().unwrap();
    assert_eq!(from_queue.get(&me).unwrap().generation, 2);
    assert_eq!(from_queue.get(&me).unwrap().proposed_rules, queued_rules);

    // The same logical state held fully on disk with nothing queued, as a
    // freshly started authority sees it.
    let fresh_state = TestAuthorityBuilder::new().build().await;
    let fresh_store = fresh_state.epoch_store_for_testing();
    let fresh_me = fresh_store.name;
    flush_deny_rule_proposal(
        &fresh_store,
        deny_proposal(fresh_me, 2, queued_rules.clone()),
    );

    let from_disk = fresh_store.load_deny_rule_proposals().unwrap();
    assert_eq!(from_disk.get(&fresh_me).unwrap().generation, 2);
    assert_eq!(
        from_disk.get(&fresh_me).unwrap().proposed_rules,
        queued_rules
    );
}

/// A restart must resume with the persisted deny rules: reopening the epoch
/// store over the same DB re-seeds the proposal cache and derives the active
/// set from the `deny_rule_proposals` table alone, with no cache priming or
/// update call.
#[tokio::test]
async fn restart_seed_restores_persisted_deny_rules() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;
    let address = Address::new([1u8; 32]);
    let rules = DenyRuleSet {
        denied_addresses: [address].into(),
        user_transaction_disabled: true,
        ..Default::default()
    };
    flush_deny_rule_proposal(&store, deny_proposal(me, 1, rules.clone()));

    // Reopen over the same epoch DB, as a restart does.
    store.release_db_handles();
    let reopened = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        (*store.epoch_start_configuration).clone(),
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();

    let persisted = reopened.load_deny_rule_proposals().unwrap();
    assert_eq!(persisted.get(&me).unwrap().generation, 1);
    assert_eq!(persisted.get(&me).unwrap().proposed_rules, rules);
    let active = reopened.get_active_transaction_deny_rules();
    assert!(active.denied_addresses.contains(&address));
    assert!(active.user_transaction_disabled);
}

/// Under `TotalComputationUnits`, the estimated execution duration is the
/// attested computation units when present, and `gas_budget / gas_price`
/// otherwise.
#[test]
fn test_get_estimated_execution_duration_total_computation_units_mode() {
    let params = CongestionControlParameters::new_for_test(
        PerObjectCongestionControlMode::TotalComputationUnits,
        false,           // congestion_control_min_free_execution_slot
        Some(1_000_000), // max_execution_duration_per_commit
        Some(0),         // max_congestion_limit_overshoot_per_commit
        0,               // max_gas_price (irrelevant here)
        false,           // use_congestion_limit_overshoot_in_gas_price_feedback_mechanism
        true,            // use_separate_gas_price_feedback_mechanism_for_randomness
    );

    let gas_budget = 12_345;
    let attested_units = 9_876;

    // Attested transaction returns its attested computation units.
    let attested_tx = attest(
        build_transaction(&[], gas_budget, TEST_ONLY_GAS_PRICE),
        attested_units,
    );
    assert_eq!(
        params.get_estimated_execution_duration(&attested_tx),
        attested_units,
    );

    // Unattested transaction falls back to gas_budget converted to gas units.
    let unattested_tx = build_transaction(&[], gas_budget, TEST_ONLY_GAS_PRICE);
    assert_eq!(
        params.get_estimated_execution_duration(&unattested_tx),
        gas_budget / TEST_ONLY_GAS_PRICE,
    );

    // Unattested transaction with a zero gas price: the `gas_budget / gas_price`
    // fallback must not divide by zero.
    let zero_gas_price_tx = build_transaction(&[], gas_budget, 0);
    assert_eq!(
        params.get_estimated_execution_duration(&zero_gas_price_tx),
        0,
    );

    // An attestation with zero computation units should not use the gas-budget
    // fallback.
    let zero_cost_attested_tx = attest(build_transaction(&[], gas_budget, TEST_ONLY_GAS_PRICE), 0);
    assert_eq!(
        params.get_estimated_execution_duration(&zero_cost_attested_tx),
        0,
    );
}

/// Attaches a validator attestation with the given `computation_units`
/// to a transaction produced by `build_transaction`.
fn attest(
    tx: VerifiedExecutableAttestedTransaction,
    computation_units: u64,
) -> VerifiedExecutableAttestedTransaction {
    let (inner, _) = tx.into_parts();
    VerifiedExecutableAttestedTransaction::new(
        inner,
        Some(Attestation::Validator {
            payload: AttestationData::V1 {
                computation_units,
                object_versions: vec![],
            },
            attestor_index: 0,
        }),
    )
}

/// Within `TotalComputationUnits` mode, a commit of attested transactions is
/// scheduled differently from a commit of the same transactions without an
/// attestation: attested txs use the (cheap) attested computation units, while
/// unattested txs fall back to the (much larger) gas budget per the documented
/// fallback in `get_estimated_execution_duration`.
///
/// Note this is an in-mode contrast, not a production comparison: under
/// `TotalTxCount` (the production default for chains without validator
/// attestation), unattested txs are billed at one unit each.
#[test]
fn test_total_computation_units_attested_vs_unattested_commit_scheduling() {
    // Per-commit limit is large enough to schedule three attested transactions
    // (30 units each → cumulative 90) but too small to schedule even a single
    // unattested transaction whose gas budget converts to 200 gas units
    // (`gas_budget / gas_price`) > limit 100.
    const MAX_EXECUTION_DURATION_PER_COMMIT: u64 = 100;
    const TX_GAS_BUDGET: u64 = 200 * TEST_ONLY_GAS_PRICE;
    const TX_ATTESTED_UNITS: u64 = 30;

    let params = CongestionControlParameters::new_for_test(
        PerObjectCongestionControlMode::TotalComputationUnits,
        false,                                   // min_free_execution_slot
        Some(MAX_EXECUTION_DURATION_PER_COMMIT), // max_execution_duration_per_commit
        Some(0),                                 // overshoot
        0,                                       // max_gas_price (irrelevant)
        false,
        true,
    );

    let shared_obj = ObjectId::random();

    // --- Attested commit: three transactions all schedule, end-to-end. ---
    let mut tracker =
        SharedObjectCongestionTracker::new(std::iter::empty(), Vec::new(), params.clone());
    for i in 0..3 {
        let tx = attest(
            build_transaction(&[(shared_obj, true)], TX_GAS_BUDGET, TEST_ONLY_GAS_PRICE),
            TX_ATTESTED_UNITS,
        );
        let shared_input_objects = tx.shared_input_objects();
        tracker.initialize_object_execution_slots(&shared_input_objects);
        match tracker.try_schedule(&tx, &HashMap::new(), 0) {
            SequencingResult::Schedule(start_time) => {
                assert_eq!(
                    start_time,
                    i * TX_ATTESTED_UNITS,
                    "attested tx #{i} should be scheduled back-to-back",
                );
                tracker.bump_object_execution_slots(&tx, start_time);
            }
            SequencingResult::Defer(_, congested) => {
                panic!("attested tx #{i} should schedule, got defer on {congested:?}")
            }
        }
    }

    // --- Unattested commit: the very first transaction defers. ---
    let mut tracker = SharedObjectCongestionTracker::new(std::iter::empty(), Vec::new(), params);
    let tx = build_transaction(&[(shared_obj, true)], TX_GAS_BUDGET, TEST_ONLY_GAS_PRICE);
    tracker.initialize_object_execution_slots(&tx.shared_input_objects());
    match tracker.try_schedule(&tx, &HashMap::new(), 0) {
        SequencingResult::Defer(_, congested) => {
            assert_eq!(congested, vec![shared_obj]);
        }
        SequencingResult::Schedule(start_time) => {
            panic!("unattested tx should defer, got schedule at {start_time}");
        }
    }
}
/// `compute_deny_rule_update_chunks` decision table: no diff yields no
/// chunks; additions and switch activations apply while removals are still
/// locked; entry removals and switch deactivations wait for the unlock.
#[test]
fn deny_rule_update_chunks_respect_removal_grace() {
    let staying = Address::new([1u8; 32]);
    let leaving = Address::new([2u8; 32]);
    let arriving = Address::new([3u8; 32]);
    let mirror = DenyRuleSet {
        denied_addresses: [staying, leaving].into(),
        user_transaction_disabled: true,
        ..Default::default()
    };
    let aggregate = DenyRuleSet {
        denied_addresses: [staying, arriving].into(),
        shared_object_disabled: true,
        ..Default::default()
    };
    let version = Version::from_u64(5);

    // Object already up to date: nothing to inject, in either grace state.
    for unlocked in [false, true] {
        let (chunks, target) =
            compute_deny_rule_update_chunks(&mirror, &mirror, unlocked, 1000, 7, 42, version);
        assert!(chunks.is_empty());
        assert_eq!(target, mirror);
    }

    // Removals locked: the arriving address and the newly active switch go
    // out, but the leaving address stays and the mirrored switch stays on.
    let (chunks, target) =
        compute_deny_rule_update_chunks(&aggregate, &mirror, false, 1000, 7, 42, version);
    assert_eq!(chunks.len(), 1);
    let chunk = &chunks[0];
    assert_eq!(chunk.epoch, 7);
    assert_eq!(chunk.round, 42);
    assert_eq!(chunk.deny_rules_obj_initial_shared_version, version);
    assert_eq!(chunk.added_addresses, [arriving].into());
    assert!(chunk.removed_addresses.is_empty());
    assert!(chunk.shared_object_disabled);
    assert!(chunk.user_transaction_disabled);
    assert_eq!(target.denied_addresses, [staying, leaving, arriving].into());
    assert!(target.shared_object_disabled);
    assert!(target.user_transaction_disabled);

    // Removals unlocked: the full diff applies and the object reaches the
    // aggregate exactly.
    let (chunks, target) =
        compute_deny_rule_update_chunks(&aggregate, &mirror, true, 1000, 7, 42, version);
    assert_eq!(chunks.len(), 1);
    let chunk = &chunks[0];
    assert_eq!(chunk.added_addresses, [arriving].into());
    assert_eq!(chunk.removed_addresses, [leaving].into());
    assert!(chunk.shared_object_disabled);
    assert!(!chunk.user_transaction_disabled);
    assert_eq!(target, aggregate);

    // A switch deactivation alone is also held back until the unlock.
    let switch_off = DenyRuleSet {
        denied_addresses: mirror.denied_addresses.clone(),
        ..Default::default()
    };
    let (chunks, target) =
        compute_deny_rule_update_chunks(&switch_off, &mirror, false, 1000, 7, 42, version);
    assert!(chunks.is_empty());
    assert_eq!(target, mirror);
    let (chunks, _) =
        compute_deny_rule_update_chunks(&switch_off, &mirror, true, 1000, 7, 42, version);
    assert_eq!(chunks.len(), 1);
    assert!(!chunks[0].user_transaction_disabled);
}

/// A delta larger than the per-transaction limit splits into disjoint chunks
/// that reassemble to the full diff, each within the limit and carrying the
/// same switch states — and the split is deterministic.
#[test]
fn deny_rule_update_chunks_split_deterministically() {
    let mirror = DenyRuleSet {
        denied_packages: (0..2u8).map(|i| ObjectId::new([100 + i; 32])).collect(),
        ..Default::default()
    };
    let aggregate = DenyRuleSet {
        denied_addresses: (0..5u8).map(|i| Address::new([i; 32])).collect(),
        denied_objects: (0..3u8).map(|i| ObjectId::new([50 + i; 32])).collect(),
        user_transaction_disabled: true,
        ..Default::default()
    };
    let version = Version::from_u64(5);

    // 5 added addresses + 3 added objects + 2 removed packages = 10 entries.
    let (chunks, target) =
        compute_deny_rule_update_chunks(&aggregate, &mirror, true, 3, 7, 42, version);
    assert_eq!(chunks.len(), 4);
    assert_eq!(target, aggregate);

    let mut added_addresses = std::collections::BTreeSet::new();
    let mut added_objects = std::collections::BTreeSet::new();
    let mut removed_packages = std::collections::BTreeSet::new();
    for chunk in &chunks {
        let entries = chunk.added_addresses.len()
            + chunk.removed_addresses.len()
            + chunk.added_objects.len()
            + chunk.removed_objects.len()
            + chunk.added_packages.len()
            + chunk.removed_packages.len();
        assert!(entries <= 3);
        // Disjointness: no entry may appear in two chunks.
        for key in &chunk.added_addresses {
            assert!(added_addresses.insert(*key));
        }
        for key in &chunk.added_objects {
            assert!(added_objects.insert(*key));
        }
        for key in &chunk.removed_packages {
            assert!(removed_packages.insert(*key));
        }
        assert!(chunk.removed_addresses.is_empty());
        assert!(chunk.removed_objects.is_empty());
        assert!(chunk.added_packages.is_empty());
        // Every chunk carries the absolute switch states and target object.
        assert!(chunk.user_transaction_disabled);
        assert_eq!(chunk.epoch, 7);
        assert_eq!(chunk.round, 42);
        assert_eq!(chunk.deny_rules_obj_initial_shared_version, version);
    }
    assert_eq!(added_addresses, aggregate.denied_addresses);
    assert_eq!(added_objects, aggregate.denied_objects);
    assert_eq!(removed_packages, mirror.denied_packages);

    // Same inputs, same chunks.
    let (again, _) = compute_deny_rule_update_chunks(&aggregate, &mirror, true, 3, 7, 42, version);
    assert_eq!(chunks, again);

    // A zero limit is clamped to one entry per transaction.
    let (chunks, _) = compute_deny_rule_update_chunks(&aggregate, &mirror, true, 0, 7, 42, version);
    assert_eq!(chunks.len(), 10);
}

/// A mid-epoch restart resumes from the persisted mirror row — written
/// atomically with each injecting commit — not from the epoch-start seed,
/// which is stale once injections have advanced the object.
#[tokio::test]
async fn restart_recovers_the_deny_rule_mirror() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let address = Address::new([7u8; 32]);

    // Fresh epoch: no persisted row, the mirror starts from the seed.
    assert_eq!(
        *store.get_mirrored_transaction_deny_rules(),
        DenyRuleSet::default()
    );

    // An injecting commit persists the advanced mirror with its results.
    let mirror = rules_denying_address(address);
    let mut output = ConsensusCommitOutput::default();
    output.record_deny_rule_mirror(mirror.clone());
    output.set_default_commit_stats_for_testing();
    let mut batch = store.db_batch_for_test();
    output.write_to_batch(&store, &mut batch).unwrap();
    batch.write().unwrap();

    // Reopen over the same epoch DB, as a restart does.
    store.release_db_handles();
    let reopened = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        (*store.epoch_start_configuration).clone(),
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();

    assert_eq!(*reopened.get_mirrored_transaction_deny_rules(), mirror);
    // The recovered mirror flows back into enforcement.
    assert!(
        reopened
            .get_active_transaction_deny_rules()
            .denied_addresses
            .contains(&address)
    );
}

/// Injection at the commit boundary: with the flag on and the object present,
/// a recorded proposal makes the next commit inject a
/// `TransactionDenyRulesUpdate`, advance the mirror in lockstep, and go quiet
/// once the object is up to date. Without the object nothing is injected.
#[tokio::test]
async fn commit_injects_deny_rule_updates_and_advances_mirror() {
    use std::collections::VecDeque;

    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
    use iota_sdk_types::TransactionKind;
    use iota_types::transaction::TransactionAPI;

    use crate::consensus_handler::ConsensusCommitInfo;

    let mut protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
    protocol_config.set_deny_rule_governance_for_testing(true);
    protocol_config.set_deny_rule_governance_on_chain_for_testing(true);
    protocol_config.set_deny_rule_update_max_entries_per_tx_for_testing(1000);
    protocol_config.set_deny_rule_removal_grace_round_floor_for_testing(0);
    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config.clone())
        .build()
        .await;
    // The builder's config override drops when `build` returns; keep one
    // alive for the reopened epoch store below.
    let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, _| protocol_config.clone());
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;
    let address = Address::new([1u8; 32]);

    // The object does not exist this epoch: nothing to update.
    let commit_info = ConsensusCommitInfo::new_for_test(1, 0, false);
    let mut output = ConsensusCommitOutput::default();
    let mut transactions = VecDeque::new();
    let mut roots = std::collections::BTreeSet::new();
    store
        .add_deny_rule_update_transactions(&mut output, &mut transactions, &mut roots, &commit_info)
        .unwrap();
    assert!(transactions.is_empty());
    assert!(roots.is_empty());

    // Reopen with the object present at the epoch boundary, as the epoch
    // after its creation starts.
    let initial_shared_version = Version::from_u64(3);
    let mut epoch_start_configuration = (*store.epoch_start_configuration).clone();
    epoch_start_configuration
        .set_transaction_deny_rules_for_testing(initial_shared_version, DenyRuleSet::default());
    store.release_db_handles();
    let store = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        epoch_start_configuration,
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();
    flush_deny_rule_proposal(&store, deny_proposal(me, 1, rules_denying_address(address)));

    // The single validator holds all stake and the grace floor is 0, so the
    // first commit injects the full aggregate.
    let mut output = ConsensusCommitOutput::default();
    let mut transactions = VecDeque::new();
    let mut roots = std::collections::BTreeSet::new();
    store
        .add_deny_rule_update_transactions(&mut output, &mut transactions, &mut roots, &commit_info)
        .unwrap();
    assert_eq!(transactions.len(), 1);
    let TransactionKind::TransactionDenyRulesUpdate(update) =
        transactions[0].data().transaction().kind()
    else {
        panic!("expected a deny-rules update transaction");
    };
    assert_eq!(update.added_addresses, [address].into());
    assert_eq!(
        update.deny_rules_obj_initial_shared_version,
        initial_shared_version
    );
    assert_eq!(
        *store.get_mirrored_transaction_deny_rules(),
        rules_denying_address(address)
    );
    // Nothing else in a commit can depend on the deny-rules object, so the
    // update only reaches a checkpoint if it is a root of its own.
    assert_eq!(
        roots,
        [transactions[0].key()].into_iter().collect(),
        "the injected update must be registered as a checkpoint root"
    );

    // The advanced mirror is persisted with the same commit output.
    output.set_default_commit_stats_for_testing();
    let mut batch = store.db_batch_for_test();
    output.write_to_batch(&store, &mut batch).unwrap();
    batch.write().unwrap();

    // The next commit finds the object up to date and injects nothing.
    let mut output = ConsensusCommitOutput::default();
    let mut transactions = VecDeque::new();
    let mut roots = std::collections::BTreeSet::new();
    store
        .add_deny_rule_update_transactions(&mut output, &mut transactions, &mut roots, &commit_info)
        .unwrap();
    assert!(transactions.is_empty());
    assert!(roots.is_empty());

    // Withdrawing the proposal makes a later, proposal-free commit inject
    // the removal; the mirror and the enforced active set must drop the
    // entry in that same commit, not at the next proposal-driven recompute.
    flush_deny_rule_proposal(&store, deny_proposal(me, 2, DenyRuleSet::default()));
    assert!(
        store
            .get_active_transaction_deny_rules()
            .denied_addresses
            .contains(&address)
    );
    let mut output = ConsensusCommitOutput::default();
    let mut transactions = VecDeque::new();
    let mut roots = std::collections::BTreeSet::new();
    store
        .add_deny_rule_update_transactions(&mut output, &mut transactions, &mut roots, &commit_info)
        .unwrap();
    assert_eq!(transactions.len(), 1);
    let TransactionKind::TransactionDenyRulesUpdate(update) =
        transactions[0].data().transaction().kind()
    else {
        panic!("expected a deny-rules update transaction");
    };
    assert_eq!(update.removed_addresses, [address].into());
    assert_eq!(
        *store.get_mirrored_transaction_deny_rules(),
        DenyRuleSet::default()
    );
    assert_eq!(
        *store.get_active_transaction_deny_rules(),
        DenyRuleSet::default()
    );
    assert_eq!(roots, [transactions[0].key()].into_iter().collect());
}

/// A commit processed after `close_all_tx` ignores every chunk: nothing is
/// scheduled or rooted, the mirror is held back, and the unchanged mirror
/// re-derives the identical delta for the next derivation.
#[tokio::test]
async fn skipped_deny_rule_chunks_hold_the_mirror_back() {
    use std::collections::VecDeque;

    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};

    use crate::consensus_handler::ConsensusCommitInfo;

    let mut protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
    protocol_config.set_deny_rule_governance_for_testing(true);
    protocol_config.set_deny_rule_governance_on_chain_for_testing(true);
    protocol_config.set_deny_rule_update_max_entries_per_tx_for_testing(1000);
    protocol_config.set_deny_rule_removal_grace_round_floor_for_testing(0);
    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config.clone())
        .build()
        .await;
    // The builder's config override drops when `build` returns; keep one
    // alive for the reopened epoch store below.
    let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, _| protocol_config.clone());
    let store = authority_state.epoch_store_for_testing();
    let me = store.name;
    let address = Address::new([1u8; 32]);

    let initial_shared_version = Version::from_u64(3);
    let mut epoch_start_configuration = (*store.epoch_start_configuration).clone();
    epoch_start_configuration
        .set_transaction_deny_rules_for_testing(initial_shared_version, DenyRuleSet::default());
    store.release_db_handles();
    let store = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        epoch_start_configuration,
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();
    flush_deny_rule_proposal(&store, deny_proposal(me, 1, rules_denying_address(address)));

    // The epoch is closing: no transaction is accepted any more.
    store.get_reconfig_state_write_lock_guard().close_all_tx();

    let commit_info = ConsensusCommitInfo::new_for_test(1, 0, false);
    let mut output = ConsensusCommitOutput::default();
    let mut transactions = VecDeque::new();
    let mut roots = std::collections::BTreeSet::new();
    store
        .add_deny_rule_update_transactions(&mut output, &mut transactions, &mut roots, &commit_info)
        .unwrap();
    assert!(transactions.is_empty());
    assert!(roots.is_empty());
    assert_eq!(
        *store.get_mirrored_transaction_deny_rules(),
        DenyRuleSet::default(),
        "an ignored chunk must hold the mirror back"
    );

    // The held-back mirror re-derives the identical delta.
    let mirror = store.get_mirrored_transaction_deny_rules();
    let (chunks, target) = compute_deny_rule_update_chunks(
        &rules_denying_address(address),
        &mirror,
        true,
        1000,
        store.epoch(),
        1,
        initial_shared_version,
    );
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].added_addresses, [address].into());
    assert_eq!(target, rules_denying_address(address));
}

/// Every chunk of a split delta becomes its own checkpoint root: a chunk that
/// executes but reaches no checkpoint would leave the object behind on every
/// node that follows checkpoints rather than consensus.
#[tokio::test]
async fn every_injected_deny_rule_chunk_becomes_a_checkpoint_root() {
    use std::collections::{BTreeSet, VecDeque};

    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};

    use crate::consensus_handler::ConsensusCommitInfo;

    let mut protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
    protocol_config.set_deny_rule_governance_for_testing(true);
    protocol_config.set_deny_rule_governance_on_chain_for_testing(true);
    // One entry per transaction, so the four denied addresses below split
    // across four chunks.
    protocol_config.set_deny_rule_update_max_entries_per_tx_for_testing(1);
    protocol_config.set_deny_rule_removal_grace_round_floor_for_testing(0);
    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config.clone())
        .build()
        .await;
    let store = authority_state.epoch_store_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, _| protocol_config.clone());
    let me = store.name;
    let rules = DenyRuleSet {
        denied_addresses: (0..4u8).map(|i| Address::new([i; 32])).collect(),
        ..Default::default()
    };

    let mut epoch_start_configuration = (*store.epoch_start_configuration).clone();
    epoch_start_configuration
        .set_transaction_deny_rules_for_testing(Version::from_u64(3), DenyRuleSet::default());
    store.release_db_handles();
    let store = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        epoch_start_configuration,
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();
    flush_deny_rule_proposal(&store, deny_proposal(me, 1, rules));

    let mut output = ConsensusCommitOutput::default();
    let mut transactions = VecDeque::new();
    let mut roots = BTreeSet::new();
    store
        .add_deny_rule_update_transactions(
            &mut output,
            &mut transactions,
            &mut roots,
            &ConsensusCommitInfo::new_for_test(1, 0, false),
        )
        .unwrap();

    assert_eq!(transactions.len(), 4);
    let injected: BTreeSet<_> = transactions.iter().map(|tx| tx.key()).collect();
    assert_eq!(roots, injected);
}

/// The guard only compares on a node that kept a mirror through the closing
/// epoch: matching states pass, and a node outside the closing committee is
/// exempt even when its seed lags the object.
#[tokio::test]
async fn deny_rule_mirror_guard_exempts_nodes_outside_the_committee() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    // Close the epoch so the comparisons below run rather than being exempt.
    store.get_reconfig_state_write_lock_guard().close_all_tx();
    let walked = rules_denying_address(Address::new([9u8; 32]));

    // Matching states (both default) pass for a committee member, as does a
    // configuration without the object.
    let mut matching = (*store.epoch_start_configuration).clone();
    matching.set_transaction_deny_rules_for_testing(Version::from_u64(3), DenyRuleSet::default());
    // `debug_fatal!` aborts test configurations, so returning is the assertion.
    authority_state.check_transaction_deny_rules_consistency(&store, &matching);
    authority_state
        .check_transaction_deny_rules_consistency(&store, &store.epoch_start_configuration);

    // A node that was not in the closing epoch's committee has no mirror to
    // compare: reopen the store under a foreign committee and diverge the
    // walked state — the guard must not fire.
    let mut diverged = (*store.epoch_start_configuration).clone();
    diverged.set_transaction_deny_rules_for_testing(Version::from_u64(3), walked);
    // The simple test committee is seeded like the test authority's own keys,
    // so drop this node's name explicitly to get a committee it is not in.
    let (simple_committee, _keys) = Committee::new_simple_test_committee();
    let foreign_committee = Committee::new_for_testing_with_normalized_voting_power(
        0,
        simple_committee
            .members()
            .filter(|(name, _)| *name != store.name)
            .map(|(name, stake)| (*name, *stake))
            .collect(),
    );
    store.release_db_handles();
    let fullnode_store = AuthorityPerEpochStore::new(
        store.name,
        std::sync::Arc::new(foreign_committee),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        (*store.epoch_start_configuration).clone(),
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();
    assert!(authority_state.is_fullnode(&fullnode_store));
    authority_state.check_transaction_deny_rules_consistency(&fullnode_store, &diverged);
}

/// With the object present, a fresh epoch persists the mirror seed row
/// immediately, so a row absent although commits have flushed is a lost
/// row: the store refuses to open rather than start from the stale seed.
#[tokio::test]
#[should_panic(expected = "deny_rule_mirror row is missing")]
async fn lost_mirror_row_after_flushed_commits_fails_the_reopen() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let mut with_object = (*store.epoch_start_configuration).clone();
    with_object
        .set_transaction_deny_rules_for_testing(Version::from_u64(3), DenyRuleSet::default());

    // Fresh epoch with the object: the constructor writes the seed row.
    store.release_db_handles();
    let store = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        with_object.clone(),
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();
    let tables = store.tables().unwrap();
    assert!(tables.deny_rule_mirror.get(&()).unwrap().is_some());

    // Flush a commit, then lose the row.
    flush_deny_rule_proposal(&store, deny_proposal(store.name, 1, DenyRuleSet::default()));
    tables.deny_rule_mirror.remove(&()).unwrap();
    drop(tables);
    store.release_db_handles();
    let _ = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        with_object,
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    );
}

/// A validator that crosses the boundary by executing synced checkpoints
/// never closes the epoch in its own consensus. Its mirror legitimately
/// lags the object and the re-seed repairs it. The guard must not report
/// that lag as a divergence.
#[tokio::test]
async fn deny_rule_mirror_guard_exempts_an_epoch_consensus_did_not_close() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    // The reconfig state stays at its default: the epoch was never closed.
    let mut diverged = (*store.epoch_start_configuration).clone();
    diverged.set_transaction_deny_rules_for_testing(
        Version::from_u64(3),
        rules_denying_address(Address::new([9u8; 32])),
    );
    // `debug_fatal!` aborts test configurations, so returning is the assertion.
    authority_state.check_transaction_deny_rules_consistency(&store, &diverged);
    assert_eq!(store.metrics.deny_rule_mirror_divergence.get(), 0);
}

/// A diverged mirror is reported: `debug_fatal!` aborts test configurations,
/// so a divergence introduced by a bug fails the suite. The epoch is closed
/// first so the comparison runs.
#[tokio::test]
#[should_panic(expected = "diverged from the mirrored state")]
async fn deny_rule_mirror_guard_reports_divergence() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    store.get_reconfig_state_write_lock_guard().close_all_tx();
    let mut diverged = (*store.epoch_start_configuration).clone();
    diverged.set_transaction_deny_rules_for_testing(
        Version::from_u64(3),
        rules_denying_address(Address::new([9u8; 32])),
    );
    authority_state.check_transaction_deny_rules_consistency(&store, &diverged);
}

/// Objects cannot be deleted, so a walk finding no object after the closing
/// epoch had one means the local store lost it. Unlike a readable divergence
/// there is nothing to re-seed from, so this fail-stops in every build.
#[tokio::test]
#[should_panic(expected = "missing from the state walked")]
async fn deny_rule_mirror_guard_fails_on_a_missing_object() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let next_without_object = (*store.epoch_start_configuration).clone();
    let mut with_object = (*store.epoch_start_configuration).clone();
    with_object
        .set_transaction_deny_rules_for_testing(Version::from_u64(3), DenyRuleSet::default());
    store.release_db_handles();
    let closing_store = AuthorityPerEpochStore::new(
        store.name,
        store.committee().clone(),
        &store.parent_path,
        store.db_options.clone(),
        store.metrics.clone(),
        with_object,
        authority_state.get_backing_package_store().clone(),
        store.execution_component.metrics(),
        store.signature_verifier.metrics.clone(),
        &ExpensiveSafetyCheckConfig::default(),
        store.chain,
        0,
    )
    .unwrap();
    authority_state.check_transaction_deny_rules_consistency(&closing_store, &next_without_object);
}

/// Failure effects for a `TransactionDenyRulesUpdate` fire the report;
/// success effects and other transaction kinds do not. Identification is by
/// kind, so a fresh authority — a restart — reports a failure it never
/// scheduled; a tracking-set refactor would break this test. The effects
/// arrive with an expected digest here, as they do while executing a certified
/// checkpoint, so the report counts and logs without asserting.
#[tokio::test]
async fn failed_deny_rule_update_execution_is_reported() {
    use std::collections::BTreeSet;

    use iota_sdk_types::{ExecutionError, ExecutionStatus, TransactionDenyRulesUpdate};
    use iota_types::{
        effects::TestEffectsBuilder, executable_transaction::VerifiedExecutableTransaction,
        transaction::VerifiedTransaction,
    };

    let update_transaction = || {
        VerifiedExecutableTransaction::new_system(
            VerifiedTransaction::new_transaction_deny_rules_update(TransactionDenyRulesUpdate {
                epoch: 0,
                round: 1,
                added_addresses: [Address::new([7u8; 32])].into(),
                removed_addresses: BTreeSet::new(),
                added_objects: BTreeSet::new(),
                removed_objects: BTreeSet::new(),
                added_packages: BTreeSet::new(),
                removed_packages: BTreeSet::new(),
                package_publish_disabled: false,
                package_upgrade_disabled: false,
                shared_object_disabled: false,
                user_transaction_disabled: false,
                receiving_objects_disabled: false,
                move_authenticator_disabled: false,
                deny_rules_obj_initial_shared_version: Version::from_u64(1),
            }),
            0,
        )
    };
    let failure = || ExecutionStatus::Failure {
        error: ExecutionError::InsufficientGas,
        command: None,
    };
    let update_effects = |transaction: &VerifiedExecutableTransaction, status: ExecutionStatus| {
        TestEffectsBuilder::new(transaction.data())
            .with_shared_input_versions(
                [(
                    iota_types::IOTA_TRANSACTION_DENY_RULES_OBJECT_ID,
                    Version::from_u64(1),
                )]
                .into(),
            )
            .with_status(status)
            .build()
    };

    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let failures = || store.metrics.deny_rule_update_execution_failures.get();

    let update = update_transaction();
    let effects = update_effects(&update, ExecutionStatus::Success);
    authority_state.report_failed_deny_rule_update_execution(
        &update,
        &effects,
        Some(effects.digest()),
        &store,
    );
    assert_eq!(failures(), 0);

    let effects = update_effects(&update, failure());
    authority_state.report_failed_deny_rule_update_execution(
        &update,
        &effects,
        Some(effects.digest()),
        &store,
    );
    assert_eq!(failures(), 1);

    let genesis = VerifiedExecutableTransaction::new_system(
        VerifiedTransaction::new_genesis_transaction(Vec::new(), Vec::new()),
        0,
    );
    let genesis_effects = TestEffectsBuilder::new(genesis.data())
        .with_status(failure())
        .build();
    authority_state.report_failed_deny_rule_update_execution(
        &genesis,
        &genesis_effects,
        Some(genesis_effects.digest()),
        &store,
    );
    assert_eq!(failures(), 1);

    let restarted_state = TestAuthorityBuilder::new().build().await;
    let restarted_store = restarted_state.epoch_store_for_testing();
    let update = update_transaction();
    let effects = update_effects(&update, failure());
    restarted_state.report_failed_deny_rule_update_execution(
        &update,
        &effects,
        Some(effects.digest()),
        &restarted_store,
    );
    assert_eq!(
        restarted_store
            .metrics
            .deny_rule_update_execution_failures
            .get(),
        1
    );
}

/// Effects the node derived itself carry no expected digest, and a failure in
/// them is this node's own broken invariant: the report asserts. Only the
/// simulator can observe a `debug_fatal!` instead of aborting on it, so the
/// live path is covered here rather than in the plain unit test above.
#[cfg(msim)]
#[iota_macros::sim_test]
async fn failed_deny_rule_update_execution_asserts_on_derived_effects() {
    use std::{
        collections::BTreeSet,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use iota_common::register_debug_fatal_handler;
    use iota_sdk_types::{ExecutionError, ExecutionStatus, TransactionDenyRulesUpdate};
    use iota_types::{
        effects::TestEffectsBuilder, executable_transaction::VerifiedExecutableTransaction,
        transaction::VerifiedTransaction,
    };

    let asserted = Arc::new(AtomicUsize::new(0));
    register_debug_fatal_handler!("TransactionDenyRulesUpdate failed execution", {
        let asserted = asserted.clone();
        move || {
            asserted.fetch_add(1, Ordering::SeqCst);
        }
    });

    let update = VerifiedExecutableTransaction::new_system(
        VerifiedTransaction::new_transaction_deny_rules_update(TransactionDenyRulesUpdate {
            epoch: 0,
            round: 1,
            added_addresses: [Address::new([7u8; 32])].into(),
            removed_addresses: BTreeSet::new(),
            added_objects: BTreeSet::new(),
            removed_objects: BTreeSet::new(),
            added_packages: BTreeSet::new(),
            removed_packages: BTreeSet::new(),
            package_publish_disabled: false,
            package_upgrade_disabled: false,
            shared_object_disabled: false,
            user_transaction_disabled: false,
            receiving_objects_disabled: false,
            move_authenticator_disabled: false,
            deny_rules_obj_initial_shared_version: Version::from_u64(1),
        }),
        0,
    );
    let effects = |status| {
        TestEffectsBuilder::new(update.data())
            .with_shared_input_versions(
                [(
                    iota_types::IOTA_TRANSACTION_DENY_RULES_OBJECT_ID,
                    Version::from_u64(1),
                )]
                .into(),
            )
            .with_status(status)
            .build()
    };

    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();

    authority_state.report_failed_deny_rule_update_execution(
        &update,
        &effects(ExecutionStatus::Success),
        None,
        &store,
    );
    assert_eq!(asserted.load(Ordering::SeqCst), 0);
    assert_eq!(store.metrics.deny_rule_update_execution_failures.get(), 0);

    authority_state.report_failed_deny_rule_update_execution(
        &update,
        &effects(ExecutionStatus::Failure {
            error: ExecutionError::InsufficientGas,
            command: None,
        }),
        None,
        &store,
    );
    assert_eq!(asserted.load(Ordering::SeqCst), 1);
    assert_eq!(store.metrics.deny_rule_update_execution_failures.get(), 1);
}
