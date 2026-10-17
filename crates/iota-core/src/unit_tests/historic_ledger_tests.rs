// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::{TransactionDigest, TransactionEvents};
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

/// A row written into an epoch's ledger bucket survives a reopen of the
/// database.
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

    // Release every handle on the database before reopening it.
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

/// The reads of a transaction's events and execution record answer a dropped
/// bucket as holding nothing, rather than as an error.
#[tokio::test]
async fn a_read_of_a_dropped_ledger_bucket_finds_nothing() {
    let dir = iota_common::tempdir();
    let (perpetual, _historic_objects, historic, _epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let digest = TransactionDigest::random();
    let bucket = historic.ensure(3).unwrap();
    let mut batch = bucket.executed_effects.batch();
    batch
        .insert_batch_tagged(
            &bucket.executed_effects,
            [(digest, iota_sdk_types::TransactionEffectsDigest::random())],
        )
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.events, [(digest, TransactionEvents::default())])
        .unwrap();
    batch.write().unwrap();
    assert!(historic.get_events(&digest).unwrap().is_some());

    perpetual
        .objects
        .db
        .drop_cf(&crate::epoch_buckets::bucket_cf_name(
            super::HISTORIC_LEDGER_CF_PREFIX,
            3,
        ))
        .unwrap();

    assert_eq!(historic.get_events(&digest).unwrap(), None);
    assert_eq!(historic.get_executed_effects_digest(&digest).unwrap(), None);
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
/// committed it, and later buckets hold nothing of it.
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

        // Both reads must reach the store, not the cache.
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
/// the epoch its effects record, and no other, through either insert path.
#[tokio::test]
async fn a_state_synced_transaction_lands_in_the_executing_epochs_bucket() {
    telemetry_subscribers::init_for_testing();
    Scenario::iterate(|mut s| async move {
        // Distinct from `COMMIT_EPOCH` and the genesis epoch.
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

        // State sync inserts the change-epoch transaction on its own, and a
        // checkpoint's other transactions together.
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

/// The batched reads answer exactly as one read per digest does, for
/// transactions spread over several buckets, digests no bucket holds and a
/// bucket dropped meanwhile, and walk the buckets once per batch.
#[tokio::test]
async fn batched_reads_answer_like_single_reads_and_walk_once() {
    let dir = iota_common::tempdir();
    let (perpetual, _historic_objects, historic, _epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();

    let executed: Vec<_> = [1, 2, 3, 3]
        .into_iter()
        .map(|epoch| write_executed_transaction(&historic, epoch))
        .collect();
    historic.ensure(4).unwrap();

    let check = |label: &str| {
        let mut digests: Vec<_> = executed.iter().map(|(digest, _)| *digest).collect();
        digests.push(TransactionDigest::random());
        let mut effects_digests: Vec<_> = executed.iter().map(|(_, effects)| *effects).collect();
        effects_digests.push(iota_sdk_types::TransactionEffectsDigest::random());

        let before = historic.bucket_walks();
        let records = historic
            .multi_get_executed_effects_digests(&digests)
            .unwrap();
        let effects = historic.multi_get_executed_effects(&digests).unwrap();
        let events = historic.multi_get_events(&digests).unwrap();
        let transactions = historic.multi_get_transactions(&digests).unwrap();
        let by_effects_digest = historic.multi_get_effects(&effects_digests).unwrap();
        assert_eq!(
            historic.bucket_walks() - before,
            5,
            "{label}: each batched read must walk the buckets once"
        );

        let single = |read: &dyn Fn(&TransactionDigest) -> bool| {
            digests.iter().map(read).collect::<Vec<_>>()
        };
        assert_eq!(
            records,
            digests
                .iter()
                .map(|digest| historic.get_executed_effects_digest(digest).unwrap())
                .collect::<Vec<_>>(),
            "{label}: execution records"
        );
        assert_eq!(
            effects,
            digests
                .iter()
                .map(|digest| historic.get_executed_effects(digest).unwrap())
                .collect::<Vec<_>>(),
            "{label}: executed effects"
        );
        assert_eq!(
            events,
            digests
                .iter()
                .map(|digest| historic.get_events(digest).unwrap())
                .collect::<Vec<_>>(),
            "{label}: events"
        );
        assert_eq!(
            transactions.iter().map(Option::is_some).collect::<Vec<_>>(),
            single(&|digest| historic.get_transaction(digest).unwrap().is_some()),
            "{label}: transactions"
        );
        assert_eq!(
            by_effects_digest,
            effects_digests
                .iter()
                .map(|digest| historic.get_effects(digest).unwrap())
                .collect::<Vec<_>>(),
            "{label}: effects by digest"
        );
        records
    };

    let all = check("every bucket");
    assert!(all[..4].iter().all(Option::is_some) && all[4].is_none());

    // A dropped bucket holds nothing, and the rest still answer.
    perpetual
        .objects
        .db
        .drop_cf(&crate::epoch_buckets::bucket_cf_name(
            super::HISTORIC_LEDGER_CF_PREFIX,
            2,
        ))
        .unwrap();
    let dropped = check("a dropped bucket");
    assert_eq!(
        dropped.iter().map(Option::is_some).collect::<Vec<_>>(),
        vec![true, false, true, true, false]
    );
}

/// Writes a transaction executed in `epoch` into that epoch's ledger bucket,
/// with its effects, execution record and events, and returns its digest and
/// effects digest.
fn write_executed_transaction(
    historic: &super::HistoricLedger,
    epoch: u64,
) -> (TransactionDigest, iota_sdk_types::TransactionEffectsDigest) {
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::random_object_ref,
        crypto::{AccountPrivateKey, deterministic_random_account_private_key},
        transaction::VerifiedTransaction,
    };

    let (sender, keypair): (iota_sdk_types::Address, AccountPrivateKey) =
        deterministic_random_account_private_key();
    let transaction = VerifiedTransaction::new_unchecked(
        TestTransactionBuilder::new(sender, random_object_ref(), 100)
            .transfer(random_object_ref(), sender)
            .build_and_sign(&keypair),
    );
    let digest = *transaction.digest();
    let effects = TestEffectsBuilder::new(transaction.inner())
        .with_epoch(epoch)
        .build();
    let effects_digest = effects.digest();
    let bucket = historic.ensure(epoch).unwrap();
    let mut batch = bucket.transactions.batch();
    batch
        .insert_batch_tagged(
            &bucket.transactions,
            [(digest, transaction.serializable_ref())],
        )
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.effects, [(effects_digest, effects)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.executed_effects, [(digest, effects_digest)])
        .unwrap();
    batch
        .insert_batch_tagged(&bucket.events, [(digest, TransactionEvents::default())])
        .unwrap();
    batch.write().unwrap();
    (digest, effects_digest)
}

/// A read of one epoch's execution records finds the transactions executed
/// in that epoch, reports the others as not executed, and walks no other
/// bucket.
#[tokio::test]
async fn an_epochs_read_finds_only_that_epochs_execution_records() {
    let dir = iota_common::tempdir();
    let (_perpetual, _historic_objects, historic, _epoch_markers) =
        AuthorityPerpetualTables::open_with_historic_objects(dir.path(), None).unwrap();
    let (in_one, _) = write_executed_transaction(&historic, 1);
    let (in_two, in_two_effects) = write_executed_transaction(&historic, 2);
    let digests = [in_one, in_two, TransactionDigest::random()];

    let before = historic.bucket_walks();
    assert_eq!(
        historic
            .multi_get_executed_effects_digests_in_epoch(2, &digests)
            .unwrap(),
        vec![None, Some(in_two_effects), None]
    );
    assert_eq!(
        historic
            .multi_get_executed_effects_digests_in_epoch(5, &digests)
            .unwrap(),
        vec![None, None, None],
        "an epoch without a bucket has executed nothing"
    );
    assert_eq!(historic.bucket_walks(), before);
}
