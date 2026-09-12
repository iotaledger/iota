// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::TransactionDigest;
use iota_types::{base_types::VerifiedExecutionData, effects::TestEffectsBuilder};
use typed_store::{database::wait_for_database_close, traits::Map};

use crate::{
    authority::authority_store_tables::AuthorityPerpetualTables,
    execution_cache::{TransactionCacheRead, writeback_cache::writeback_cache_tests::Scenario},
};

/// The epoch `Scenario` writes and commits its transactions in.
const COMMIT_EPOCH: u64 = 1;

/// The checkpoint the tests below finalize their transaction at.
const CHECKPOINT: u64 = 7;

/// A row written into an epoch's ledger bucket survives a restart: the next
/// open rediscovers the bucket's column family on disk instead of serving an
/// empty store.
#[tokio::test]
async fn ledger_rows_survive_a_reopen() {
    let dir = iota_common::tempdir();
    let (perpetual, historic_objects, historic, epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let digest = TransactionDigest::random();
    let bucket = historic.ensure(3).unwrap();
    let mut batch = bucket.tx_to_checkpoint.batch();
    batch
        .insert_batch_tagged(&bucket.tx_to_checkpoint, [(digest, 42u64)])
        .unwrap();
    batch.write().unwrap();

    // Release every handle on the database before reopening the same path,
    // as a restart does.
    let weak_db = Arc::downgrade(&perpetual.objects.db);
    drop(bucket);
    drop(historic);
    drop(epoch_markers);
    drop(historic_objects);
    drop(perpetual);
    assert!(wait_for_database_close(weak_db).await);

    let (_perpetual, _historic_objects, reopened, _epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    assert_eq!(reopened.earliest_bucket_epoch(), Some(3));
    assert_eq!(
        reopened
            .ensure(3)
            .unwrap()
            .tx_to_checkpoint
            .get(&digest)
            .unwrap(),
        Some(42)
    );
}

/// Everything the commit path writes for a transaction goes into the bucket of
/// the epoch that committed it, and nothing of it is left in the flat
/// perpetual tables.
#[tokio::test]
async fn a_committed_transaction_is_read_from_its_epoch_bucket() {
    telemetry_subscribers::init_for_testing();
    Scenario::iterate(|mut s| async move {
        s.with_created(&[1]);
        s.with_events();
        let digest = s.do_tx().await;
        s.commit(digest).await;
        s.store
            .insert_finalized_transactions_perpetual_checkpoints(
                &[digest],
                COMMIT_EPOCH,
                CHECKPOINT,
            )
            .unwrap();

        let (epoch, bucket) = s
            .store
            .get_historic_ledger()
            .find_epoch(&digest)
            .unwrap()
            .expect("a committed transaction must be found in a bucket");
        assert_eq!(epoch, COMMIT_EPOCH);

        let effects_digest = bucket
            .executed_effects
            .get(&digest)
            .unwrap()
            .expect("the execution record must be in the bucket");
        assert!(
            bucket.transactions.get(&digest).unwrap().is_some(),
            "the transaction must be in the bucket"
        );
        assert!(
            bucket.effects.get(&effects_digest).unwrap().is_some(),
            "the effects must be in the same bucket as the execution record"
        );
        assert!(
            bucket.events.get(&digest).unwrap().is_some(),
            "the events must be in the bucket"
        );
        assert_eq!(
            bucket.tx_to_checkpoint.get(&digest).unwrap(),
            Some(CHECKPOINT),
            "the finalizing checkpoint must be in the bucket"
        );

        let flat = &s.store.perpetual_tables;
        assert!(flat.transactions.get(&digest).unwrap().is_none());
        assert!(flat.effects.get(&effects_digest).unwrap().is_none());
        assert_eq!(flat.executed_effects.get(&digest).unwrap(), None);
        assert!(flat.events_2.get(&digest).unwrap().is_none());
        assert_eq!(
            flat.executed_transactions_to_checkpoint
                .get(&digest)
                .unwrap(),
            None
        );

        // The store's own reads must return what the bucket holds.
        assert!(s.store.get_transaction_block(&digest).unwrap().is_some());
        assert!(s.store.get_executed_effects(&digest).unwrap().is_some());
        assert!(s.store.get_events(&digest).unwrap().is_some());
        assert_eq!(
            s.store
                .get_transaction_perpetual_checkpoint(&digest)
                .unwrap(),
            Some((COMMIT_EPOCH, CHECKPOINT))
        );
    })
    .await;
}

/// A transaction's whole record stays in the bucket of the epoch that
/// committed it, however many epochs follow: one `find_epoch` names that
/// bucket and every table for the transaction is read out of it, with the
/// later buckets holding nothing for it.
#[tokio::test]
async fn one_probe_resolves_every_table_for_a_transaction() {
    telemetry_subscribers::init_for_testing();
    Scenario::iterate(|mut s| async move {
        s.with_created(&[1]);
        s.with_events();
        let digest = s.do_tx().await;
        s.commit(digest).await;
        s.store
            .insert_finalized_transactions_perpetual_checkpoints(
                &[digest],
                COMMIT_EPOCH,
                CHECKPOINT,
            )
            .unwrap();

        let ledger = s.store.get_historic_ledger();
        let later_epochs = [COMMIT_EPOCH + 1, COMMIT_EPOCH + 2];
        let later_buckets: Vec<_> = later_epochs
            .iter()
            .map(|&epoch| ledger.ensure(epoch).unwrap())
            .collect();

        let (epoch, bucket) = ledger
            .find_epoch(&digest)
            .unwrap()
            .expect("the committing epoch's bucket must still answer");
        assert_eq!(
            epoch, COMMIT_EPOCH,
            "the newer epochs must not shadow the one that committed the transaction"
        );

        let effects_digest = bucket.executed_effects.get(&digest).unwrap().unwrap();
        assert!(bucket.effects.get(&effects_digest).unwrap().is_some());
        assert!(bucket.events.get(&digest).unwrap().is_some());
        assert_eq!(
            bucket.tx_to_checkpoint.get(&digest).unwrap(),
            Some(CHECKPOINT)
        );

        for (epoch, later) in later_epochs.iter().zip(&later_buckets) {
            assert!(
                later.transactions.get(&digest).unwrap().is_none()
                    && later.executed_effects.get(&digest).unwrap().is_none()
                    && later.effects.get(&effects_digest).unwrap().is_none()
                    && later.events.get(&digest).unwrap().is_none()
                    && later.tx_to_checkpoint.get(&digest).unwrap().is_none(),
                "epoch {epoch}'s bucket must hold no part of the transaction's record"
            );
        }
    })
    .await;
}

/// Reading a committed transaction's effects through the cache walks the
/// buckets once, not once for the execution record and again for the effects.
#[tokio::test]
async fn reading_a_committed_transactions_effects_walks_the_buckets_once() {
    telemetry_subscribers::init_for_testing();
    Scenario::iterate(|mut s| async move {
        s.with_created(&[1]);
        let digest = s.do_tx().await;
        s.commit(digest).await;

        // Every later epoch is another bucket a walk has to visit, so the
        // count below is the walk count, not the bucket count.
        for epoch in [COMMIT_EPOCH + 1, COMMIT_EPOCH + 2] {
            s.store.get_historic_ledger().ensure(epoch).unwrap();
        }

        // Both reads must reach the store rather than the cache the commit
        // populated.
        s.evict_caches();
        let before = s.store.get_historic_ledger().bucket_walks();
        assert!(
            s.cache.multi_get_executed_effects(&[digest])[0].is_some(),
            "the committed effects must be readable"
        );
        assert_eq!(
            s.store.get_historic_ledger().bucket_walks() - before,
            1,
            "reading a transaction's effects must walk the buckets once"
        );

        s.evict_caches();
        let before = s.store.get_historic_ledger().bucket_walks();
        s.cache
            .try_notify_read_executed_effects("test", &[digest])
            .await
            .unwrap();
        assert_eq!(
            s.store.get_historic_ledger().bucket_walks() - before,
            1,
            "waiting on an already-executed transaction's effects must walk \
             the buckets once"
        );
    })
    .await;
}

/// A transaction state sync records ahead of execution goes into the bucket of
/// the epoch that executed it — the epoch its effects record — and into no
/// other, whether it arrives on its own or as part of a checkpoint's contents.
#[tokio::test]
async fn a_state_synced_transaction_lands_in_the_executing_epochs_bucket() {
    telemetry_subscribers::init_for_testing();
    Scenario::iterate(|mut s| async move {
        // An epoch of its own, so that landing in it cannot be confused with
        // landing in the epoch `Scenario` commits in or the genesis epoch.
        let executed_in = COMMIT_EPOCH + 4;
        let ledger = s.store.get_historic_ledger().clone();
        let other_bucket = ledger.ensure(executed_in + 1).unwrap();

        let mut synced = Vec::new();
        for _ in 0..2 {
            let outputs = s.take_outputs();
            let transaction = (*outputs.transaction).clone();
            let effects = TestEffectsBuilder::new(transaction.inner())
                .with_epoch(executed_in)
                .build();
            synced.push((transaction, effects));
        }

        // State sync inserts a checkpoint's whole contents; the change-epoch
        // transaction arrives on its own.
        let (transaction, effects) = synced.pop().unwrap();
        s.store
            .insert_transaction_and_effects(&transaction, &effects)
            .unwrap();
        let mut inserted = vec![(*transaction.digest(), effects.digest())];

        let (transaction, effects) = synced.pop().unwrap();
        inserted.push((*transaction.digest(), effects.digest()));
        s.store
            .multi_insert_transaction_and_effects(
                [VerifiedExecutionData::new(transaction, effects)].iter(),
            )
            .unwrap();

        let bucket = ledger.ensure(executed_in).unwrap();
        for (digest, effects_digest) in inserted {
            assert!(
                bucket.transactions.get(&digest).unwrap().is_some(),
                "the transaction must be in the bucket of the epoch that executed it"
            );
            assert!(
                bucket.effects.get(&effects_digest).unwrap().is_some(),
                "the effects must be in the bucket of the epoch that produced them"
            );
            assert!(
                other_bucket.transactions.get(&digest).unwrap().is_none()
                    && other_bucket.effects.get(&effects_digest).unwrap().is_none(),
                "no other epoch's bucket may hold the transaction or its effects"
            );

            // The rows are not an execution record: nothing has executed it.
            assert!(ledger.find_epoch(&digest).unwrap().is_none());
        }
    })
    .await;
}
