// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use iota_types::{object::Object, storage::ObjectKey};

use super::HistoricObjects;
use crate::authority::authority_store_tables::AuthorityPerpetualTables;

/// A relocated version is readable from the bucket of the epoch it was
/// relocated into, and a version never relocated is absent.
#[tokio::test]
async fn test_relocated_version_is_readable_from_its_bucket() {
    let dir = iota_common::tempdir();
    let perpetual = AuthorityPerpetualTables::open(dir.path(), None);
    let historic = HistoricObjects::open(perpetual.objects.db.clone()).unwrap();

    let object = Object::immutable_with_id_for_testing(ObjectId::random());
    let key = ObjectKey(object.id(), object.version());

    let bucket = historic.ensure(3).unwrap();
    let mut batch = perpetual.objects.batch();
    batch
        .insert_batch_tagged(&bucket.objects, [(key, object.clone())])
        .unwrap();
    batch.write().unwrap();

    assert_eq!(historic.get(&key).unwrap().as_ref(), Some(&object));
    assert_eq!(
        historic
            .get(&ObjectKey(ObjectId::random(), 1.into()))
            .unwrap(),
        None
    );
}

/// Buckets of different epochs are independent, and a lookup finds a key
/// in whichever bucket holds it.
#[tokio::test]
async fn test_lookup_spans_epoch_buckets() {
    let dir = iota_common::tempdir();
    let perpetual = AuthorityPerpetualTables::open(dir.path(), None);
    let historic = HistoricObjects::open(perpetual.objects.db.clone()).unwrap();

    let older = Object::immutable_with_id_for_testing(ObjectId::random());
    let newer = Object::immutable_with_id_for_testing(ObjectId::random());
    let older_key = ObjectKey(older.id(), older.version());
    let newer_key = ObjectKey(newer.id(), newer.version());

    for (epoch, key, object) in [(1, older_key, &older), (2, newer_key, &newer)] {
        let bucket = historic.ensure(epoch).unwrap();
        let mut batch = perpetual.objects.batch();
        batch
            .insert_batch_tagged(&bucket.objects, [(key, (*object).clone())])
            .unwrap();
        batch.write().unwrap();
    }

    assert_eq!(historic.get(&older_key).unwrap().as_ref(), Some(&older));
    assert_eq!(historic.get(&newer_key).unwrap().as_ref(), Some(&newer));
}
