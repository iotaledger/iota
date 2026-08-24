// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_sdk_types::{Address, DenyRuleSet, ObjectId, TransactionDigest};
use iota_types::{
    base_types::AuthorityName, committee::Committee, crypto::KeypairTraits,
    messages_consensus::TransactionDenyRuleProposal,
};
use tokio::time::timeout;
use typed_store::rocks::DBBatch;

use crate::authority::{
    authority_per_epoch_store::{
        AuthorityPerEpochStore, consensus_quarantine::ConsensusCommitOutput,
    },
    test_authority_builder::TestAuthorityBuilder,
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
