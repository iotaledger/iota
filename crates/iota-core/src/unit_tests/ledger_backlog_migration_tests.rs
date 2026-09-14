// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, sync::Arc};

use iota_sdk_types::{
    Address, CheckpointContentsDigest, TransactionDigest, TransactionEffectsDigest,
    TransactionEvents,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::random_object_ref,
    committee::EpochId,
    crypto::{AccountPrivateKey, deterministic_random_account_private_key},
    effects::TestEffectsBuilder,
    messages_checkpoint::{CheckpointSequenceNumber, FullCheckpointContents, VerifiedCheckpoint},
    transaction::VerifiedTransaction,
};
use prometheus_filtered::Registry;
use typed_store::{database::wait_for_database_close, traits::Map};

use super::{
    CheckpointBacklogMigrationProgress, LedgerBacklogMigration, LedgerBacklogMigrationProgress,
};
use crate::{
    authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables},
    checkpoints::{CheckpointStore, CheckpointWatermark, test_checkpoint_with_contents},
};

/// The epoch the node is starting in while the migration runs, and so the
/// newest epoch the seed below writes history for.
const RUNNING_EPOCH: EpochId = 3;

/// The two values that give the narrowest window a node can really be given:
/// both keep the epoch the migration is running in and the one below it, so
/// with the migration running in epoch 3 they keep epochs 2 and 3 and leave
/// epoch 1 behind.
const NARROWEST_RETENTIONS: [u64; 2] = [0, 1];

/// The epoch the executed and synced watermarks are seeded in. A node
/// restarted between publishing the new epoch and executing its first
/// checkpoint carries watermarks from the epoch below, which is the case the
/// migration's floor has to leave resolvable.
const WATERMARK_EPOCH: EpochId = RUNNING_EPOCH - 1;

/// Where a seeded transaction's epoch is recorded, which is what the
/// migration has to read it back from.
#[derive(Clone, Copy)]
enum EpochSource {
    /// `executed_transactions_to_checkpoint` holds it, as it does on a
    /// fullnode.
    FinalizingCheckpoint,
    /// Only the effects hold it, as on a validator, which never writes that
    /// table.
    Effects,
    /// Nothing on disk holds it: a body persisted or synced but never
    /// executed.
    Nothing,
}

/// One transaction's worth of seeded flat rows, and the epoch the migration
/// must file every one of them under.
struct SeededTransaction {
    digest: TransactionDigest,
    effects_digest: Option<TransactionEffectsDigest>,
    epoch: EpochId,
}

/// What the seed wrote, so the assertions can name it back.
struct Seeded {
    transactions: Vec<SeededTransaction>,
    checkpoints: Vec<VerifiedCheckpoint>,
    /// A contents row the seed deliberately left without a summary, standing
    /// for the crash window between the two writes.
    contents_without_summary: CheckpointContentsDigest,
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
    // The gas object reference is random on every call, so every transaction
    // built here has a digest of its own.
    let transaction = TestTransactionBuilder::new(sender, random_object_ref(), 100)
        .transfer(random_object_ref(), sender)
        .build_and_sign(&keypair);
    VerifiedTransaction::new_unchecked(transaction)
}

/// Writes one transaction's flat rows the way the build before the buckets
/// wrote them.
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

/// Writes one checkpoint's summary and contents into the flat tables the way
/// the build before the buckets wrote them, together with the sequence-keyed
/// summary and the epoch boundary — both of which that build wrote in the same
/// batch as the digest-keyed summary, and neither of which is ever bucketed.
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

/// Writes the flat tables an earlier build would have left behind: three
/// epochs of transaction and checkpoint history, with none of it in a bucket.
///
/// Epoch 2 gets a second transaction whose epoch only its effects record, so
/// that the validator shape — no `executed_transactions_to_checkpoint` row at
/// all — is covered, and the running epoch gets a body with no effects, which
/// nothing on disk places.
///
/// The executed and synced watermarks are left in [`WATERMARK_EPOCH`], as they
/// are on a node restarted before the running epoch's first checkpoint has
/// been executed. Both resolve their checkpoint by digest through the buckets,
/// so a migration that dropped that epoch would leave them unresolvable.
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

    let stray = FullCheckpointContents::random_for_testing();
    let contents_without_summary = stray.checkpoint_contents().digest();
    checkpoint_store
        .tables
        .checkpoint_content
        .insert(&contents_without_summary, &stray.checkpoint_contents())
        .unwrap();

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

/// How many rows are left in the eight flat tables the migration drains.
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
///
/// `earliest_bucket_epoch` is read before anything calls `ensure`, since
/// `ensure` would create the very bucket an absent one is asserted by.
fn assert_migrated(
    store: &AuthorityStore,
    checkpoint_store: &CheckpointStore,
    seeded: &Seeded,
    floor: EpochId,
) {
    let historic_ledger = store.get_historic_ledger();
    let historic_checkpoints = &checkpoint_store.historic_checkpoints;
    // The seed's oldest epoch is 1, so the oldest bucket either store should
    // be left holding is the floor, or 1 when there is no floor.
    let oldest_bucket = Some(floor.max(1));
    assert_eq!(historic_ledger.earliest_bucket_epoch(), oldest_bucket);
    assert_eq!(historic_checkpoints.earliest_bucket_epoch(), oldest_bucket);

    for transaction in &seeded.transactions {
        let digest = &transaction.digest;
        // A body with no execution record names no epoch, so the migration
        // drops it rather than guessing; state sync fetches it again.
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
                "the contents of a summary below the floor must go with it"
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

    // A contents row no summary names cannot be placed, so it is kept under
    // the epoch the migration ran in rather than dropped.
    assert!(
        historic_checkpoints
            .ensure(RUNNING_EPOCH)
            .unwrap()
            .checkpoint_content
            .get(&seeded.contents_without_summary)
            .unwrap()
            .is_some()
    );

    // Whatever the retention, the migration must not delete the epoch the
    // executed and synced watermarks name: both resolve their checkpoint by
    // digest through the buckets, and the checkpoint executor turns an
    // unresolvable executed watermark into a panic on every start.
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
/// belongs to — the epoch its finalizing checkpoint recorded, the epoch its
/// effects recorded where no such row exists, and the running epoch where
/// nothing on disk places it — and the flat tables are left empty.
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

/// A node whose retention has already left epochs behind deletes their rows
/// rather than building buckets the next reconfiguration would drop again,
/// and it reports the checkpoint range it no longer holds.
///
/// Run at both of the values that give the narrowest window there is, since
/// the two must leave the same epochs behind: expiry counts `retained - 1`
/// epochs below its anchor and 0 saturates there, so 0 and 1 both keep the
/// anchor epoch and the running one.
#[tokio::test]
async fn rows_below_a_finite_floor_are_deleted_not_bucketed() {
    for retained in NARROWEST_RETENTIONS {
        let store_dir = iota_common::tempdir();
        let checkpoint_dir = iota_common::tempdir();
        let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
        let seeded = seed(&store, &checkpoint_store);

        migration(&store, checkpoint_store.clone(), Some(retained), 5_000)
            .run()
            .unwrap();

        // The floor the last boundary applied is the epoch below the running
        // one, so only epoch 1 left no bucket behind in either store.
        assert_migrated(&store, &checkpoint_store, &seeded, WATERMARK_EPOCH);

        // Epoch 1's last checkpoint is the highest the node no longer holds,
        // so that a state-sync peer is not told a dropped checkpoint is
        // available.
        assert_eq!(
            checkpoint_store
                .tables
                .watermarks
                .get(&CheckpointWatermark::HighestPruned)
                .unwrap()
                .map(|(sequence, _)| sequence),
            Some(10),
            "retention {retained} must report epoch 1 as pruned"
        );
    }
}

/// Two checkpoints in different epochs can name one contents row — every
/// checkpoint carrying no transaction has the same contents digest — and the
/// flat table holds a single row for the pair. Each epoch's bucket must end up
/// with a copy of its own, or expiring the older epoch would leave the
/// retained checkpoint with a summary and no contents.
///
/// A slice of one puts the two summaries in different slices, so whichever is
/// processed first takes the flat row and the other has to read the contents
/// back out of the bucket that first one went into. Which of the two comes
/// first is decided by the digest order the summaries are walked in, so the
/// assertion covers both.
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

/// With the retention unset there is no floor, so every seeded epoch keeps its
/// own bucket and nothing is deleted.
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

    // Nothing was deleted, so the node claims no pruned range at all.
    assert_eq!(
        checkpoint_store
            .tables
            .watermarks
            .get(&CheckpointWatermark::HighestPruned)
            .unwrap(),
        None
    );
}

/// A run stopped part-way resumes from the watermark it recorded, across a
/// restart, and leaves the same state an uninterrupted run does — the state
/// [`assert_migrated`] describes, which the uninterrupted tests above assert
/// as well.
#[tokio::test]
async fn the_migration_resumes_from_its_watermark() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    let seeded = seed(&store, &checkpoint_store);

    // One slice of one row, so the run stops in the middle of the first of
    // the eight tables: the seed holds five transaction bodies, so four are
    // left for the resumed run to find in that table alone.
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
    // The watermark names a row this run decided, which for a body with an
    // execution record means a bucket and for one without means deletion.
    // Which of the five the single-row slice took depends on digest order, so
    // the seed says which outcome to expect.
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

/// Once both stores' flat tables are drained, a later start does nothing: from
/// then on every row of this history is written straight into its epoch's
/// bucket.
#[tokio::test]
async fn a_finished_migration_leaves_later_starts_nothing_to_do() {
    let store_dir = iota_common::tempdir();
    let checkpoint_dir = iota_common::tempdir();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    migration(&store, checkpoint_store.clone(), None, 5_000)
        .run()
        .unwrap();

    // A row an earlier build could not have written, standing for one a later
    // write puts in the flat table by mistake: a finished migration must not
    // pick it up.
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

/// A node whose state sync has run ahead of execution holds transactions the
/// migration cannot attribute, because only execution records name an epoch.
/// Those rows are dropped, so the synced watermark has to come back to the
/// executed one: the checkpoint executor reads transactions by digest and
/// panics on a missing one, and it takes its work from the synced watermark.
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

    // State sync ran on past what execution reached, staging a body that no
    // execution record places.
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

/// The rewind only ever moves the watermark back. A node whose execution has
/// caught up with its sync — every node that is not behind — must come out of
/// the migration with both watermarks where it left them.
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

/// The rewind belongs to the run that deletes rows, not to every later start.
/// A migrated node keeps whatever state sync has fetched ahead of execution,
/// which on a healthy node is a large and expensive buffer: rewinding it on
/// each restart would make the node fetch those checkpoints again every time.
#[tokio::test]
async fn a_restart_after_the_migration_keeps_the_synced_watermark() {
    let store_dir = tempfile::tempdir().unwrap();
    let checkpoint_dir = tempfile::tempdir().unwrap();
    let (store, checkpoint_store) = open(store_dir.path(), checkpoint_dir.path());
    seed(&store, &checkpoint_store);

    // First start: the migration runs and rewinds, as it must.
    migration(&store, checkpoint_store.clone(), Some(1), 2)
        .run()
        .unwrap();

    // State sync then runs ahead of execution again, as it does on any node
    // that is keeping up.
    let ahead = seed_checkpoint(&checkpoint_store, RUNNING_EPOCH, 91);
    checkpoint_store
        .update_highest_synced_checkpoint(&ahead)
        .unwrap();
    let synced_before_restart = checkpoint_store
        .get_highest_synced_checkpoint_seq_number()
        .unwrap();

    // Second start: nothing left to migrate, so nothing may be given back.
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
