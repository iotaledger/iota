// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::TransactionDigest;
use typed_store::{database::wait_for_database_close, traits::Map};

use crate::authority::authority_store_tables::AuthorityPerpetualTables;

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
