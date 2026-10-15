// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, sync::Arc};

use iota_sdk_types::{
    Address, CheckpointContents, CheckpointContentsDigest, RandomnessRound, TransactionDigest,
    TransactionEffectsDigest, TransactionEvents, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{ExecutionDigests, random_object_ref},
    committee::EpochId,
    crypto::{AccountPrivateKey, deterministic_random_account_private_key},
    effects::TestEffectsBuilder,
    messages_checkpoint::{
        CheckpointContentsExt, CheckpointSequenceNumber, FullCheckpointContents, VerifiedCheckpoint,
    },
    transaction::VerifiedTransaction,
};
use prometheus_filtered::Registry;
use typed_store::{database::wait_for_database_close, traits::Map};

use super::{
    CheckpointBacklogMigrationProgress, LedgerBacklogMigration, LedgerBacklogMigrationProgress,
};
use crate::{
    authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables},
    checkpoints::{
        CheckpointStore, CheckpointWatermark, EMPTY_CHECKPOINT_CONTENTS_DIGEST,
        test_checkpoint_with_contents,
    },
};

/// The epoch the node starts in while the migration runs, and the newest
/// epoch the seed writes history for.
const RUNNING_EPOCH: EpochId = 3;

/// The narrowest retention a node can be given: it keeps epochs 2 and 3 and
/// leaves epoch 1 behind.
const NARROWEST_RETENTION: u64 = 1;

/// The epoch the executed and synced watermarks are seeded in, as on a node
/// restarted before executing the running epoch's first checkpoint.
const WATERMARK_EPOCH: EpochId = RUNNING_EPOCH - 1;

/// Where a seeded transaction's epoch is recorded.
#[derive(Clone, Copy)]
enum EpochSource {
    /// `executed_transactions_to_checkpoint` holds it, as on a fullnode.
    FinalizingCheckpoint,
    /// Only the effects hold it, as on a validator, which never writes that
    /// table.
    Effects,
    /// Nothing on disk holds it: a body synced but never executed.
    Nothing,
}

/// One transaction's seeded flat rows, and the epoch the migration must file
/// them under.
struct SeededTransaction {
    digest: TransactionDigest,
    effects_digest: Option<TransactionEffectsDigest>,
    epoch: EpochId,
}

/// What the seed wrote.
struct Seeded {
    transactions: Vec<SeededTransaction>,
    checkpoints: Vec<VerifiedCheckpoint>,
    /// Contents rows left without a summary, as after a crash between the two
    /// writes, each with the epoch whose bucket it belongs in.
    contents_without_summary: Vec<(CheckpointContentsDigest, EpochId)>,
}

fn open(store_dir: &Path, checkpoint_dir: &Path) -> (Arc<AuthorityStore>, Arc<CheckpointStore>) {
    let (perpetual, historic_objects, historic_ledger, epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(store_dir, None).unwrap();
    let store = AuthorityStore::open_no_genesis(
        Arc::new(perpetual),
        Arc::new(historic_objects),
        Arc::new(historic_ledger),
        Arc::new(epoch_markers),
        false,
        &Registry::new(),
    )
    .unwrap();
    (store, CheckpointStore::new(checkpoint_dir))
}

fn random_transaction() -> VerifiedTransaction {
    let (sender, keypair): (Address, AccountPrivateKey) =
        deterministic_random_account_private_key();
    let transaction = TestTransactionBuilder::new(sender, random_object_ref(), 100)
        .transfer(random_object_ref(), sender)
        .build_and_sign(&keypair);
    VerifiedTransaction::new_unchecked(transaction)
}

/// Writes one transaction's rows into the flat tables.
fn seed_transaction(
    store: &AuthorityStore,
    epoch: EpochId,
    sequence: CheckpointSequenceNumber,
    source: EpochSource,
) -> SeededTransaction {
    let tables = &store.perpetual_tables;
    let transaction = random_transaction();
    let digest = *transaction.digest();
    tables
        .transactions
        .insert(&digest, transaction.serializable_ref())
        .unwrap();

    if matches!(source, EpochSource::Nothing) {
        return SeededTransaction {
            digest,
            effects_digest: None,
            epoch,
        };
    }

    let effects = TestEffectsBuilder::new(transaction.inner())
        .with_epoch(epoch)
        .build();
    let effects_digest = effects.digest();
    tables.effects.insert(&effects_digest, &effects).unwrap();
    tables
        .executed_effects
        .insert(&digest, &effects_digest)
        .unwrap();
    tables
        .events_2
        .insert(&digest, &TransactionEvents::default())
        .unwrap();
    if matches!(source, EpochSource::FinalizingCheckpoint) {
        tables
            .executed_transactions_to_checkpoint
            .insert(&digest, &(epoch, sequence))
            .unwrap();
    }

    SeededTransaction {
        digest,
        effects_digest: Some(effects_digest),
        epoch,
    }
}

/// Writes one checkpoint's summary and contents into the flat tables,
/// together with the sequence-keyed summary and the epoch boundary, which are
/// never bucketed.
fn seed_checkpoint(
    checkpoint_store: &CheckpointStore,
    epoch: EpochId,
    sequence: CheckpointSequenceNumber,
) -> VerifiedCheckpoint {
    let full_contents = FullCheckpointContents::random_for_testing();
    let checkpoint = test_checkpoint_with_contents(epoch, sequence, &full_contents);
    let tables = &checkpoint_store.tables;
    tables
        .checkpoint_content
        .insert(
            &checkpoint.contents_digest,
            &full_contents.checkpoint_contents(),
        )
        .unwrap();
    tables
        .checkpoint_by_digest
        .insert(checkpoint.digest(), checkpoint.serializable_ref())
        .unwrap();
    tables
        .certified_checkpoints
        .insert(&sequence, checkpoint.serializable_ref())
        .unwrap();
    checkpoint_store
        .insert_epoch_last_checkpoint(epoch, &checkpoint)
        .unwrap();
    checkpoint
}

/// Writes three epochs of transaction and checkpoint history into the flat
/// tables, covering every [`EpochSource`], and leaves the executed and synced
/// watermarks in [`WATERMARK_EPOCH`].
fn seed(store: &AuthorityStore, checkpoint_store: &CheckpointStore) -> Seeded {
    let transactions = vec![
        seed_transaction(store, 1, 10, EpochSource::FinalizingCheckpoint),
        seed_transaction(store, 2, 20, EpochSource::FinalizingCheckpoint),
        seed_transaction(store, 2, 21, EpochSource::Effects),
        seed_transaction(store, RUNNING_EPOCH, 30, EpochSource::FinalizingCheckpoint),
        seed_transaction(store, RUNNING_EPOCH, 31, EpochSource::Nothing),
    ];
    let checkpoints = vec![
        seed_checkpoint(checkpoint_store, 1, 10),
        seed_checkpoint(checkpoint_store, 2, 21),
        seed_checkpoint(checkpoint_store, RUNNING_EPOCH, 30),
    ];

    // One row per epoch the seed executed transactions in, and the row every
    // empty checkpoint shares, which names no transaction.
    let mut strays = vec![(
        CheckpointContents::new_with_digests_only_for_tests([]),
        RUNNING_EPOCH,
    )];
    for transaction in &transactions[..2] {
        let digests = ExecutionDigests::new(
            transaction.digest,
            transaction
                .effects_digest
                .expect("the first two seeded transactions are executed"),
        );
        strays.push((
            CheckpointContents::new_with_digests_only_for_tests([digests]),
            transaction.epoch,
        ));
    }
    let mut contents_without_summary = Vec::new();
    for (contents, epoch) in strays {
        let digest = contents.digest();
        checkpoint_store
            .tables
            .checkpoint_content
            .insert(&digest, &contents)
            .unwrap();
        contents_without_summary.push((digest, epoch));
    }

    let watermarked = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.epoch() == WATERMARK_EPOCH)
        .expect("the seed must hold a checkpoint of the watermark epoch");
    checkpoint_store
        .set_highest_executed_checkpoint_subtle(watermarked)
        .unwrap();
    checkpoint_store
        .update_highest_synced_checkpoint(watermarked)
        .unwrap();

    Seeded {
        transactions,
        checkpoints,
        contents_without_summary,
    }
}

fn migration(
    store: &AuthorityStore,
    checkpoint_store: Arc<CheckpointStore>,
    epochs_to_retain: Option<u64>,
    keys_per_slice: usize,
) -> LedgerBacklogMigration {
    let mut migration =
        LedgerBacklogMigration::new(store, checkpoint_store, RUNNING_EPOCH, epochs_to_retain);
    migration.keys_per_slice = keys_per_slice;
    migration
}

fn ledger_progress(store: &AuthorityStore) -> Option<LedgerBacklogMigrationProgress> {
    store
        .perpetual_tables
        .ledger_backlog_migration_progress
        .get(&())
        .unwrap()
}

fn checkpoint_progress(
    checkpoint_store: &CheckpointStore,
) -> Option<CheckpointBacklogMigrationProgress> {
    checkpoint_store
        .tables
        .checkpoint_backlog_migration_progress
        .get(&())
        .unwrap()
}

/// How many rows are left in the flat tables the migration drains.
fn flat_rows(store: &AuthorityStore, checkpoint_store: &CheckpointStore) -> usize {
    let ledger = &store.perpetual_tables;
    let checkpoints = &checkpoint_store.tables;
    ledger.transactions.safe_iter().count()
        + ledger.effects.safe_iter().count()
        + ledger.executed_effects.safe_iter().count()
        + ledger.events_2.safe_iter().count()
        + ledger
            .executed_transactions_to_checkpoint
            .safe_iter()
            .count()
        + checkpoints.checkpoint_content.safe_iter().count()
        + checkpoints.checkpoint_by_digest.safe_iter().count()
}

/// Asserts that every row of `seeded` whose epoch is at or above `floor` is in
/// that epoch's bucket, that the rows below `floor` are gone, that the
/// executed and synced watermarks still resolve, and that no flat row is left
/// in either store.
fn assert_migrated(
    store: &AuthorityStore,
    checkpoint_store: &CheckpointStore,
    seeded: &Seeded,
    floor: EpochId,
) {
    let historic_ledger = store.get_historic_ledger();
    let historic_checkpoints = &checkpoint_store.historic_checkpoints;
    // Read before any `ensure`, which would create the bucket asserted absent.
    let oldest_bucket = Some(floor.max(1));
    assert_eq!(historic_ledger.earliest_bucket_epoch(), oldest_bucket);
    assert_eq!(historic_checkpoints.earliest_bucket_epoch(), oldest_bucket);

    for transaction in &seeded.transactions {
        let digest = &transaction.digest;
        // A body with no execution record names no epoch; state sync fetches
        // it again.
        if transaction.effects_digest.is_none() {
            assert!(
                historic_ledger.get_transaction(digest).unwrap().is_none(),
                "a body with no execution record must not be filed in any bucket"
            );
            assert!(
                store
                    .perpetual_tables
                    .transactions
                    .get(digest)
                    .unwrap()
                    .is_none(),
                "a body with no execution record must not be left flat either"
            );
            continue;
        }
        if transaction.epoch < floor {
            assert_eq!(
                historic_ledger.find_epoch(digest).unwrap().map(|(e, _)| e),
                None,
                "the record of a transaction below the floor must be gone"
            );
            assert!(historic_ledger.get_transaction(digest).unwrap().is_none());
            continue;
        }

        let bucket = historic_ledger.ensure(transaction.epoch).unwrap();
        assert!(
            bucket.transactions.get(digest).unwrap().is_some(),
            "the body of {digest} belongs in epoch {}'s bucket",
            transaction.epoch
        );
        let effects_digest = transaction
            .effects_digest
            .expect("bodies with no execution record are handled above");
        assert_eq!(
            historic_ledger.find_epoch(digest).unwrap().map(|(e, _)| e),
            Some(transaction.epoch),
            "the execution record of {digest} names the wrong epoch"
        );
        assert!(bucket.effects.get(&effects_digest).unwrap().is_some());
        assert_eq!(
            bucket.executed_effects.get(digest).unwrap(),
            Some(effects_digest)
        );
        assert!(bucket.events.get(digest).unwrap().is_some());
        assert!(
            historic_ledger
                .get_executed_effects(digest)
                .unwrap()
                .is_some(),
            "the store's own read must resolve {digest} out of one bucket"
        );
    }

    for checkpoint in &seeded.checkpoints {
        let epoch = checkpoint.epoch();
        if epoch < floor {
            assert!(
                historic_checkpoints
                    .find_by_digest(checkpoint.digest())
                    .unwrap()
                    .is_none(),
                "a summary below the floor must be gone"
            );
            assert!(
                historic_checkpoints
                    .find_contents(&checkpoint.contents_digest)
                    .unwrap()
                    .is_none(),
                "the contents of a summary below the floor must be gone"
            );
            continue;
        }
        let bucket = historic_checkpoints.ensure(epoch).unwrap();
        assert!(
            bucket
                .checkpoint_by_digest
                .get(checkpoint.digest())
                .unwrap()
                .is_some(),
            "the summary of checkpoint {} belongs in epoch {epoch}'s bucket",
            checkpoint.sequence_number()
        );
        assert!(
            bucket
                .checkpoint_content
                .get(&checkpoint.contents_digest)
                .unwrap()
                .is_some(),
            "the contents of checkpoint {} belong in its summary's bucket",
            checkpoint.sequence_number()
        );
    }

    for (digest, epoch) in &seeded.contents_without_summary {
        if *epoch < floor {
            assert!(
                historic_checkpoints
                    .find_contents(digest)
                    .unwrap()
                    .is_none(),
                "contents naming transactions below the floor must be gone"
            );
            continue;
        }
        assert!(
            historic_checkpoints
                .ensure(*epoch)
                .unwrap()
                .checkpoint_content
                .get(digest)
                .unwrap()
                .is_some(),
            "contents no summary names belong in epoch {epoch}'s bucket"
        );
    }

    // The checkpoint executor panics on start if the executed watermark does
    // not resolve.
    let watermarked = seeded
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.epoch() == WATERMARK_EPOCH)
        .expect("the seed must hold a checkpoint of the watermark epoch");
    assert_eq!(
        checkpoint_store
            .get_highest_executed_checkpoint()
            .unwrap()
            .map(|checkpoint| *checkpoint.digest()),
        Some(*watermarked.digest()),
        "the executed watermark must still resolve after the migration"
    );
    assert_eq!(
        checkpoint_store
            .get_highest_synced_checkpoint()
            .unwrap()
            .map(|checkpoint| *checkpoint.digest()),
        Some(*watermarked.digest()),
        "the synced watermark must still resolve after the migration"
    );

    assert_eq!(flat_rows(store, checkpoint_store), 0);
    assert_eq!(
        ledger_progress(store),
        Some(LedgerBacklogMigrationProgress::Done)
    );
    assert_eq!(
        checkpoint_progress(checkpoint_store),
        Some(CheckpointBacklogMigrationProgress::Done)
    );
}

/// With no retention limit every row lands in the bucket of the epoch it
/// belongs to, and the flat tables are left empty.
#[tokio::test]
async fn rows_land_in_their_true_epoch() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    let seeded = seed(&store, &checkpoint_store);

    migration(&store, checkpoint_store.clone(), None, 5_000)
        .run()
        .unwrap();

    assert_migrated(&store, &checkpoint_store, &seeded, 0);
}

/// Rows of epochs outside the retention are deleted rather than bucketed,
/// and the checkpoint range no longer held is reported as pruned.
#[tokio::test]
async fn rows_below_a_finite_floor_are_deleted_not_bucketed() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    let seeded = seed(&store, &checkpoint_store);

    migration(
        &store,
        checkpoint_store.clone(),
        Some(NARROWEST_RETENTION),
        5_000,
    )
    .run()
    .unwrap();

    assert_migrated(&store, &checkpoint_store, &seeded, WATERMARK_EPOCH);

    // Otherwise a state-sync peer would be told a dropped checkpoint is
    // available.
    assert_eq!(
        checkpoint_store
            .tables
            .watermarks
            .get(&CheckpointWatermark::HighestPruned)
            .unwrap()
            .map(|(sequence, _)| sequence),
        Some(10),
        "epoch 1 must be reported as pruned"
    );
}

/// Two checkpoints in different epochs naming one contents row each get a
/// copy in their own epoch's bucket, so expiring the older epoch leaves the
/// newer one its contents. A slice of one puts the two summaries in
/// different slices.
#[tokio::test]
async fn two_epochs_naming_one_contents_row_each_keep_a_copy() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());

    let full_contents = FullCheckpointContents::random_for_testing();
    let older = test_checkpoint_with_contents(1, 10, &full_contents);
    let newer = test_checkpoint_with_contents(2, 20, &full_contents);
    let contents_digest = older.contents_digest;
    assert_eq!(
        newer.contents_digest, contents_digest,
        "the two checkpoints must name one contents row for this to model the case"
    );
    let tables = &checkpoint_store.tables;
    tables
        .checkpoint_content
        .insert(&contents_digest, &full_contents.checkpoint_contents())
        .unwrap();
    for checkpoint in [&older, &newer] {
        tables
            .checkpoint_by_digest
            .insert(checkpoint.digest(), checkpoint.serializable_ref())
            .unwrap();
    }

    migration(&store, checkpoint_store.clone(), None, 1)
        .run()
        .unwrap();

    for checkpoint in [&older, &newer] {
        assert!(
            checkpoint_store
                .historic_checkpoints
                .ensure(checkpoint.epoch())
                .unwrap()
                .checkpoint_content
                .get(&contents_digest)
                .unwrap()
                .is_some(),
            "epoch {} must hold its own copy of the shared contents",
            checkpoint.epoch()
        );
    }
    assert_eq!(tables.checkpoint_content.safe_iter().count(), 0);
}

/// With the retention unset every seeded epoch keeps its own bucket and
/// nothing is deleted.
#[tokio::test]
async fn unlimited_retention_buckets_every_epoch() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    let seeded = seed(&store, &checkpoint_store);

    migration(&store, checkpoint_store.clone(), None, 5_000)
        .run()
        .unwrap();

    let historic_ledger = store.get_historic_ledger();
    assert_eq!(historic_ledger.earliest_bucket_epoch(), Some(1));
    for epoch in 1..=RUNNING_EPOCH {
        assert!(
            historic_ledger
                .ensure(epoch)
                .unwrap()
                .transactions
                .safe_iter()
                .next()
                .is_some(),
            "epoch {epoch} must hold the transactions it executed"
        );
    }
    assert_eq!(
        checkpoint_store
            .historic_checkpoints
            .earliest_bucket_epoch(),
        Some(1)
    );
    for checkpoint in &seeded.checkpoints {
        assert!(
            checkpoint_store
                .historic_checkpoints
                .ensure(checkpoint.epoch())
                .unwrap()
                .checkpoint_by_digest
                .get(checkpoint.digest())
                .unwrap()
                .is_some()
        );
    }

    assert_eq!(
        checkpoint_store
            .tables
            .watermarks
            .get(&CheckpointWatermark::HighestPruned)
            .unwrap(),
        None
    );
}

/// A run stopped part-way resumes from its recorded watermark across a
/// restart and leaves the same state an uninterrupted run does.
#[tokio::test]
async fn the_migration_resumes_from_its_watermark() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    let seeded = seed(&store, &checkpoint_store);

    // One slice of one row stops the run inside the first table, leaving four
    // of the five seeded bodies.
    let interrupted = migration(&store, checkpoint_store.clone(), None, 1);
    interrupted.move_transactions(None).unwrap();
    let watermark = match ledger_progress(&store) {
        Some(LedgerBacklogMigrationProgress::Transactions(Some(digest))) => digest,
        other => panic!("the interrupted run must have recorded a watermark, got {other:?}"),
    };
    assert_eq!(
        store.perpetual_tables.transactions.safe_iter().count(),
        4,
        "one row moved and four left, or the slice size is not being honoured"
    );
    // Which row the slice took depends on digest order, so the seed says
    // whether it should have been bucketed or deleted.
    let attributable = seeded
        .transactions
        .iter()
        .find(|transaction| transaction.digest == watermark)
        .expect("the watermark must name a seeded transaction")
        .effects_digest
        .is_some();
    let bucketed = store
        .get_historic_ledger()
        .get_transaction(&watermark)
        .unwrap()
        .is_some();
    assert_eq!(
        bucketed, attributable,
        "the row the watermark names must be in a bucket when an execution \
         record places it, and gone when none does"
    );
    assert!(
        store
            .perpetual_tables
            .transactions
            .get(&watermark)
            .unwrap()
            .is_none(),
        "the row the watermark names must have left the flat table either way"
    );

    // Release every handle on both databases before reopening the same paths,
    // as a restart does.
    let weak_ledger = Arc::downgrade(&store.perpetual_tables.objects.db);
    let weak_checkpoints = Arc::downgrade(&checkpoint_store.tables.certified_checkpoints.db);
    drop(interrupted);
    drop(store);
    drop(checkpoint_store);
    assert!(wait_for_database_close(weak_ledger).await);
    assert!(wait_for_database_close(weak_checkpoints).await);

    let (resumed_store, resumed_checkpoints) = open(store_dir.path(), checkpoint_dir.path());
    migration(&resumed_store, resumed_checkpoints.clone(), None, 1)
        .run()
        .unwrap();

    assert_migrated(&resumed_store, &resumed_checkpoints, &seeded, 0);
}

/// Once both stores' flat tables are drained, a later start does nothing.
#[tokio::test]
async fn a_finished_migration_leaves_later_starts_nothing_to_do() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    migration(&store, checkpoint_store.clone(), None, 5_000)
        .run()
        .unwrap();

    // A row written to the flat table after the migration finished must not
    // be picked up.
    let stray = random_transaction();
    store
        .perpetual_tables
        .transactions
        .insert(stray.digest(), stray.serializable_ref())
        .unwrap();

    migration(&store, checkpoint_store, None, 5_000)
        .run()
        .unwrap();

    assert!(
        store
            .perpetual_tables
            .transactions
            .get(stray.digest())
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .get_historic_ledger()
            .get_transaction(stray.digest())
            .unwrap()
            .is_none()
    );
}

/// Transactions synced ahead of execution name no epoch and are dropped, so
/// the synced watermark is rewound to the executed one; otherwise the
/// checkpoint executor would panic on the missing transactions.
#[tokio::test]
async fn the_synced_watermark_rewinds_so_dropped_checkpoints_are_fetched_again() {
    let store_dir = tempfile::tempdir().unwrap();
    let checkpoint_dir = tempfile::tempdir().unwrap();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    let executed = checkpoint_store
        .get_highest_executed_checkpoint_seq_number()
        .unwrap()
        .expect("the seed sets the executed watermark");

    let ahead = seed_checkpoint(&checkpoint_store, RUNNING_EPOCH, 90);
    let staged = seed_transaction(&store, RUNNING_EPOCH, 90, EpochSource::Nothing);
    checkpoint_store
        .update_highest_synced_checkpoint(&ahead)
        .unwrap();
    assert!(
        checkpoint_store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap()
            > Some(executed),
        "the fixture must leave state sync ahead of execution"
    );

    migration(&store, checkpoint_store.clone(), Some(1), 2)
        .run()
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap(),
        Some(executed),
        "the synced watermark must come back to the executed checkpoint"
    );
    assert!(
        store
            .get_historic_ledger()
            .get_transaction(&staged.digest)
            .unwrap()
            .is_none(),
        "the staged body must not be filed under an epoch nothing recorded for it"
    );
}

/// The rewind only moves the synced watermark back, so a node whose
/// execution has caught up with its sync keeps it.
#[tokio::test]
async fn a_node_that_is_not_behind_keeps_its_synced_watermark() {
    let store_dir = tempfile::tempdir().unwrap();
    let checkpoint_dir = tempfile::tempdir().unwrap();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    let before = checkpoint_store
        .get_highest_synced_checkpoint_seq_number()
        .unwrap();
    assert_eq!(
        before,
        checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .unwrap(),
        "the seed must leave the two watermarks together"
    );

    migration(&store, checkpoint_store.clone(), Some(1), 2)
        .run()
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap(),
        before
    );
}

/// Only the run that deletes rows rewinds the synced watermark; a later
/// start must not, or the node would fetch those checkpoints again on every
/// restart.
#[tokio::test]
async fn a_restart_after_the_migration_keeps_the_synced_watermark() {
    let store_dir = tempfile::tempdir().unwrap();
    let checkpoint_dir = tempfile::tempdir().unwrap();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    migration(&store, checkpoint_store.clone(), Some(1), 2)
        .run()
        .unwrap();

    let ahead = seed_checkpoint(&checkpoint_store, RUNNING_EPOCH, 91);
    checkpoint_store
        .update_highest_synced_checkpoint(&ahead)
        .unwrap();
    let synced_before_restart = checkpoint_store
        .get_highest_synced_checkpoint_seq_number()
        .unwrap();

    migration(&store, checkpoint_store.clone(), Some(1), 2)
        .run()
        .unwrap();

    assert_eq!(
        checkpoint_store
            .get_highest_synced_checkpoint_seq_number()
            .unwrap(),
        synced_before_restart,
        "a restart of a migrated node must not rewind the synced watermark"
    );
}

/// A contents row shared by an expired checkpoint and a retained one
/// survives for the retained one. A slice of one puts the two summaries in
/// different slices.
#[tokio::test]
async fn an_expired_checkpoint_does_not_take_a_retained_one_s_contents() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());

    let full_contents = FullCheckpointContents::random_for_testing();
    // Epoch 1 is below the floor at retention 1, epoch 2 is the last retained.
    let expired = test_checkpoint_with_contents(1, 10, &full_contents);
    let retained = test_checkpoint_with_contents(WATERMARK_EPOCH, 20, &full_contents);
    let contents_digest = expired.contents_digest;
    assert_eq!(retained.contents_digest, contents_digest);

    let tables = &checkpoint_store.tables;
    tables
        .checkpoint_content
        .insert(&contents_digest, &full_contents.checkpoint_contents())
        .unwrap();
    for checkpoint in [&expired, &retained] {
        tables
            .checkpoint_by_digest
            .insert(checkpoint.digest(), checkpoint.serializable_ref())
            .unwrap();
    }

    migration(&store, checkpoint_store.clone(), Some(1), 1)
        .run()
        .unwrap();

    assert!(
        checkpoint_store
            .historic_checkpoints
            .ensure(WATERMARK_EPOCH)
            .unwrap()
            .checkpoint_content
            .get(&contents_digest)
            .unwrap()
            .is_some(),
        "the retained checkpoint must keep the contents it shares with the expired one",
    );
}

/// A retained empty checkpoint whose shared contents row was deleted by the
/// checkpoint pruner gets its contents back in its bucket.
#[tokio::test]
async fn a_retained_empty_checkpoint_gets_back_the_contents_row_a_pruner_deleted() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());

    let empty = FullCheckpointContents::new_with_causally_ordered_transactions([]);
    let checkpoint = test_checkpoint_with_contents(WATERMARK_EPOCH, 20, &empty);
    assert_eq!(
        checkpoint.contents_digest,
        *EMPTY_CHECKPOINT_CONTENTS_DIGEST
    );
    // The summary is on disk; its contents row is not.
    checkpoint_store
        .tables
        .checkpoint_by_digest
        .insert(checkpoint.digest(), checkpoint.serializable_ref())
        .unwrap();

    migration(&store, checkpoint_store.clone(), None, 1)
        .run()
        .unwrap();

    assert!(
        checkpoint_store
            .get_checkpoint_contents(&EMPTY_CHECKPOINT_CONTENTS_DIGEST)
            .unwrap()
            .is_some_and(|contents| contents.is_empty()),
        "the retained empty checkpoint must have its contents after the migration"
    );
}

/// A run resumed under a longer retention keeps the floor of the run that
/// started the migration: the epoch that run had begun deleting goes
/// entirely, and is reported as pruned.
#[tokio::test]
async fn a_resumed_run_keeps_the_floor_the_migration_started_with() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    let seeded = seed(&store, &checkpoint_store);

    let mut interrupted = migration(
        &store,
        checkpoint_store.clone(),
        Some(NARROWEST_RETENTION),
        1,
    );
    interrupted.pin_floor().unwrap();
    interrupted.move_transactions(None).unwrap();

    migration(&store, checkpoint_store.clone(), None, 5_000)
        .run()
        .unwrap();

    assert_migrated(&store, &checkpoint_store, &seeded, WATERMARK_EPOCH);
    assert_eq!(
        checkpoint_store
            .tables
            .watermarks
            .get(&CheckpointWatermark::HighestPruned)
            .unwrap()
            .map(|(sequence, _)| sequence),
        Some(10),
        "the epoch the first run deleted rows of must be reported as pruned"
    );
}

/// A randomness state update the node stored but has not executed is filed
/// under the epoch it names rather than deleted, since a validator may not be
/// able to build it again.
#[tokio::test]
async fn an_unexecuted_randomness_update_is_kept() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    let update = VerifiedTransaction::new_randomness_state_update(
        RUNNING_EPOCH,
        RandomnessRound::new(7),
        vec![1, 2, 3],
        Version::from_u64(1),
    );
    let digest = *update.digest();
    store
        .perpetual_tables
        .transactions
        .insert(&digest, update.serializable_ref())
        .unwrap();

    migration(&store, checkpoint_store, Some(NARROWEST_RETENTION), 5_000)
        .run()
        .unwrap();

    let historic_ledger = store.get_historic_ledger();
    assert!(
        historic_ledger
            .ensure(RUNNING_EPOCH)
            .unwrap()
            .transactions
            .get(&digest)
            .unwrap()
            .is_some(),
        "the update must be filed under the epoch it names"
    );
}
