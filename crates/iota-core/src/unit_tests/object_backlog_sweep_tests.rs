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

/// The epoch that is current while the sweep runs.
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
    let (perpetual, historic, historic_ledger) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    AuthorityStore::open_no_genesis(
        Arc::new(perpetual),
        Arc::new(historic),
        Arc::new(historic_ledger),
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

/// Writes a live table in which every object still has its superseded
/// versions next to its newest row.
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
    while sweep.sweep_slice(SWEEP_EPOCH).unwrap().1 {}
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
/// deleted `deleted`, with its effects in the flat perpetual table.
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
    // Unlike `test_utils::certified_summary`, this names the contents above,
    // which the sweep resolves through the summary.
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
        .insert_checkpoint_contents(&checkpoint, contents)
        .unwrap();
    checkpoint_store
        .update_highest_executed_checkpoint(&checkpoint)
        .unwrap();
    effects
}

/// Records the objects pruner's watermark the bounded walk starts from.
fn seed_pruner_watermark(store: &AuthorityStore, watermark: CheckpointSequenceNumber) {
    store
        .perpetual_tables
        .object_backlog_sweep_bound
        .insert(&(), &watermark)
        .unwrap();
}

/// The live table keeps the newest version of every object and every
/// tombstone, including one an unwrap left below a newer version. Superseded
/// versions are relocated into the current epoch's bucket, where each
/// tombstone is recorded too.
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

/// A relocated version is served by the bounded read from the bucket, and one
/// relocated from under a tombstone is never served above that tombstone.
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
        (live_id(), 1, Some(1)),
        (live_id(), 2, Some(2)),
        (live_id(), 3, Some(3)),
        (deleted_id(), 2, Some(2)),
        // At and above the tombstone.
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

/// One call walks the whole table, however many slices that takes.
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

    sweep(store.clone(), empty_checkpoint_store(&dir), SWEEP_EPOCH)
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
    assert_eq!(sweep.sweep_slice(SWEEP_EPOCH).unwrap(), (1, true));
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
/// nothing: from then on, commit relocates superseded versions itself.
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

/// With an objects pruner watermark, the walk relocates exactly the versions
/// the checkpoints above it superseded, and leaves the rows below it alone.
#[tokio::test]
async fn the_bounded_walk_relocates_what_the_checkpoints_above_the_watermark_superseded() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    store
        .perpetual_tables
        .objects
        .multi_insert([
            value(live_id(), 1),
            value(live_id(), 2),
            // A row only the unbounded walk would move.
            value(deleted_id(), 1),
            value(deleted_id(), 2),
        ])
        .unwrap();
    seed_pruner_watermark(&store, 7);
    seed_checkpoint(&store, &checkpoint_store, 8, &[(live_id(), 1)], &[]);

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH)
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

/// A tombstone written above the watermark is recorded in the bucket and
/// left in the live table.
#[tokio::test]
async fn the_bounded_walk_records_the_tombstones_above_the_watermark() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed_pruner_watermark(&store, 3);
    let effects = seed_checkpoint(&store, &checkpoint_store, 4, &[], &[(deleted_id(), 2)]);
    // The tombstone's version is the lamport version the effects decide.
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

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH)
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

#[tokio::test]
async fn a_watermark_below_the_retained_checkpoints_refuses_the_bounded_walk() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed(&store);
    // The checkpoints above the objects pruner's watermark are pruned.
    seed_pruner_watermark(&store, 5);
    checkpoint_store
        .update_highest_pruned_checkpoint(&executed_checkpoint(0, 9))
        .unwrap();

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH)
        .await
        .unwrap();

    // The unbounded walk's outcome.
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

/// The bounded walk resumes at the checkpoint after the last slice it wrote.
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

    store
        .perpetual_tables
        .object_backlog_sweep_checkpoint
        .insert(&(), &1)
        .unwrap();

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH)
        .await
        .unwrap();

    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![ObjectKey(live_id(), 2.into())],
        "checkpoint 1 is not walked again, and checkpoint 2 is not skipped"
    );
}

/// Without an objects pruner watermark, the whole table is walked.
#[tokio::test]
async fn no_watermark_walks_the_whole_table() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    seed(&store);

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH)
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

/// A checkpoint whose effects are committed but which is above the executed
/// watermark, as a crash between the two leaves it, is still walked: the
/// sweep records itself done either way, so a skipped version stays for good.
#[tokio::test]
async fn the_walk_reaches_a_committed_checkpoint_above_the_executed_watermark() {
    let dir = iota_common::tempdir();
    let store = open_store(&dir);
    let checkpoint_store = empty_checkpoint_store(&dir);

    store
        .perpetual_tables
        .objects
        .multi_insert([value(live_id(), 1), value(live_id(), 2)])
        .unwrap();
    seed_pruner_watermark(&store, 7);
    let executed = seed_checkpoint(&store, &checkpoint_store, 8, &[], &[]);
    seed_checkpoint(&store, &checkpoint_store, 9, &[(live_id(), 1)], &[]);
    let _ = executed;

    let eight = checkpoint_store
        .get_checkpoint_by_sequence_number(8)
        .unwrap()
        .unwrap();
    checkpoint_store
        .set_highest_executed_checkpoint_subtle(&eight)
        .unwrap();
    let nine = checkpoint_store
        .get_checkpoint_by_sequence_number(9)
        .unwrap()
        .unwrap();
    checkpoint_store
        .update_highest_synced_checkpoint(&nine)
        .unwrap();

    sweep(store.clone(), checkpoint_store, SWEEP_EPOCH)
        .await
        .unwrap();

    assert_eq!(
        relocated_keys(&store, SWEEP_EPOCH),
        vec![ObjectKey(live_id(), 1.into())],
        "the version checkpoint 9 superseded must be relocated, not left behind",
    );
    assert_eq!(progress(&store), Some(ObjectBacklogSweepProgress::Done));
}
