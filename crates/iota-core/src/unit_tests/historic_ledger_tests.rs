// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::TransactionDigest;
use typed_store::{database::wait_for_database_close, traits::Map};

use crate::{
    authority::authority_store_tables::AuthorityPerpetualTables,
    execution_cache::writeback_cache::writeback_cache_tests::Scenario,
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
    let (perpetual, historic_objects, historic) =
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
    drop(historic_objects);
    drop(perpetual);
    assert!(wait_for_database_close(weak_db).await);

    let (_perpetual, _historic_objects, reopened) =
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
