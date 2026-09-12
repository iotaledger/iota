// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::{
    Address, CheckpointContents, CheckpointSummary, GasCostSummary, ObjectId, Owner,
    TransactionEffects, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{ExecutionDigests, random_object_ref},
    committee::EpochId,
    crypto::{
        AccountPrivateKey, AuthorityStrongQuorumSignInfo, deterministic_random_account_private_key,
    },
    effects::{TestEffectsBuilder, TransactionEffectsExt},
    message_envelope::Envelope,
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber, VerifiedCheckpoint},
    object::Object,
    storage::ObjectKey,
    transaction::VerifiedTransaction,
};
use prometheus_filtered::Registry;
use tempfile::TempDir;
use typed_store::{database::wait_for_database_close, traits::Map};

use super::{KEYS_PER_SLICE, ObjectBacklogSweep, ObjectBacklogSweepProgress, sweep};
use crate::{
    authority::{
        AuthorityStore,
        authority_store_tables::AuthorityPerpetualTables,
        authority_store_types::{StoreObject, StoreObjectWrapper, get_store_object},
    },
    checkpoints::CheckpointStore,
    test_utils::executed_checkpoint,
};

/// The epoch that is current while the sweep runs, and whose bucket it
/// records the tombstones it finds in.
const SWEEP_EPOCH: EpochId = 7;

/// An object with three live versions, walked first.
fn live_id() -> ObjectId {
    ObjectId::new([1; 32])
}

/// An object deleted at its third version, walked second.
fn deleted_id() -> ObjectId {
    ObjectId::new([2; 32])
}

/// An object wrapped at its second version and unwrapped at its third,
/// walked last.
fn wrapped_id() -> ObjectId {
    ObjectId::new([3; 32])
}

fn open_store(dir: &TempDir) -> Arc<AuthorityStore> {
    let (perpetual, historic) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    AuthorityStore::open_no_genesis(
        Arc::new(perpetual),
        Arc::new(historic),
        false,
        &Registry::new(),
    )
    .unwrap()
}

/// A checkpoint store with nothing in it, for the walks that never read one.
fn empty_checkpoint_store(dir: &TempDir) -> Arc<CheckpointStore> {
    CheckpointStore::new(&dir.path().join("checkpoints"))
}

fn value(id: ObjectId, version: u64) -> (ObjectKey, StoreObjectWrapper) {
    (
        ObjectKey(id, version.into()),
        get_store_object(
            Object::with_id_owner_version_for_testing(id, version.into(), Owner::Immutable),
            None,
        ),
    )
}

fn tombstone(id: ObjectId, version: u64, row: StoreObject) -> (ObjectKey, StoreObjectWrapper) {
    (ObjectKey(id, version.into()), StoreObjectWrapper::from(row))
}

/// Writes the live table an earlier build would have left behind: every
/// object still carries the versions superseded before the upgrade next to
/// its newest row.
fn seed(store: &AuthorityStore) {
    store
        .perpetual_tables
        .objects
        .multi_insert([
            value(live_id(), 1),
            value(live_id(), 2),
            value(live_id(), 3),
            value(deleted_id(), 1),
            value(deleted_id(), 2),
            tombstone(deleted_id(), 3, StoreObject::Deleted),
            value(wrapped_id(), 1),
            tombstone(wrapped_id(), 2, StoreObject::Wrapped),
            value(wrapped_id(), 3),
        ])
        .unwrap();
}

fn sweeper(store: &AuthorityStore, keys_per_slice: usize) -> ObjectBacklogSweep {
    ObjectBacklogSweep {
        perpetual_tables: store.perpetual_tables.clone(),
        historic_objects: store.get_historic_objects().clone(),
        keys_per_slice,
    }
}

/// Runs the whole walk, in slices of `keys_per_slice`.
fn sweep_all(store: &AuthorityStore, keys_per_slice: usize) {
    let sweep = sweeper(store, keys_per_slice);
    while sweep.sweep_slice(SWEEP_EPOCH).unwrap() {}
}

fn live_keys(store: &AuthorityStore) -> Vec<ObjectKey> {
    store
        .perpetual_tables
        .objects
        .safe_iter()
        .map(|row| row.unwrap().0)
        .collect()
}

fn recorded_tombstones(store: &AuthorityStore, epoch: EpochId) -> Vec<ObjectKey> {
    store
        .get_historic_objects()
        .ensure(epoch)
        .unwrap()
        .tombstones
        .safe_iter()
        .map(|row| row.unwrap().0)
        .collect()
}

/// The keys of the versions relocated into `epoch`'s bucket.
fn relocated_keys(store: &AuthorityStore, epoch: EpochId) -> Vec<ObjectKey> {
    store
        .get_historic_objects()
        .ensure(epoch)
        .unwrap()
        .objects
        .safe_iter()
        .map(|row| row.unwrap().0)
        .collect()
}

fn progress(store: &AuthorityStore) -> Option<ObjectBacklogSweepProgress> {
    store
        .perpetual_tables
        .object_backlog_sweep_progress
        .get(&())
        .unwrap()
}

/// Writes a checkpoint whose single transaction superseded `mutated` and
/// wrote a tombstone over each of `deleted`, the way the build before the
/// buckets recorded it: the summary and its contents in the checkpoint
/// store, the effects in the flat perpetual table.
fn seed_checkpoint(
    store: &AuthorityStore,
    checkpoint_store: &CheckpointStore,
    sequence_number: CheckpointSequenceNumber,
    mutated: &[(ObjectId, u64)],
    deleted: &[(ObjectId, u64)],
) -> TransactionEffects {
    let (sender, keypair): (Address, AccountPrivateKey) =
        deterministic_random_account_private_key();
    let transaction = VerifiedTransaction::new_unchecked(
        TestTransactionBuilder::new(sender, random_object_ref(), 100)
            .transfer(random_object_ref(), sender)
            .build_and_sign(&keypair),
    );
    let effects = TestEffectsBuilder::new(transaction.inner())
        .with_mutated_objects(
            mutated
                .iter()
                .map(|(id, version)| (*id, (*version).into(), Owner::Address(sender))),
        )
        .with_deleted_objects(deleted.iter().map(|(id, version)| (*id, (*version).into())))
        .build();
    let effects_digest = effects.digest();
    store
        .perpetual_tables
        .effects
        .insert(&effects_digest, &effects)
        .unwrap();

    let contents = CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::new(
        *transaction.digest(),
        effects_digest,
    )]);
    // Like `test_utils::certified_summary`, but naming the contents above:
    // the sweep resolves them through the summary's digest.
    let summary = CheckpointSummary {
        epoch: 0,
        sequence_number,
        network_total_transactions: 0,
        contents_digest: contents.digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: GasCostSummary::default(),
        end_of_epoch_data: None,
        timestamp_ms: 0,
        version_specific_data: Vec::new(),
        checkpoint_commitments: Vec::new(),
    };
    let checkpoint = VerifiedCheckpoint::new_unchecked(Envelope::new_from_data_and_sig(
        summary,
        AuthorityStrongQuorumSignInfo {
            epoch: 0,
            signature: Default::default(),
            signers_map: Default::default(),
        },
    ));
    checkpoint_store
        .insert_verified_checkpoint(&checkpoint)
        .unwrap();
    checkpoint_store
        .insert_checkpoint_contents(contents)
        .unwrap();
    checkpoint_store
        .update_highest_executed_checkpoint(&checkpoint)
        .unwrap();
    effects
}

/// Records the watermark an earlier build's objects pruner would have left.
fn seed_pruner_watermark(store: &AuthorityStore, watermark: CheckpointSequenceNumber) {
    store
        .perpetual_tables
        .object_backlog_sweep_bound
        .insert(&(), &watermark)
        .unwrap();
}

/// The live table is left with the newest version of every object and with
/// every tombstone, including the one an unwrap left below a newer version.
/// Every superseded version is relocated into the current epoch's bucket, and
/// each tombstone is recorded there too, so that ordinary retention deletes
/// the two together later.
#[tokio::test]
async fn the_sweep_keeps_the_latest_version_and_the_tombstones() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    seed(&store);

    sweep_all(&store, 5_000);

    assert_eq!(
        live_keys(&store),
        vec![
            ObjectKey(live_id(), 3.into()),
            ObjectKey(deleted_id(), 3.into()),
            ObjectKey(wrapped_id(), 2.into()),
            ObjectKey(wrapped_id(), 3.into()),
        ]
    );
    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(live_id(), 1.into()),
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
            ObjectKey(wrapped_id(), 1.into()),
        ]
    );
    assert_eq!(
        recorded_tombstones(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(deleted_id(), 3.into()),
            ObjectKey(wrapped_id(), 2.into()),
        ]
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}

/// A relocated version is the object it was in the live table, and the
/// bounded read serves it from the bucket once it is no longer live. A
/// version relocated from under a tombstone is served below that tombstone
/// and never above it.
#[tokio::test]
async fn a_relocated_version_is_readable_from_the_current_epoch_bucket() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    seed(&store);

    sweep_all(&store, 5_000);

    let key = ObjectKey(live_id(), 2.into());
    let object = Object::with_id_owner_version_for_testing(live_id(), 2.into(), Owner::Immutable);
    assert_eq!(
        store
            .get_historic_objects()
            .ensure(SWEEP_EPOCH)
            .unwrap()
            .objects
            .get(&key)
            .unwrap(),
        Some(object.clone())
    );
    assert_eq!(
        store.get_historic_objects().get(&key).unwrap(),
        Some(object)
    );

    for (id, bound, expected) in [
        // Relocated out of the live table, and answered from the bucket.
        (live_id(), 1, Some(1)),
        (live_id(), 2, Some(2)),
        // Still the newest live version.
        (live_id(), 3, Some(3)),
        // Below the tombstone the deletion left in the live table.
        (deleted_id(), 2, Some(2)),
        // At and above it the object is gone, and the versions relocated
        // from beneath it are not served in its place.
        (deleted_id(), 3, None),
        (deleted_id(), 4, None),
    ] {
        assert_eq!(
            store
                .find_object_lt_or_eq_version_with_historic_fallback(id, bound.into())
                .unwrap()
                .map(|object| object.version()),
            expected.map(Version::from),
            "object {id} bounded at {bound}"
        );
    }
}

/// One call walks the whole table, however many slices that takes, so a
/// caller that awaits it is left with no backlog at all: the versions the
/// slices after the first one decide on are relocated too.
#[tokio::test]
async fn one_call_drives_the_walk_past_the_slice_boundary() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let last_version = KEYS_PER_SLICE as u64 + 10;
    store
        .perpetual_tables
        .objects
        .multi_insert((1..=last_version).map(|version| value(live_id(), version)))
        .unwrap();

    sweep(
        store.clone(),
        empty_checkpoint_store(&dir),
        SWEEP_EPOCH,
        false,
    )
    .await
    .unwrap();

    assert_eq!(
        live_keys(&store),
        vec![ObjectKey(live_id(), last_version.into())]
    );
    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        (1..last_version)
            .map(|version| ObjectKey(live_id(), version.into()))
            .collect::<Vec<_>>()
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}

/// A walk stopped part-way resumes from the key it recorded, across a
/// restart, and leaves the same table an uninterrupted walk does.
#[tokio::test]
async fn the_sweep_resumes_from_its_watermark() {
    let uninterrupted_dir = iota_common::tempdir();
    let uninterrupted = open_store(&uninterrupted_dir);
    seed(&uninterrupted);
    sweep_all(&uninterrupted, 5_000);

    let dir = iota_common::tempdir();
    let interrupted = open_store(&dir);
    seed(&interrupted);
    let sweep = sweeper(&interrupted, 1);
    assert!(sweep.sweep_slice(SWEEP_EPOCH).unwrap());
    // One row decided, the first version of the first object id, which the
    // second version supersedes.
    assert_eq!(
        progress(&interrupted),
        Some(ObjectBacklogSweepProgress::SweptThrough(ObjectKey(
            live_id(),
            1.into()
        )))
    );
    assert_eq!(live_keys(&interrupted).len(), 8);
    assert_eq!(
        relocated_keys(&interrupted, SWEEP_EPOCH),
        vec![ObjectKey(live_id(), 1.into())]
    );

    // Release every handle on the database before reopening the same path,
    // as a restart does.
    let weak_db = Arc::downgrade(&interrupted.perpetual_tables.objects.db);
    drop(sweep);
    drop(interrupted);
    assert!(wait_for_database_close(weak_db).await);

    let resumed = open_store(&dir);
    sweep_all(&resumed, 1);

    assert_eq!(live_keys(&resumed), live_keys(&uninterrupted));
    assert_eq!(
        relocated_keys(&resumed, SWEEP_EPOCH),
        relocated_keys(&uninterrupted, SWEEP_EPOCH)
    );
    assert_eq!(
        recorded_tombstones(&resumed, SWEEP_EPOCH),
        recorded_tombstones(&uninterrupted, SWEEP_EPOCH)
    );
    assert_eq!(progress(&resumed), Some(ObjectBacklogSweepProgress::Done));
}

/// Once the walk has reached the end of the table, a later start does
/// nothing: from then on a superseded version leaves the live table in the
/// batch that supersedes it, and there is no backlog left to drain.
#[tokio::test]
async fn a_finished_sweep_leaves_later_starts_nothing_to_do() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    seed(&store);
    sweep_all(&store, 5_000);

    let (key, row) = value(live_id(), 4);
    store.perpetual_tables.objects.insert(&key, &row).unwrap();

    sweep_all(&store, 5_000);

    let superseded = ObjectKey(live_id(), 3.into());
    assert!(
        store
            .perpetual_tables
            .objects
            .get(&superseded)
            .unwrap()
            .is_some()
    );
    assert!(
        !relocated_keys(&store, SWEEP_EPOCH).contains(&superseded),
        "a version superseded after the walk finished is the commit's to relocate"
    );
}

/// With the earlier build's watermark to hand, the walk reads the effects of
/// the checkpoints above it instead of the live table, and relocates exactly
/// the versions those checkpoints superseded. The versions below the
/// watermark are the pruner's business and are left alone — here, a
/// superseded version deliberately left in the table stays put, which is what
/// tells the bounded walk apart from the unbounded one.
#[tokio::test]
async fn the_bounded_walk_relocates_what_the_checkpoints_above_the_watermark_superseded() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    store
        .perpetual_tables
        .objects
        .multi_insert([
            // Superseded by checkpoint 8, above the watermark.
            value(live_id(), 1),
            value(live_id(), 2),
            // Superseded before the watermark and never deleted, standing in
            // for a row the unbounded walk would have moved.
            value(deleted_id(), 1),
            value(deleted_id(), 2),
        ])
        .unwrap();
    seed_pruner_watermark(&store, 7);
    seed_checkpoint(&store, &checkpoint_store, 8, &[(live_id(), 1)], &[]);

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH, false)
        .await
        .unwrap();

    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![ObjectKey(live_id(), 1.into())],
        "only the version checkpoint 8 superseded is relocated"
    );
    assert_eq!(
        live_keys(&store),
        vec![
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
        ],
        "the rows at or below the watermark are left where the pruner left them"
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}

/// A tombstone written above the watermark is recorded as a head in the
/// bucket and left in the live table, so that a bounded read still answers
/// "deleted" and retention can collect it later.
#[tokio::test]
async fn the_bounded_walk_records_the_tombstones_above_the_watermark() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed_pruner_watermark(&store, 3);
    let effects = seed_checkpoint(&store, &checkpoint_store, 4, &[], &[(deleted_id(), 2)]);
    // The tombstone sits at the transaction's lamport version, which the
    // effects decide, so the live table is seeded from them rather than from
    // a version guessed here.
    let heads: Vec<ObjectKey> = effects
        .all_tombstones()
        .into_iter()
        .map(|(id, version)| ObjectKey(id, version))
        .collect();
    store
        .perpetual_tables
        .objects
        .multi_insert(
            heads
                .iter()
                .map(|key| (*key, StoreObjectWrapper::from(StoreObject::Deleted))),
        )
        .unwrap();

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH, false)
        .await
        .unwrap();

    assert_eq!(recorded_tombstones(&store, SWEEP_EPOCH), heads);
    for key in &heads {
        assert!(
            live_keys(&store).contains(key),
            "the head stays in the live table until its bucket expires"
        );
    }
}

/// A database whose pruner ran with the compaction filter left rows beneath
/// its watermark, so the watermark must not be trusted and the whole table is
/// walked instead.
#[tokio::test]
async fn a_pruner_database_refuses_the_bounded_walk() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed(&store);
    seed_pruner_watermark(&store, u64::MAX);

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH, true)
        .await
        .unwrap();

    // The unbounded walk's outcome: every superseded version relocated,
    // which the bounded walk would not have done at this watermark.
    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(live_id(), 1.into()),
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
            ObjectKey(wrapped_id(), 1.into()),
        ]
    );
}

/// A watermark the checkpoint pruner has itself overtaken names checkpoints
/// the store no longer holds, so it cannot be used to find the backlog and
/// the whole table is walked instead. An earlier build could leave this by
/// holding fewer epochs of checkpoints than of object versions, or by having
/// object pruning turned off after it had once run.
#[tokio::test]
async fn a_watermark_below_the_retained_checkpoints_refuses_the_bounded_walk() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed(&store);
    // The objects pruner stopped at 5; the checkpoint pruner went on to 9, so
    // the summaries that would name the backlog are gone.
    seed_pruner_watermark(&store, 5);
    checkpoint_store
        .update_highest_pruned_checkpoint(&executed_checkpoint(0, 9))
        .unwrap();

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH, false)
        .await
        .unwrap();

    // The unbounded walk's outcome, which the bounded one could not have
    // reached from a watermark of 5.
    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(live_id(), 1.into()),
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
            ObjectKey(wrapped_id(), 1.into()),
        ]
    );
}

/// The bounded walk resumes at the checkpoint after the last slice it wrote,
/// so an interrupted run neither repeats a slice nor skips one.
#[tokio::test]
async fn the_bounded_walk_resumes_at_the_checkpoint_it_recorded() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    store
        .perpetual_tables
        .objects
        .multi_insert([
            value(live_id(), 1),
            value(live_id(), 2),
            value(live_id(), 3),
        ])
        .unwrap();
    seed_pruner_watermark(&store, 0);
    seed_checkpoint(&store, &checkpoint_store, 1, &[(live_id(), 1)], &[]);
    seed_checkpoint(&store, &checkpoint_store, 2, &[(live_id(), 2)], &[]);

    // Stand where a run interrupted after checkpoint 1 would have left it.
    store
        .perpetual_tables
        .object_backlog_sweep_checkpoint
        .insert(&(), &1)
        .unwrap();

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH, false)
        .await
        .unwrap();

    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![ObjectKey(live_id(), 2.into())],
        "checkpoint 1 is not walked again, and checkpoint 2 is not skipped"
    );
}

/// Without a watermark there is nothing to bound the walk with, so the whole
/// table is walked — the case of a database no objects pruner ever ran on.
#[tokio::test]
async fn no_watermark_walks_the_whole_table() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed(&store);

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH, false)
        .await
        .unwrap();

    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![
            ObjectKey(live_id(), 1.into()),
            ObjectKey(live_id(), 2.into()),
            ObjectKey(deleted_id(), 1.into()),
            ObjectKey(deleted_id(), 2.into()),
            ObjectKey(wrapped_id(), 1.into()),
        ]
    );
}
