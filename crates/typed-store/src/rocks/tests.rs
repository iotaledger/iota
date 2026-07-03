// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::RangeBounds;

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
    use std::ops::Bound::{self, Excluded, Included, Unbounded};

    let tmp_dir = iota_common::tempdir();
    let db: DBMap<u32, String> = open_map(tmp_dir.path(), None);
    // [1, 100) so that bounds like 20 / 50 land on existing keys, exercising the
    // exclusive-upper "skip the boundary key" path of the reverse seek.
    for i in 1..100u32 {
        db.insert(&i, &i.to_string()).unwrap();
    }

    let check = |range: (Bound<u32>, Bound<u32>)| {
        let forward: Vec<_> = db.safe_range_iter(range).map(|r| r.unwrap()).collect();
        let mut reversed: Vec<_> = db
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
        db.safe_iter_with_prefix_from(&1u64, &cursor.unwrap_or(0))
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
    for (a, b) in [(1u64, 1u64), (1, 2), (1, 3), (2, 1)] {
        db.insert(&(a, b), &format!("{a}-{b}")).unwrap();
    }

    // Resume the prefix-1 scan from cursor 2 (inclusive); the upper bound is still
    // the end of prefix 1, so (1, 1) and the whole prefix 2 are excluded.
    let got: Vec<_> = db
        .safe_iter_with_prefix_from(&1u64, &2u64)
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        vec![((1, 2), "1-2".to_string()), ((1, 3), "1-3".to_string())],
        got,
    );
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
async fn test_create_cf_at_runtime() {
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

        // The dynamically created column family must be covered by flush_all.
        db.flush_all().expect("Failed to flush");
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
