// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::{Bound, RangeBounds};

use rstest::rstest;
use serde::{Serialize, de::DeserializeOwned};

use super::*;
use crate::traits::Map;

fn get_iter<K, V>(db: &DBMap<K, V>) -> impl Iterator<Item = (K, V)> + use<'_, K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    db.safe_iter().map(|item| item.unwrap())
}

fn get_reverse_iter<'a, K, V>(
    db: &'a DBMap<K, V>,
    range: impl RangeBounds<K> + 'a,
) -> impl Iterator<Item = Result<(K, V), TypedStoreError>> + 'a
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    db.safe_range_iter_reversed(range)
}

fn get_iter_with_bounds<K, V>(
    db: &DBMap<K, V>,
    lower_bound: Option<K>,
    upper_bound: Option<K>,
) -> impl Iterator<Item = (K, V)> + use<'_, K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    db.safe_iter_with_bounds(lower_bound, upper_bound)
        .map(|item| item.unwrap())
}

fn get_range_iter<'a, K, V>(
    db: &'a DBMap<K, V>,
    range: impl RangeBounds<K> + 'a,
) -> impl Iterator<Item = (K, V)> + 'a
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    db.safe_range_iter(range).map(|item| item.unwrap())
}

#[tokio::test]
async fn test_open() {
    let tmp_dir = iota_common::tempdir();
    let _db = open_map::<_, u32, String>(tmp_dir.path(), None);
}

#[tokio::test]
async fn test_reopen() {
    let tmp_dir = iota_common::tempdir();
    let arc = {
        let db = open_map::<_, u32, String>(tmp_dir.path(), None);
        db.insert(&123456789, &"123456789".to_string())
            .expect("Failed to insert");
        db
    };
    let db = DBMap::<u32, String>::reopen(&arc.db, None, &ReadWriteOptions::default(), false)
        .expect("Failed to re-open storage");
    assert!(
        db.contains_key(&123456789)
            .expect("Failed to retrieve item in storage")
    );
}

#[tokio::test]
async fn test_contains_key() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&123456789, &"123456789".to_string())
        .expect("Failed to insert");
    assert!(
        db.contains_key(&123456789)
            .expect("Failed to call contains key")
    );
    assert!(
        !db.contains_key(&000000000)
            .expect("Failed to call contains key")
    );
}

#[tokio::test]
async fn test_safe_drop_db() {
    let tmp_dir = iota_common::tempdir();
    let path = tmp_dir.path();
    {
        let db: DBMap<i32, String> = open_map(path, Some("table"));
        db.insert(&777, &"123".to_string()).unwrap();
    }
    assert!(
        safe_drop_db(path.to_path_buf(), Duration::from_secs(5))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_multi_contain() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&123, &"123".to_string())
        .expect("Failed to insert");
    db.insert(&456, &"456".to_string())
        .expect("Failed to insert");
    db.insert(&789, &"789".to_string())
        .expect("Failed to insert");

    let result = db
        .multi_contains_keys([123, 456])
        .expect("Failed to check multi keys existence");

    assert_eq!(result.len(), 2);
    assert!(result[0]);
    assert!(result[1]);

    let result = db
        .multi_contains_keys([123, 987, 789])
        .expect("Failed to check multi keys existence");

    assert_eq!(result.len(), 3);
    assert!(result[0]);
    assert!(!result[1]);
    assert!(result[2]);
}

#[tokio::test]
async fn test_get() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&123456789, &"123456789".to_string())
        .expect("Failed to insert");
    assert_eq!(
        Some("123456789".to_string()),
        db.get(&123456789).expect("Failed to get")
    );
    assert_eq!(None, db.get(&000000000).expect("Failed to get"));
}

#[tokio::test]
async fn test_multi_get() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&123, &"123".to_string())
        .expect("Failed to insert");
    db.insert(&456, &"456".to_string())
        .expect("Failed to insert");

    let result = db.multi_get([123, 456, 789]).expect("Failed to multi get");

    assert_eq!(result.len(), 3);
    assert_eq!(result[0], Some("123".to_string()));
    assert_eq!(result[1], Some("456".to_string()));
    assert_eq!(result[2], None);
}

#[tokio::test]
async fn test_skip() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&123, &"123".to_string())
        .expect("Failed to insert");
    db.insert(&456, &"456".to_string())
        .expect("Failed to insert");
    db.insert(&789, &"789".to_string())
        .expect("Failed to insert");

    // Skip all smaller
    let key_vals: Vec<_> = get_iter_with_bounds(&db, Some(456), None).collect();
    assert_eq!(key_vals.len(), 2);
    assert_eq!(key_vals[0], (456, "456".to_string()));
    assert_eq!(key_vals[1], (789, "789".to_string()));

    // Skip to the end
    assert_eq!(get_iter_with_bounds(&db, Some(999), None).count(), 0);

    // Skip to last
    assert_eq!(
        get_reverse_iter(&db, ..).next(),
        Some(Ok((789, "789".to_string()))),
    );

    // Skip to successor of first value
    assert_eq!(get_iter_with_bounds(&db, Some(000), None).count(), 3);
    assert_eq!(get_iter_with_bounds(&db, Some(000), None).count(), 3);
}

#[tokio::test]
async fn test_reverse_iter_with_bounds() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);
    db.insert(&123, &"123".to_string())
        .expect("Failed to insert");
    db.insert(&456, &"456".to_string())
        .expect("Failed to insert");
    db.insert(&789, &"789".to_string())
        .expect("Failed to insert");

    let mut iter = get_reverse_iter(&db, ..=999);
    assert_eq!(iter.next().unwrap(), Ok((789, "789".to_string())));

    db.insert(&999, &"999".to_string())
        .expect("Failed to insert");
    let mut iter = get_reverse_iter(&db, ..=999);
    assert_eq!(iter.next().unwrap(), Ok((999, "999".to_string())));

    let mut iter = get_reverse_iter(&db, ..);
    assert_eq!(iter.next().unwrap(), Ok((999, "999".to_string())));
}

#[tokio::test]
async fn test_remove() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&123456789, &"123456789".to_string())
        .expect("Failed to insert");
    assert!(db.get(&123456789).expect("Failed to get").is_some());

    db.remove(&123456789).expect("Failed to remove");
    assert!(db.get(&123456789).expect("Failed to get").is_none());
}

#[tokio::test]
async fn test_iter() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);
    db.insert(&123456789, &"123456789".to_string())
        .expect("Failed to insert");
    db.insert(&987654321, &"987654321".to_string())
        .expect("Failed to insert");

    let mut iter = get_iter(&db);

    assert_eq!(Some((123456789, "123456789".to_string())), iter.next());
    assert_eq!(Some((987654321, "987654321".to_string())), iter.next());
    assert_eq!(None, iter.next());
}

#[tokio::test]
async fn test_iter_reverse() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    db.insert(&1, &"1".to_string()).expect("Failed to insert");
    db.insert(&2, &"2".to_string()).expect("Failed to insert");
    db.insert(&3, &"3".to_string()).expect("Failed to insert");

    let mut iter = get_reverse_iter(&db, ..);
    assert_eq!(Some(Ok((3, "3".to_string()))), iter.next());
    assert_eq!(Some(Ok((2, "2".to_string()))), iter.next());
    assert_eq!(Some(Ok((1, "1".to_string()))), iter.next());
    assert_eq!(None, iter.next());

    let mut iter = get_iter_with_bounds(&db, Some(1), None);
    assert_eq!(Some((1, "1".to_string())), iter.next());
    assert_eq!(Some((2, "2".to_string())), iter.next());
}

#[tokio::test]
async fn test_insert_batch() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);
    let keys_vals = (1..100).map(|i| (i, i.to_string()));
    let mut insert_batch = db.batch();
    insert_batch
        .insert_batch(&db, keys_vals.clone())
        .expect("Failed to batch insert");
    insert_batch.write().expect("Failed to execute batch");
    for (k, v) in keys_vals {
        let val = db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }
}

#[tokio::test]
async fn test_insert_batch_across_cf() {
    let tmp_dir = iota_common::tempdir();
    let rocks = open_rocksdb(tmp_dir.path(), &["First_CF", "Second_CF"]);

    let db_cf_1 = DBMap::reopen(
        &rocks,
        Some("First_CF"),
        &ReadWriteOptions::default(),
        false,
    )
    .expect("Failed to open storage");
    let keys_vals_1 = (1..100).map(|i| (i, i.to_string()));

    let db_cf_2 = DBMap::reopen(
        &rocks,
        Some("Second_CF"),
        &ReadWriteOptions::default(),
        false,
    )
    .expect("Failed to open storage");
    let keys_vals_2 = (1000..1100).map(|i| (i, i.to_string()));

    let mut batch = db_cf_1.batch();
    batch
        .insert_batch(&db_cf_1, keys_vals_1.clone())
        .expect("Failed to batch insert")
        .insert_batch(&db_cf_2, keys_vals_2.clone())
        .expect("Failed to batch insert");

    batch.write().expect("Failed to execute batch");
    for (k, v) in keys_vals_1 {
        let val = db_cf_1.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }

    for (k, v) in keys_vals_2 {
        let val = db_cf_2.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }
}

#[tokio::test]
async fn test_insert_batch_across_different_db() {
    let tmp_dir1 = iota_common::tempdir();
    let rocks = open_rocksdb(tmp_dir1.path(), &["First_CF", "Second_CF"]);
    let tmp_dir2 = iota_common::tempdir();
    let rocks2 = open_rocksdb(tmp_dir2.path(), &["First_CF", "Second_CF"]);

    let db_cf_1: DBMap<i32, String> = DBMap::reopen(
        &rocks,
        Some("First_CF"),
        &ReadWriteOptions::default(),
        false,
    )
    .expect("Failed to open storage");
    let keys_vals_1 = (1..100).map(|i| (i, i.to_string()));

    let db_cf_2: DBMap<i32, String> = DBMap::reopen(
        &rocks2,
        Some("Second_CF"),
        &ReadWriteOptions::default(),
        false,
    )
    .expect("Failed to open storage");
    let keys_vals_2 = (1000..1100).map(|i| (i, i.to_string()));

    assert!(
        db_cf_1
            .batch()
            .insert_batch(&db_cf_1, keys_vals_1)
            .expect("Failed to batch insert")
            .insert_batch(&db_cf_2, keys_vals_2)
            .is_err()
    );
}

#[tokio::test]
async fn test_delete_batch() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map::<_, u32, String>(tmp_dir.path(), None);

    let keys_vals = (1..100).map(|i| (i, i.to_string()));
    let mut batch = db.batch();
    batch
        .insert_batch(&db, keys_vals)
        .expect("Failed to batch insert");

    // delete the odd-index keys
    let deletion_keys = (1..100).step_by(2);
    batch
        .delete_batch(&db, deletion_keys)
        .expect("Failed to batch delete");

    batch.write().expect("Failed to execute batch");

    for (k, _) in get_iter(&db) {
        assert_eq!(k % 2, 0);
    }
}

#[tokio::test]
async fn test_delete_range() {
    let tmp_dir = iota_common::tempdir();
    let options = ReadWriteOptions::default();
    let db: DBMap<i32, String> = DBMap::reopen(
        &open_rocksdb(tmp_dir.path(), &[rocksdb::DEFAULT_COLUMN_FAMILY_NAME]),
        None,
        &options,
        false,
    )
    .unwrap();

    // Note that the last element is (100, "100".to_owned()) here
    let keys_vals = (0..101).map(|i| (i, i.to_string()));
    let mut batch = db.batch();
    batch
        .insert_batch(&db, keys_vals)
        .expect("Failed to batch insert");

    batch
        .schedule_delete_range(&db, &50, &100)
        .expect("Failed to delete range");

    batch.write().expect("Failed to execute batch");

    for k in 0..50 {
        assert!(db.contains_key(&k).expect("Failed to query legal key"),);
    }
    for k in 50..100 {
        assert!(!db.contains_key(&k).expect("Failed to query legal key"));
    }

    // range operator is not inclusive of to
    assert!(db.contains_key(&100).expect("Failed to query legal key"));
}

mod delete_all {
    use super::*;

    #[tokio::test]
    async fn deletes_every_entry() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<i32, String> = open_map(tmp_dir.path(), None);

        let mut batch = db.batch();
        batch
            .insert_batch(&db, (0..101).map(|i| (i, i.to_string())))
            .expect("Failed to batch insert");
        batch.write().expect("Failed to execute batch");

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
        assert_eq!(get_iter(&db).count(), 0);

        // The tombstone must not shadow entries written after it, including one
        // at the key it ended on.
        db.insert(&100, &"100".to_owned())
            .expect("Failed to insert");
        assert_eq!(
            vec![(100, "100".to_owned())],
            get_iter(&db).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn deletes_a_single_entry() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<i32, String> = open_map(tmp_dir.path(), None);

        db.insert(&1, &"1".to_owned()).expect("Failed to insert");

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn deletes_an_all_ones_last_key() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<u64, String> = open_map(tmp_dir.path(), None);

        // The greatest key serializes to all ones, so no key of the same length
        // sorts above it.
        for k in [0, 1, u64::MAX] {
            db.insert(&k, &k.to_string()).expect("Failed to insert");
        }

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn deletes_an_empty_key() {
        let tmp_dir = iota_common::tempdir();
        // A unit key serializes to no bytes at all, so the last key is the
        // empty one.
        let db: DBMap<(), String> = open_map(tmp_dir.path(), None);

        db.insert(&(), &"value".to_owned())
            .expect("Failed to insert");

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn deletes_signed_keys() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<i32, String> = open_map(tmp_dir.path(), None);

        // Negative keys serialize above the positive ones, so the last key in
        // the table is -1 rather than 7.
        for k in [-5, -1, 0, 7] {
            db.insert(&k, &k.to_string()).expect("Failed to insert");
        }

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn deletes_variable_length_keys() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<String, String> = open_map(tmp_dir.path(), None);

        for k in ["a", "bb", "ccc"] {
            db.insert(&k.to_owned(), &k.to_uppercase())
                .expect("Failed to insert");
        }

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn deletes_entries_already_on_disk() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<i32, String> = open_map(tmp_dir.path(), None);

        for i in 0..10 {
            db.insert(&i, &i.to_string()).expect("Failed to insert");
        }
        // Move the entries out of the memtable, so the tombstone has to cover
        // entries already written to SST files.
        db.flush().expect("Failed to flush");

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn succeeds_on_an_empty_map() {
        let tmp_dir = iota_common::tempdir();
        let db: DBMap<i32, String> = open_map(tmp_dir.path(), None);

        db.schedule_delete_all().expect("Failed to delete all");

        assert!(db.is_empty());
    }

    #[tokio::test]
    async fn leaves_other_column_families_intact() {
        let tmp_dir = iota_common::tempdir();
        let rocks = open_rocksdb(tmp_dir.path(), &["First_CF", "Second_CF"]);
        let options = ReadWriteOptions::default();
        let db_cf_1: DBMap<i32, String> = DBMap::reopen(&rocks, Some("First_CF"), &options, false)
            .expect("Failed to open storage");
        let db_cf_2: DBMap<i32, String> = DBMap::reopen(&rocks, Some("Second_CF"), &options, false)
            .expect("Failed to open storage");

        let keys_vals_2 = (0..10).map(|i| (i, i.to_string())).collect::<Vec<_>>();
        let mut batch = db_cf_1.batch();
        batch
            .insert_batch(&db_cf_1, (0..10).map(|i| (i, i.to_string())))
            .expect("Failed to batch insert")
            .insert_batch(&db_cf_2, keys_vals_2.clone())
            .expect("Failed to batch insert");
        batch.write().expect("Failed to execute batch");

        db_cf_1.schedule_delete_all().expect("Failed to delete all");

        assert!(db_cf_1.is_empty());
        assert_eq!(keys_vals_2, get_iter(&db_cf_2).collect::<Vec<_>>());
    }
}

#[tokio::test]
async fn test_delete_range_across_databases_is_rejected() {
    let first_dir = iota_common::tempdir();
    let second_dir = iota_common::tempdir();
    let first_db: DBMap<i32, String> = open_map(first_dir.path(), None);
    let second_db: DBMap<i32, String> = open_map(second_dir.path(), None);

    let mut batch = first_db.batch();
    let err = batch
        .schedule_delete_range(&second_db, &0, &10)
        .expect_err("Deleting a range in another database should be rejected");

    assert!(matches!(err, TypedStoreError::CrossDBBatch));
}

#[tokio::test]
async fn test_iter_with_bounds() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    // Add [1, 50) and (50, 100) in the db
    for i in 1..100 {
        if i != 50 {
            db.insert(&i, &i.to_string()).unwrap();
        }
    }

    // Tests basic bounded scan.
    let db_iter = get_iter_with_bounds(&db, Some(20), Some(90));
    assert_eq!(
        (20..50)
            .chain(51..90)
            .map(|i| (i, i.to_string()))
            .collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Don't specify upper bound.
    let db_iter = get_iter_with_bounds(&db, Some(20), None);
    assert_eq!(
        (20..50)
            .chain(51..100)
            .map(|i| (i, i.to_string()))
            .collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Don't specify lower bound.
    let db_iter = get_iter_with_bounds(&db, None, Some(90));
    assert_eq!(
        (1..50)
            .chain(51..90)
            .map(|i| (i, i.to_string()))
            .collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Don't specify any bounds.
    let db_iter = get_iter_with_bounds(&db, None, None);
    assert_eq!(
        (1..50)
            .chain(51..100)
            .map(|i| (i, i.to_string()))
            .collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Specify a bound outside of dataset.
    let db_iter = db.safe_iter_with_bounds(Some(200), Some(300));
    assert!(db_iter.collect::<Vec<_>>().is_empty());

    // Skip to first key in the bound (bound is [1, 50))
    let db_iter = get_iter_with_bounds(&db, Some(1), Some(50));
    assert_eq!(
        (1..50).map(|i| (i, i.to_string())).collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );
}

#[rstest]
#[tokio::test]
async fn test_range_iter() {
    let tmp_dir = iota_common::tempdir();
    let db = open_map(tmp_dir.path(), None);

    // Add [1, 50) and (50, 100) in the db
    for i in 1..100 {
        if i != 50 {
            db.insert(&i, &i.to_string()).unwrap();
        }
    }

    // Tests basic range iterating with inclusive end.
    let db_iter = get_range_iter(&db, 10..=20);
    assert_eq!(
        (10..21).map(|i| (i, i.to_string())).collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Tests range with min start and exclusive end.
    let db_iter = get_range_iter(&db, ..20);
    assert_eq!(
        (1..20).map(|i| (i, i.to_string())).collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Tests range with max end.
    let db_iter = get_range_iter(&db, 60..);
    assert_eq!(
        (60..100).map(|i| (i, i.to_string())).collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );

    // Skip to first key in the bound (bound is [1, 49))
    let db_iter = get_range_iter(&db, 1..49);
    assert_eq!(
        (1..49).map(|i| (i, i.to_string())).collect::<Vec<_>>(),
        db_iter.collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn test_safe_range_iter_inclusive_upper_bound_at_max() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u8, String> = open_map(tmp_dir.path(), None);
    db.insert(&100u8, &"100".to_string()).unwrap();
    db.insert(&u8::MAX, &"max".to_string()).unwrap();

    let results: Vec<_> = get_range_iter(&db, 100u8..=u8::MAX).collect();
    assert_eq!(
        vec![(100u8, "100".to_string()), (u8::MAX, "max".to_string())],
        results,
        "Range ..=u8::MAX must include the entry at u8::MAX",
    );
}

#[tokio::test]
async fn test_reversed_safe_iter_inclusive_upper_bound_at_max() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u8, String> = open_map(tmp_dir.path(), None);
    db.insert(&100u8, &"100".to_string()).unwrap();
    db.insert(&u8::MAX, &"max".to_string()).unwrap();

    let results: Vec<_> = get_reverse_iter(&db, ..=u8::MAX)
        .map(|item| item.unwrap())
        .collect();
    assert_eq!(
        vec![(u8::MAX, "max".to_string()), (100u8, "100".to_string())],
        results,
        "safe_range_iter_reversed with ..=u8::MAX must include the entry at u8::MAX",
    );
}

#[tokio::test]
async fn test_safe_range_iter_exclusive_lower_bound_at_max() {
    use std::ops::Bound;

    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u8, String> = open_map(tmp_dir.path(), None);
    db.insert(&100u8, &"100".to_string()).unwrap();
    db.insert(&u8::MAX, &"max".to_string()).unwrap();

    let results: Vec<_> =
        get_range_iter(&db, (Bound::Excluded(u8::MAX), Bound::Unbounded)).collect();
    assert_eq!(
        Vec::<(u8, String)>::new(),
        results,
        "Range (Excluded(u8::MAX), Unbounded) must yield no entries -- there is no key strictly greater than u8::MAX",
    );
}

#[tokio::test]
async fn test_safe_range_iter_exclusive_lower_bound_at_max_with_inclusive_upper() {
    use std::ops::Bound;

    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u8, String> = open_map(tmp_dir.path(), None);
    db.insert(&100u8, &"100".to_string()).unwrap();
    db.insert(&u8::MAX, &"max".to_string()).unwrap();

    let results: Vec<_> =
        get_range_iter(&db, (Bound::Excluded(u8::MAX), Bound::Included(u8::MAX))).collect();
    assert_eq!(
        Vec::<(u8, String)>::new(),
        results,
        "Range (Excluded(u8::MAX), Included(u8::MAX)) must yield no entries -- the lower bound excludes the only candidate",
    );
}

#[tokio::test]
async fn test_safe_range_iter_reversed_matches_forward() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u32, String> = open_map(tmp_dir.path(), None);
    fill_for_range_symmetry(&db);
    assert_reversed_matches_forward(&db);
}

/// A tagged map owes the same symmetry, and owes it with the neighbouring tags
/// populated across the whole key space: its bounds are computed by a different
/// function from the plain map's, and it is the reverse scan that carries its
/// upper bound in the seek rather than in the read options.
#[tokio::test]
async fn tagged_reversed_scan_matches_forward() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared"]);
    let open = |tag: u8| {
        TaggedDBMap::<u32, String>::reopen(&db, "shared", tag, &ReadWriteOptions::default(), false)
            .expect("failed to open the map")
    };
    let below = open(0);
    let map = open(1);
    let above = open(2);

    for neighbour in [&below, &above] {
        neighbour
            .multi_insert([(0, "noise".to_string()), (u32::MAX, "noise".to_string())])
            .unwrap();
    }
    fill_for_range_symmetry(&map);

    assert_reversed_matches_forward(&map);
}

type RangeOfKeys = (std::ops::Bound<u32>, std::ops::Bound<u32>);

/// Fills a map with `[1, 100)`, so that bounds like 20 and 50 land on existing
/// keys and exercise the "skip the boundary key" path of the reverse seek.
fn fill_for_range_symmetry<'a, M: Map<'a, u32, String>>(map: &M)
where
    M::Error: std::fmt::Debug,
{
    for i in 1..100u32 {
        map.insert(&i, &i.to_string()).unwrap();
    }
}

/// A reverse scan yields exactly what the forward one does, reversed, for every
/// shape a range can take. Written for any map, so that a tagged map is held to
/// what a plain one does even though it computes its bounds elsewhere.
fn assert_reversed_matches_forward<'a, M: Map<'a, u32, String>>(map: &'a M) {
    use std::ops::Bound::{Excluded, Included, Unbounded};

    let check = |range: RangeOfKeys| {
        let forward: Vec<_> = map.safe_range_iter(range).map(|r| r.unwrap()).collect();
        let mut reversed: Vec<_> = map
            .safe_range_iter_reversed(range)
            .map(|r| r.unwrap())
            .collect();
        reversed.reverse();
        assert_eq!(
            forward, reversed,
            "reversed(range) must equal forward(range) reversed; range = {range:?}",
        );
    };

    check((Included(10), Excluded(20))); // 10..20, boundary key 20 exists
    check((Included(10), Included(20))); // 10..=20
    check((Excluded(10), Excluded(20))); // (10, 20)
    check((Unbounded, Excluded(20))); // ..20
    check((Unbounded, Included(20))); // ..=20
    check((Included(50), Unbounded)); // 50..
    check((Excluded(50), Unbounded)); // (50, ..)
    check((Unbounded, Unbounded)); // full table
    check((Included(40), Excluded(40))); // empty
    check((Included(50), Included(50))); // single element
    check((Included(0), Included(u32::MAX))); // the whole key space
    check((Excluded(0), Excluded(u32::MAX)));
}

#[tokio::test]
async fn test_safe_iter_with_prefix() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<(u64, u64), String> = open_map(tmp_dir.path(), None);
    for (a, b) in [(1u64, 1u64), (1, 2), (1, 3), (2, 1), (2, 2)] {
        db.insert(&(a, b), &format!("{a}-{b}")).unwrap();
    }

    let forward: Vec<_> = db
        .safe_iter_with_prefix(&1u64)
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        vec![
            ((1, 1), "1-1".to_string()),
            ((1, 2), "1-2".to_string()),
            ((1, 3), "1-3".to_string()),
        ],
        forward,
        "prefix scan must yield exactly the entries under the prefix",
    );

    let reversed: Vec<_> = db
        .safe_iter_with_prefix_reversed(&1u64)
        .map(|r| r.unwrap())
        .collect();
    let mut expected = forward;
    expected.reverse();
    assert_eq!(
        expected, reversed,
        "reversed prefix scan must be forward reversed"
    );

    // The neighbouring prefix must not leak in.
    let other: Vec<_> = db
        .safe_iter_with_prefix(&2u64)
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        vec![((2, 1), "2-1".to_string()), ((2, 2), "2-2".to_string())],
        other,
    );
}

#[tokio::test]
async fn test_safe_iter_with_prefix_at_max() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<(u8, u8), String> = open_map(tmp_dir.path(), None);
    for (a, b) in [(254u8, 9u8), (255, 1), (255, 2)] {
        db.insert(&(a, b), &format!("{a}-{b}")).unwrap();
    }

    // Prefix 0xFF has no representable upper bound; the scan runs to the end of
    // the column family and must still exclude the lower neighbour.
    let got: Vec<_> = db
        .safe_iter_with_prefix(&u8::MAX)
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        vec![
            ((255, 1), "255-1".to_string()),
            ((255, 2), "255-2".to_string()),
        ],
        got,
    );
}

#[tokio::test]
async fn test_safe_iter_with_prefix_multi_field() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<(u64, u64, u64), String> = open_map(tmp_dir.path(), None);
    for (a, b, c) in [
        (1u64, 1u64, 9u64), // different second field
        (1, 2, 0),
        (1, 2, 7),
        (1, 2, u64::MAX), // boundary at the third field's max
        (1, 3, 0),        // next second field
        (2, 2, 0),        // different first field
    ] {
        db.insert(&(a, b, c), &format!("{a}-{b}-{c}")).unwrap();
    }

    // A two-field prefix must select exactly the keys whose first two fields
    // match, equivalent to the closed range over the whole third-field space.
    let by_prefix: Vec<_> = db
        .safe_iter_with_prefix(&(1u64, 2u64))
        .map(|r| r.unwrap())
        .collect();
    let by_range: Vec<_> = db
        .safe_range_iter((1u64, 2u64, u64::MIN)..=(1u64, 2u64, u64::MAX))
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(by_prefix, by_range);
    assert_eq!(
        vec![(1, 2, 0), (1, 2, 7), (1, 2, u64::MAX)],
        by_prefix.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn test_safe_iter_surfaces_deserialization_error() {
    // A bad VALUE: store <u32, u8>, reopen with a wider value type. The stored
    // 1-byte values cannot deserialize into (u64, u64). Forward and reverse
    // iteration share `SafeIter::next`, so both must surface the error rather
    // than silently ending the scan.
    {
        let tmp_dir = iota_common::tempdir();
        let arc = {
            let db = open_map::<_, u32, u8>(tmp_dir.path(), None);
            db.insert(&1, &7).unwrap();
            db.insert(&2, &8).unwrap();
            db
        };
        let db =
            DBMap::<u32, (u64, u64)>::reopen(&arc.db, None, &ReadWriteOptions::default(), false)
                .expect("failed to reopen");

        let forward = db.safe_iter().next();
        assert!(
            matches!(forward, Some(Err(TypedStoreError::Serialization(_)))),
            "forward iteration must yield Some(Err(..)) on a bad value; got {forward:?}",
        );
        let reverse = db.safe_range_iter_reversed(..).next();
        assert!(
            matches!(reverse, Some(Err(TypedStoreError::Serialization(_)))),
            "reverse iteration must yield Some(Err(..)) on a bad value; got {reverse:?}",
        );
    }

    // A bad KEY: store <u8, u32>, reopen with a wider key type so the key itself
    // fails to deserialize (exercises the other error arm of `SafeIter::next`).
    {
        let tmp_dir = iota_common::tempdir();
        let arc = {
            let db = open_map::<_, u8, u32>(tmp_dir.path(), None);
            db.insert(&1, &7).unwrap();
            db
        };
        let db =
            DBMap::<(u64, u64), u32>::reopen(&arc.db, None, &ReadWriteOptions::default(), false)
                .expect("failed to reopen");

        let first = db.safe_iter().next();
        assert!(
            matches!(first, Some(Err(TypedStoreError::Serialization(_)))),
            "a key that fails to deserialize must yield Some(Err(..)); got {first:?}",
        );
    }
}

#[tokio::test]
async fn test_prefix_from_matches_old_bounded_scan() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<(u64, u64), String> = open_map(tmp_dir.path(), None);
    for (a, b) in [(1u64, 10u64), (1, 20), (1, 30), (2, 5)] {
        db.insert(&(a, b), &format!("{a}-{b}")).unwrap();
    }

    // Old approach: explicit bounds [(1, cursor), (1, u64::MAX)) -- upper
    // EXCLUSIVE.
    let old = |cursor: Option<u64>| -> Vec<(u64, u64)> {
        db.safe_iter_with_bounds(Some((1u64, cursor.unwrap_or(0))), Some((1u64, u64::MAX)))
            .skip(usize::from(cursor.is_some()))
            .map(|r| r.unwrap().0)
            .collect()
    };
    // New approach: prefix-from with the same cursor as the key remainder.
    let new = |cursor: Option<u64>| -> Vec<(u64, u64)> {
        db.safe_iter_with_prefix_from(&1u64, Bound::Included(&cursor.unwrap_or(0)))
            .skip(usize::from(cursor.is_some()))
            .map(|r| r.unwrap().0)
            .collect()
    };

    // For all realistic data the prefix scan is equivalent to the old bounds.
    for cursor in [None, Some(10u64), Some(20), Some(30)] {
        assert_eq!(old(cursor), new(cursor), "cursor {cursor:?}");
    }

    // The only divergence: the old exclusive `(1, u64::MAX)` upper bound dropped
    // an entry sitting exactly at that tail; the prefix scan correctly keeps it.
    db.insert(&(1u64, u64::MAX), &"max".to_string()).unwrap();
    assert!(!old(None).contains(&(1, u64::MAX)));
    assert!(new(None).contains(&(1, u64::MAX)));
}

#[tokio::test]
async fn test_safe_iter_with_prefix_from() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<(u64, u64), String> = open_map(tmp_dir.path(), None);
    for (a, b) in [(1u64, 1u64), (1, 2), (1, 3), (1, u64::MAX), (2, 1)] {
        db.insert(&(a, b), &format!("{a}-{b}")).unwrap();
    }
    let keys = |lower: Bound<&u64>| -> Vec<(u64, u64)> {
        db.safe_iter_with_prefix_from(&1u64, lower)
            .map(|r| r.unwrap().0)
            .collect()
    };

    // Resume the prefix-1 scan from cursor 2 (inclusive); the upper bound is still
    // the end of prefix 1, so (1, 1) and the whole prefix 2 are excluded.
    let got: Vec<_> = db
        .safe_iter_with_prefix_from(&1u64, Bound::Included(&2u64))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        vec![
            ((1, 2), "1-2".to_string()),
            ((1, 3), "1-3".to_string()),
            ((1, u64::MAX), format!("1-{}", u64::MAX)),
        ],
        got,
    );

    // An exclusive bound resumes strictly after the cursor — also when the
    // cursor's row does not exist.
    assert_eq!(keys(Bound::Excluded(&2)), [(1, 3), (1, u64::MAX)]);
    db.remove(&(1, 2)).unwrap();
    assert_eq!(keys(Bound::Excluded(&2)), [(1, 3), (1, u64::MAX)]);

    // Excluding the maximum remainder (an all-0xFF tail) yields an empty
    // scan rather than the next prefix's keys.
    assert!(keys(Bound::Excluded(&u64::MAX)).is_empty());

    // An unbounded lower bound covers the whole prefix.
    assert_eq!(keys(Bound::Unbounded), [(1, 1), (1, 3), (1, u64::MAX)]);
}

#[tokio::test]
async fn test_safe_range_iter_reversed_inclusive_ranges() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u32, String> = open_map(tmp_dir.path(), None);
    for i in 1..100u32 {
        db.insert(&i, &i.to_string()).unwrap();
    }

    // Closed range: [10, 20] in descending order.
    let got: Vec<_> = db
        .safe_range_iter_reversed(10..=20)
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        (10..=20)
            .rev()
            .map(|i| (i, i.to_string()))
            .collect::<Vec<_>>(),
        got,
    );

    // Upper-only inclusive range: [.., 20] in descending order.
    let got: Vec<_> = db
        .safe_range_iter_reversed(..=20)
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        (1..=20)
            .rev()
            .map(|i| (i, i.to_string()))
            .collect::<Vec<_>>(),
        got,
    );
}

#[tokio::test]
async fn test_is_empty() {
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<i32, String> = open_map(tmp_dir.path(), Some("table"));
    // Test empty map is truly empty
    assert!(db.is_empty());

    let keys_vals = (0..101).map(|i| (i, i.to_string()));
    let mut insert_batch = db.batch();
    insert_batch
        .insert_batch(&db, keys_vals)
        .expect("Failed to batch insert");

    insert_batch.write().expect("Failed to execute batch");

    // Check we have multiple entries and not empty
    assert!(db.safe_iter().count() > 1);
    assert!(!db.is_empty());
}

#[tokio::test]
async fn test_multi_insert() {
    // Init a DB
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<i32, String> = open_map(tmp_dir.path(), Some("table"));
    // Create kv pairs
    let keys_vals = (0..101).map(|i| (i, i.to_string()));

    db.multi_insert(keys_vals.clone())
        .expect("Failed to multi-insert");

    for (k, v) in keys_vals {
        let val = db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }
}

#[tokio::test]
async fn test_checkpoint() {
    let tmp_dir = iota_common::tempdir();
    let path_prefix = tmp_dir.path();
    let db_path = path_prefix.join("db");
    let db: DBMap<i32, String> = open_map(db_path, Some("table"));
    // Create kv pairs
    let keys_vals = (0..101).map(|i| (i, i.to_string()));

    db.multi_insert(keys_vals.clone())
        .expect("Failed to multi-insert");
    let checkpointed_path = path_prefix.join("checkpointed_db");
    db.db
        .checkpoint(&checkpointed_path)
        .expect("Failed to create db checkpoint");
    // Create more kv pairs
    let new_keys_vals = (101..201).map(|i| (i, i.to_string()));
    db.multi_insert(new_keys_vals.clone())
        .expect("Failed to multi-insert");
    // Verify checkpoint
    let checkpointed_db: DBMap<i32, String> = open_map(checkpointed_path, Some("table"));
    // Ensure keys inserted before checkpoint are present in original and
    // checkpointed db
    for (k, v) in keys_vals {
        let val = db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v.clone()), val);
        let val = checkpointed_db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }
    // Ensure keys inserted after checkpoint are only present in original db but not
    // in checkpointed db
    for (k, v) in new_keys_vals {
        let val = db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v.clone()), val);
        let val = checkpointed_db.get(&k).expect("Failed to get inserted key");
        assert_eq!(None, val);
    }
}

#[tokio::test]
async fn test_multi_remove() {
    // Init a DB
    let tmp_dir = iota_common::tempdir();
    let db: DBMap<i32, String> = open_map(tmp_dir.path(), Some("table"));

    // Create kv pairs
    let keys_vals = (0..101).map(|i| (i, i.to_string()));

    db.multi_insert(keys_vals.clone())
        .expect("Failed to multi-insert");

    // Check insertion
    for (k, v) in keys_vals.clone() {
        let val = db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }

    // Remove 50 items
    db.multi_remove(keys_vals.clone().map(|kv| kv.0).take(50))
        .expect("Failed to multi-remove");
    assert_eq!(db.safe_iter().count(), 101 - 50);

    // Check that the remaining are present
    for (k, v) in keys_vals.skip(50) {
        let val = db.get(&k).expect("Failed to get inserted key");
        assert_eq!(Some(v), val);
    }
}

#[tokio::test]
async fn open_as_secondary_test() {
    let tmp_dir = iota_common::tempdir();
    let primary_path = tmp_dir.path();

    // Init a DB
    let primary_db: DBMap<i32, String> = open_map(primary_path, Some("table"));
    // Create kv pairs
    let keys_vals = (0..101).map(|i| (i, i.to_string()));

    primary_db
        .multi_insert(keys_vals.clone())
        .expect("Failed to multi-insert");

    let opt = rocksdb::Options::default();
    let secondary_store = open_cf_opts_secondary(
        primary_path,
        None,
        None,
        MetricConf::default(),
        &[("table", opt)],
    )
    .unwrap();
    let secondary_db = DBMap::<i32, String>::reopen(
        &secondary_store,
        Some("table"),
        &ReadWriteOptions::default(),
        false,
    )
    .unwrap();

    secondary_db.try_catch_up_with_primary().unwrap();
    // Check secondary
    for (k, v) in keys_vals {
        assert_eq!(secondary_db.get(&k).unwrap(), Some(v));
    }

    // Update the value from 0 to 10
    primary_db.insert(&0, &"10".to_string()).unwrap();

    // This should still be stale since secondary is behind
    assert_eq!(secondary_db.get(&0).unwrap(), Some("0".to_string()));

    // Try force catchup
    secondary_db.try_catch_up_with_primary().unwrap();

    // New value should be present
    assert_eq!(secondary_db.get(&0).unwrap(), Some("10".to_string()));
}

#[tokio::test]
async fn a_column_family_can_be_created_at_runtime() {
    let tmp_dir = iota_common::tempdir();
    let path = tmp_dir.path();
    {
        let db = open_rocksdb(path, &["static_cf"]);
        assert!(db.cf_handle("dynamic_cf").is_none());
        db.create_cf("dynamic_cf", &rocksdb::Options::default())
            .expect("Failed to create column family");
        assert!(db.cf_handle("dynamic_cf").is_some());

        // Creating an already-existing column family must fail.
        assert!(
            db.create_cf("dynamic_cf", &rocksdb::Options::default())
                .is_err()
        );

        let map = DBMap::<u32, String>::reopen(
            &db,
            Some("dynamic_cf"),
            &ReadWriteOptions::default(),
            false,
        )
        .expect("Failed to open map on dynamic column family");
        map.insert(&1, &"one".to_string())
            .expect("Failed to insert");
        assert_eq!(map.get(&1).unwrap(), Some("one".to_string()));

        // The dynamically created column family must be covered by flush_all:
        // its memtable reaches disk as an SST file of that column family.
        let sst_files = || {
            db.live_files()
                .expect("Failed to list live files")
                .into_iter()
                .filter(|file| file.column_family_name == "dynamic_cf")
                .count()
        };
        assert_eq!(sst_files(), 0, "the row must still be in the memtable");
        db.flush_all().expect("Failed to flush");
        assert_eq!(sst_files(), 1, "flush_all must cover the column family");
    }
    {
        // The column family is rediscovered when reopening the database from
        // disk without declaring it.
        let db = open_rocksdb(path, &["static_cf"]);
        assert!(db.cf_handle("dynamic_cf").is_some());
        let map = DBMap::<u32, String>::reopen(
            &db,
            Some("dynamic_cf"),
            &ReadWriteOptions::default(),
            false,
        )
        .expect("Failed to open map on rediscovered column family");
        assert_eq!(map.get(&1).unwrap(), Some("one".to_string()));

        db.drop_cf("dynamic_cf")
            .expect("Failed to drop column family");
        assert!(db.cf_handle("dynamic_cf").is_none());
    }
    {
        // A dropped column family stays gone after reopening.
        let db = open_rocksdb(path, &["static_cf"]);
        assert!(db.cf_handle("dynamic_cf").is_none());
    }
}

/// Differently-typed [`TaggedDBMap`]s share one column family without their
/// rows surfacing in each other: gets, full scans in both directions,
/// bounded ranges, and deletes all stay within their map's tag.
#[tokio::test]
async fn tagged_dbmaps_share_a_column_family() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared"]);
    let numbers: TaggedDBMap<u32, String> =
        TaggedDBMap::reopen(&db, "shared", 0, &ReadWriteOptions::default(), false)
            .expect("failed to open the numbers map");
    let words: TaggedDBMap<String, u64> =
        TaggedDBMap::reopen(&db, "shared", 1, &ReadWriteOptions::default(), false)
            .expect("failed to open the words map");

    let mut batch = numbers.batch();
    batch
        .insert_batch_tagged(&numbers, [(1, "one".to_string()), (2, "two".to_string())])
        .unwrap()
        .insert_batch_tagged(&words, [("one".to_string(), 1), ("two".to_string(), 2)])
        .unwrap();
    batch.write().unwrap();

    // The rows are in the named column family, under the tagged keys, and not
    // in the default one the maps would fall back to if the name were ignored.
    let shared = DBMap::<(u8, u32), String>::reopen(
        &db,
        Some("shared"),
        &ReadWriteOptions::default(),
        false,
    )
    .expect("failed to open the shared column family");
    assert_eq!(shared.get(&(0, 1)).unwrap(), Some("one".to_string()));
    assert_eq!(shared.safe_iter().count(), 4);

    // Point lookups stay within the tag.
    assert_eq!(numbers.get(&1).unwrap(), Some("one".to_string()));
    assert_eq!(words.get(&"two".to_string()).unwrap(), Some(2));
    assert_eq!(
        numbers.multi_get([1, 2, 3]).unwrap(),
        vec![Some("one".to_string()), Some("two".to_string()), None]
    );
    assert_eq!(
        numbers.multi_contains_keys([1, 3]).unwrap(),
        vec![true, false]
    );
    assert!(words.contains_key(&"one".to_string()).unwrap());

    // Full scans in both directions yield only the map's own rows.
    let rows: Vec<_> = numbers.safe_iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(rows, vec![(1, "one".to_string()), (2, "two".to_string())]);
    let rows: Vec<_> = numbers
        .safe_range_iter_reversed(..)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(2, "two".to_string()), (1, "one".to_string())]);
    let rows: Vec<_> = words.safe_iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(rows, vec![("one".to_string(), 1), ("two".to_string(), 2)]);

    // Bounded ranges honour their own inclusivity, in both directions.
    let rows: Vec<_> = numbers
        .safe_range_iter(2..=u32::MAX)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(2, "two".to_string())]);
    let rows: Vec<_> = numbers
        .safe_iter_with_bounds(None, Some(2))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(1, "one".to_string())]);
    let rows: Vec<_> = numbers
        .safe_range_iter(1..2)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(1, "one".to_string())]);
    let rows: Vec<_> = numbers
        .safe_range_iter((Bound::Excluded(1), Bound::Included(2)))
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(2, "two".to_string())]);
    let rows: Vec<_> = numbers
        .safe_range_iter_reversed(u32::MIN..=1)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(1, "one".to_string())]);
    let rows: Vec<_> = numbers
        .safe_range_iter_reversed(..2)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(1, "one".to_string())]);

    // Single-key writes stay within the tag as well.
    numbers.insert(&3, &"three".to_string()).unwrap();
    assert_eq!(numbers.get(&3).unwrap(), Some("three".to_string()));
    numbers.remove(&3).unwrap();
    assert_eq!(numbers.get(&3).unwrap(), None);

    // Deletes stay within the tag.
    let mut batch = numbers.batch();
    batch.delete_batch_tagged(&numbers, [1, 2]).unwrap();
    batch.write().unwrap();
    assert!(numbers.is_empty());
    assert!(!words.is_empty());
    assert_eq!(words.get(&"one".to_string()).unwrap(), Some(1));
}

/// Ranges are bounded by the serialized keys, so for a key type whose encoding
/// does not order the way `Ord` does they cover something else. Pinned here
/// because it surprises callers; a plain `DBMap` behaves the same way.
#[tokio::test]
async fn tagged_ranges_follow_the_key_encoding_rather_than_ord() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared"]);
    let words: TaggedDBMap<String, u32> =
        TaggedDBMap::reopen(&db, "shared", 0, &ReadWriteOptions::default(), false)
            .expect("failed to open the words map");
    let signed: TaggedDBMap<i32, u32> =
        TaggedDBMap::reopen(&db, "shared", 1, &ReadWriteOptions::default(), false)
            .expect("failed to open the signed map");

    // A string is serialized with its length first, so the order is by length.
    words
        .multi_insert([
            ("b".to_string(), 0),
            ("aa".to_string(), 0),
            ("aaa".to_string(), 0),
        ])
        .unwrap();
    let keys: Vec<_> = words
        .safe_iter()
        .map(|row| row.unwrap().0)
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        ["b", "aa", "aaa"],
        "Ord would order these differently"
    );
    assert!(
        words
            .safe_range_iter("aa".to_string().."b".to_string())
            .next()
            .is_none(),
        "the upper bound serializes below the lower one"
    );

    // A negative integer is two's complement, so it sorts above the positives.
    signed.multi_insert([(0, 0), (1, 0), (-1, 0)]).unwrap();
    let keys: Vec<_> = signed.safe_iter().map(|row| row.unwrap().0).collect();
    assert_eq!(keys, [0, 1, -1], "Ord would order these differently");
    assert!(signed.safe_range_iter(-1..=1).next().is_none());
}

/// Clearing a map that holds nothing writes nothing, whatever its tag: a
/// tombstone would be read past by every iterator of the column family until
/// compaction drops it.
#[tokio::test]
async fn clearing_an_empty_tagged_map_writes_nothing() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared"]);
    let writes_so_far = || match &db.storage {
        crate::database::Storage::Rocks(rocks) => rocks.underlying.latest_sequence_number(),
        _ => unreachable!("the database is rocksdb"),
    };
    let open = |tag| {
        TaggedDBMap::<u32, String>::reopen(&db, "shared", tag, &ReadWriteOptions::default(), false)
            .expect("failed to open the map")
    };

    // The maximum tag ends its range on the last entry, the others on the tag
    // that follows, so each takes its own path to the same answer.
    for tag in [0, u8::MAX] {
        let map = open(tag);
        let before = writes_so_far();
        map.schedule_delete_all().expect("failed to clear the map");
        assert_eq!(writes_so_far(), before, "tag {tag} wrote over nothing");
    }

    // And a map that does hold rows still writes its tombstone.
    let map = open(0);
    map.insert(&1, &"one".to_string()).unwrap();
    let before = writes_so_far();
    map.schedule_delete_all().expect("failed to clear the map");
    assert!(writes_so_far() > before);
    assert!(map.is_empty());
}

/// A tag belongs to its column family, so the same one is free in another.
#[tokio::test]
async fn a_tag_is_free_in_another_column_family() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared", "other"]);

    let numbers =
        TaggedDBMap::<u32, String>::reopen(&db, "shared", 0, &ReadWriteOptions::default(), false)
            .expect("failed to open the numbers map");
    let words =
        TaggedDBMap::<String, u64>::reopen(&db, "other", 0, &ReadWriteOptions::default(), false)
            .expect("failed to open the words map");

    numbers.insert(&1, &"one".to_string()).unwrap();
    words.insert(&"one".to_string(), &1).unwrap();
    assert_eq!(numbers.get(&1).unwrap(), Some("one".to_string()));
    assert_eq!(words.get(&"one".to_string()).unwrap(), Some(1));
}

/// Reads `key` from the `[CFOptions "cf_name"]` section of the database's
/// current OPTIONS file.
fn cf_option(path: &Path, cf_name: &str, key: &str) -> String {
    let current = std::fs::read_dir(path)
        .expect("Failed to read the database directory")
        .filter_map(|entry| {
            let name = entry.expect("Failed to read a directory entry").file_name();
            name.to_str()?.starts_with("OPTIONS-").then_some(name)
        })
        .max()
        .expect("The database has no OPTIONS file");
    let options = std::fs::read_to_string(path.join(current)).expect("Failed to read the options");

    options
        .split(&format!("[CFOptions \"{cf_name}\"]"))
        .nth(1)
        .expect("The options name no such column family")
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("The column family has no {key} option"))
        .to_owned()
}

/// A column family created at runtime keeps its options when the database is
/// reopened without declaring it, instead of falling back to the rocksdb
/// defaults.
#[tokio::test]
async fn a_rediscovered_column_family_keeps_its_options() {
    let tmp_dir = iota_common::tempdir();
    let path = tmp_dir.path();
    let declared = |cf: &'static str| {
        open_cf_opts(
            path,
            None,
            MetricConf::default(),
            &[(cf, default_db_options().options)],
        )
        .expect("Failed to open rocksdb")
    };
    {
        let db = declared("static_cf");
        db.create_cf("dynamic_cf", &default_db_options().options)
            .expect("Failed to create column family");
    }

    // Reopened without naming the runtime column family, so it is rediscovered.
    let _db = declared("static_cf");
    assert_eq!(
        cf_option(path, "dynamic_cf", "bottommost_compression"),
        cf_option(path, "static_cf", "bottommost_compression"),
    );
}

/// The default column family is opened with the crate's options from the start,
/// rather than with the rocksdb defaults its wrapper would otherwise give it.
#[tokio::test]
async fn the_default_column_family_keeps_the_crate_s_options() {
    let tmp_dir = iota_common::tempdir();
    let path = tmp_dir.path();
    let declared = || {
        open_cf_opts(
            path,
            None,
            MetricConf::default(),
            &[("static_cf", default_db_options().options)],
        )
        .expect("Failed to open rocksdb")
    };

    // The database is new, so nothing is there to rediscover.
    let db = declared();
    let expected = cf_option(path, "static_cf", "bottommost_compression");
    assert_eq!(
        cf_option(
            path,
            rocksdb::DEFAULT_COLUMN_FAMILY_NAME,
            "bottommost_compression"
        ),
        expected,
    );

    // And the options do not change when it is rediscovered on the next open.
    drop(db);
    let _db = declared();
    assert_eq!(
        cf_option(
            path,
            rocksdb::DEFAULT_COLUMN_FAMILY_NAME,
            "bottommost_compression"
        ),
        expected,
    );
}

/// Opening a map on a column family the database does not have is refused,
/// rather than yielding a map whose every operation panics.
#[tokio::test]
async fn opening_a_map_on_a_missing_column_family_fails() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["static_cf"]);

    assert!(matches!(
        DBMap::<u32, String>::reopen(&db, Some("absent"), &ReadWriteOptions::default(), false),
        Err(TypedStoreError::UnregisteredColumn(cf)) if cf == "absent"
    ));
}

/// Dropping the default column family is refused before rocksdb is asked,
/// because its wrapper would lose the handle on the way to being refused.
#[tokio::test]
async fn the_default_column_family_cannot_be_dropped() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["static_cf"]);
    let map = DBMap::<u32, String>::reopen(&db, None, &ReadWriteOptions::default(), false)
        .expect("Failed to open map on the default column family");

    assert!(db.drop_cf(rocksdb::DEFAULT_COLUMN_FAMILY_NAME).is_err());

    // The column family is still there, and still usable.
    assert!(db.cf_handle(rocksdb::DEFAULT_COLUMN_FAMILY_NAME).is_some());
    map.insert(&1, &"one".to_string())
        .expect("Failed to insert");
    assert_eq!(map.get(&1).unwrap(), Some("one".to_string()));
}

/// A column family can be dropped and created again on one database handle,
/// and comes back empty.
#[tokio::test]
async fn a_dropped_column_family_can_be_created_again() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["static_cf"]);
    let options = rocksdb::Options::default();
    db.create_cf("cycled", &options)
        .expect("Failed to create column family");
    {
        let map =
            DBMap::<u32, String>::reopen(&db, Some("cycled"), &ReadWriteOptions::default(), false)
                .expect("Failed to open map");
        map.insert(&1, &"one".to_string())
            .expect("Failed to insert");
    }

    db.drop_cf("cycled").expect("Failed to drop column family");
    assert!(
        DBMap::<u32, String>::reopen(&db, Some("cycled"), &ReadWriteOptions::default(), false)
            .is_err()
    );

    db.create_cf("cycled", &options)
        .expect("Failed to create column family again");
    let map =
        DBMap::<u32, String>::reopen(&db, Some("cycled"), &ReadWriteOptions::default(), false)
            .expect("Failed to open map on the recreated column family");
    assert_eq!(map.get(&1).unwrap(), None);
}

/// `flush_all` covers the `default` column family, which rocksdb opens for
/// every database and which no caller declares.
#[tokio::test]
async fn flush_all_covers_the_default_column_family() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["static_cf"]);
    let map = DBMap::<u32, String>::reopen(&db, None, &ReadWriteOptions::default(), false)
        .expect("Failed to open map on the default column family");
    map.insert(&1, &"one".to_string())
        .expect("Failed to insert");

    let sst_files = || {
        db.live_files()
            .expect("Failed to list live files")
            .into_iter()
            .filter(|file| file.column_family_name == "default")
            .count()
    };
    assert_eq!(sst_files(), 0, "the row must still be in the memtable");
    db.flush_all().expect("Failed to flush");
    assert_eq!(
        sst_files(),
        1,
        "flush_all must cover the default column family"
    );
}

/// A tagged batch write refuses a map from another database, as an untagged
/// one does.
#[tokio::test]
async fn tagged_batch_rejects_a_map_from_another_database() {
    let tmp_dirs = [iota_common::tempdir(), iota_common::tempdir()];
    let dbs = tmp_dirs.each_ref().map(|dir| {
        let db = open_rocksdb(dir.path(), &["shared"]);
        TaggedDBMap::<u32, String>::reopen(&db, "shared", 0, &ReadWriteOptions::default(), false)
            .expect("failed to open the map")
    });
    let [map, other] = &dbs;

    let mut batch = map.batch();
    assert!(matches!(
        batch.insert_batch_tagged(other, [(1, "one".to_string())]),
        Err(TypedStoreError::CrossDBBatch)
    ));
    assert!(matches!(
        batch.delete_batch_tagged(other, [1]),
        Err(TypedStoreError::CrossDBBatch)
    ));
}

/// A bounded range stays within its own tag even when the upper bound is the
/// maximum key, where making the exclusive rocksdb bound would otherwise carry
/// into the next tag's key range — while still yielding that maximum key.
#[tokio::test]
async fn tagged_range_iter_stays_within_its_tag() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared"]);
    // The neighbouring tag's key must serialize to fewer, all-zero bytes: the
    // escaped bound is tag 1 followed by zeros, and only a shorter all-zero key
    // sorts below it.
    let wide: TaggedDBMap<u64, String> =
        TaggedDBMap::reopen(&db, "shared", 0, &ReadWriteOptions::default(), false)
            .expect("failed to open the wide map");
    let narrow: TaggedDBMap<u8, String> =
        TaggedDBMap::reopen(&db, "shared", 1, &ReadWriteOptions::default(), false)
            .expect("failed to open the narrow map");

    let mut batch = wide.batch();
    batch
        .insert_batch_tagged(
            &wide,
            [(1, "one".to_string()), (u64::MAX, "max".to_string())],
        )
        .unwrap()
        .insert_batch_tagged(&narrow, [(0, "zero".to_string())])
        .unwrap();
    batch.write().unwrap();

    let rows: Vec<_> = wide
        .safe_range_iter(0..=u64::MAX)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![(1, "one".to_string()), (u64::MAX, "max".to_string())]
    );
    let rows: Vec<_> = wide
        .safe_range_iter_reversed(0..=u64::MAX)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![(u64::MAX, "max".to_string()), (1, "one".to_string())]
    );

    // The neighbouring tag keeps its row and reaches it through its own range.
    let rows: Vec<_> = narrow
        .safe_range_iter(0..=u8::MAX)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![(0, "zero".to_string())]);
}

/// Clearing a tagged map removes all of its own entries and none of the
/// entries the other tags keep in the same column family.
#[tokio::test]
async fn tagged_schedule_delete_all_clears_only_its_own_tag() {
    let tmp_dir = iota_common::tempdir();
    let db = open_rocksdb(tmp_dir.path(), &["shared"]);
    let open = |tag: u8| {
        TaggedDBMap::<u32, String>::reopen(&db, "shared", tag, &ReadWriteOptions::default(), false)
            .expect("failed to open the map")
    };
    // The cleared tag has an immediate neighbour on either side, and the
    // maximum tag has no following tag to bound its own range against.
    let below = open(0);
    let cleared = open(1);
    let above = open(2);
    let last = open(u8::MAX);

    let mut batch = cleared.batch();
    for map in [&below, &cleared, &above, &last] {
        batch
            .insert_batch_tagged(
                map,
                [(0, "zero".to_string()), (u32::MAX, "max".to_string())],
            )
            .unwrap();
    }
    batch.write().unwrap();

    let entries = |map: &TaggedDBMap<u32, String>| -> Vec<(u32, String)> {
        map.safe_iter().collect::<Result<_, _>>().unwrap()
    };
    let expected = vec![(0, "zero".to_string()), (u32::MAX, "max".to_string())];

    cleared.schedule_delete_all().unwrap();
    assert!(entries(&cleared).is_empty());
    assert_eq!(entries(&below), expected);
    assert_eq!(entries(&above), expected);
    assert_eq!(entries(&last), expected);

    // The maximum tag takes the bound from its own last key.
    last.schedule_delete_all().unwrap();
    assert!(entries(&last).is_empty());
    assert_eq!(entries(&below), expected);
    assert_eq!(entries(&above), expected);

    // Clearing an already empty map is a no-op, at the maximum tag too.
    cleared.schedule_delete_all().unwrap();
    last.schedule_delete_all().unwrap();
    assert_eq!(entries(&below), expected);
}

fn open_map<P: AsRef<Path>, K, V>(path: P, opt_cf: Option<&str>) -> DBMap<K, V> {
    let cf_key = opt_cf.unwrap_or(rocksdb::DEFAULT_COLUMN_FAMILY_NAME);
    DBMap::<K, V>::reopen(
        &open_rocksdb(path, &[cf_key]),
        opt_cf,
        &ReadWriteOptions::default(),
        false,
    )
    .expect("failed to open rocksdb")
}

fn open_rocksdb<P: AsRef<Path>>(path: P, opt_cfs: &[&str]) -> Arc<Database> {
    let opts = rocksdb::Options::default();
    open_cf_opts(
        path,
        None,
        MetricConf::default(),
        &opt_cfs
            .iter()
            .map(|cf| (*cf, opts.clone()))
            .collect::<Vec<_>>(),
    )
    .expect("failed to open rocksdb")
}
