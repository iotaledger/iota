// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use std::{borrow::Borrow, collections::HashSet, fmt::Debug, sync::Mutex, time::Duration};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use typed_store::{
    DBMapUtils, be_fix_int_ser,
    metrics::SamplingInterval,
    rocks::{DBMap, MetricConf},
    traits::Map,
};

fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("Failed to open temporary directory")
        .keep()
}
/// This struct is used to illustrate how the utility works
#[derive(DBMapUtils)]
struct Tables {
    table1: DBMap<String, String>,
    table2: DBMap<i32, String>,
}
// Check that generics work
#[derive(DBMapUtils)]
struct TablesGenerics<Q, W> {
    table1: DBMap<String, String>,
    table2: DBMap<u32, Generic<Q, W>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Generic<T, V> {
    field1: T,
    field2: V,
}

#[derive(DBMapUtils)]
struct RenameTables1 {
    table: DBMap<String, String>,
}

#[derive(DBMapUtils)]
struct RenameTables2 {
    #[rename = "table"]
    renamed_table: DBMap<String, String>,
}

impl<
    T: Eq + Debug + Serialize + for<'de> Deserialize<'de>,
    V: Eq + Debug + Serialize + for<'de> Deserialize<'de>,
> Generic<T, V>
{
}

/// This struct shows that single elem structs work
#[derive(DBMapUtils)]
struct TablesSingle {
    table1: DBMap<String, String>,
}

#[tokio::test]
async fn macro_test() {
    let primary_path = temp_dir();
    let tbls_primary =
        Tables::open_tables_read_write(primary_path.clone(), MetricConf::default(), None, None);

    // Write to both tables
    let mut raw_key_bytes1 = 0;
    let mut raw_value_bytes1 = 0;
    let kv_range = 1..10;
    for i in kv_range.clone() {
        let key = i.to_string();
        let value = i.to_string();
        let k_buf = be_fix_int_ser::<String>(&key);
        let value_buf = bcs::to_bytes::<String>(&value).unwrap();
        raw_key_bytes1 += k_buf.len();
        raw_value_bytes1 += value_buf.len();
    }
    let keys_vals_1 = kv_range.map(|i| (i.to_string(), i.to_string()));
    tbls_primary
        .table1
        .multi_insert(keys_vals_1.clone())
        .expect("Failed to multi-insert");

    let mut raw_key_bytes2 = 0;
    let mut raw_value_bytes2 = 0;
    let kv_range = 3..10;
    for i in kv_range.clone() {
        let key = i;
        let value = i.to_string();
        let k_buf = be_fix_int_ser(key.borrow());
        let value_buf = bcs::to_bytes::<String>(&value).unwrap();
        raw_key_bytes2 += k_buf.len();
        raw_value_bytes2 += value_buf.len();
    }
    let keys_vals_2 = kv_range.map(|i| (i, i.to_string()));
    tbls_primary
        .table2
        .multi_insert(keys_vals_2.clone())
        .expect("Failed to multi-insert");

    // Open in secondary mode
    let tbls_secondary =
        Tables::get_read_only_handle(primary_path, None, None, MetricConf::default());

    // Check all the tables can be listed
    let observed_table_names: HashSet<_> = Tables::describe_tables()
        .iter()
        .map(|q| q.0.clone())
        .collect();

    let exp: HashSet<String> =
        HashSet::from_iter(vec!["table1", "table2"].into_iter().map(|s| s.to_owned()));
    assert_eq!(HashSet::from_iter(observed_table_names), exp);

    // check raw byte sizes of key and values
    let summary1 = tbls_secondary.table_summary("table1").unwrap();
    assert_eq!(9, summary1.num_keys);
    assert_eq!(raw_key_bytes1, summary1.key_bytes_total);
    assert_eq!(raw_value_bytes1, summary1.value_bytes_total);
    let summary2 = tbls_secondary.table_summary("table2").unwrap();
    assert_eq!(7, summary2.num_keys);
    assert_eq!(raw_key_bytes2, summary2.key_bytes_total);
    assert_eq!(raw_value_bytes2, summary2.value_bytes_total);

    // Test all entries
    let m = tbls_secondary.dump("table1", 100, 0).unwrap();
    for (k, v) in keys_vals_1 {
        assert_eq!(format!("\"{v}\""), *m.get(&format!("\"{k}\"")).unwrap());
    }

    let m = tbls_secondary.dump("table2", 100, 0).unwrap();
    for (k, v) in keys_vals_2 {
        assert_eq!(format!("\"{v}\""), *m.get(&k.to_string()).unwrap());
    }

    // Check that catchup logic works
    let keys_vals_1 = (100..110).map(|i| (i.to_string(), i.to_string()));
    tbls_primary
        .table1
        .multi_insert(keys_vals_1)
        .expect("Failed to multi-insert");

    // Test pagination
    let m = tbls_secondary.dump("table1", 2, 0).unwrap();
    assert_eq!(2, m.len());
    assert_eq!(format!("\"1\""), *m.get("\"1\"").unwrap());
    assert_eq!(format!("\"2\""), *m.get("\"2\"").unwrap());

    let m = tbls_secondary.dump("table1", 3, 2).unwrap();
    assert_eq!(3, m.len());
    assert_eq!(format!("\"7\""), *m.get("\"7\"").unwrap());
    assert_eq!(format!("\"8\""), *m.get("\"8\"").unwrap());
}

#[tokio::test]
async fn rename_test() {
    let dbdir = temp_dir();

    let key = "key".to_string();
    let value = "value".to_string();
    {
        let original_db =
            RenameTables1::open_tables_read_write(dbdir.clone(), MetricConf::default(), None, None);
        original_db.table.insert(&key, &value).unwrap();
    }

    // sleep for 1 second
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    {
        let renamed_db =
            RenameTables2::open_tables_read_write(dbdir.clone(), MetricConf::default(), None, None);
        assert_eq!(renamed_db.renamed_table.get(&key), Ok(Some(value)));
    }
}

#[derive(DBMapUtils)]
struct DeprecatedTables {
    table1: DBMap<String, String>,
    #[deprecated_db_map]
    table2: Option<DBMap<i32, String>>,
}

#[tokio::test]
async fn deprecate_test() {
    let dbdir = temp_dir();
    let key = "key".to_string();
    let value = "value".to_string();
    {
        let original_db =
            Tables::open_tables_read_write(dbdir.clone(), MetricConf::default(), None, None);
        original_db.table1.insert(&key, &value).unwrap();
        original_db.table2.insert(&0, &value).unwrap();
    }

    // First open: table2 CF exists on disk, gets dropped during cleanup
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    {
        let db = DeprecatedTables::open_tables_read_write(
            dbdir.clone(),
            MetricConf::default(),
            None,
            None,
        );
        assert_eq!(db.table1.get(&key), Ok(Some(value.clone())));
        // After cleanup, deprecated field is set to None
        assert!(
            db.table2.is_none(),
            "deprecated field should be None after cleanup"
        );
    }

    // Verify table2 CF was actually removed from disk
    let tables = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
    assert!(
        !tables.contains(&"table2".to_string()),
        "table2 CF should have been dropped"
    );

    // Second open: table2 CF no longer exists on disk — must not panic
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    {
        let db = DeprecatedTables::open_tables_read_write(
            dbdir.clone(),
            MetricConf::default(),
            None,
            None,
        );
        assert_eq!(db.table1.get(&key), Ok(Some(value.clone())));
        assert!(
            db.table2.is_none(),
            "deprecated field should be None when CF never existed"
        );
    }
}

/// Proves that a deprecated CF can use `(), ()` type parameters even when the
/// original CF had different key/value types (i32, String). This lets us delete
/// the old type definitions entirely.
#[derive(DBMapUtils)]
struct DeprecatedTablesTypeErased {
    table1: DBMap<String, String>,
    #[deprecated_db_map]
    table2: Option<DBMap<(), ()>>,
}

#[tokio::test]
async fn deprecate_type_erased_test() {
    let dbdir = temp_dir();
    let key = "key".to_string();
    let value = "value".to_string();

    // Step 1: Create DB with original types (i32, String) and write data
    {
        let original_db =
            Tables::open_tables_read_write(dbdir.clone(), MetricConf::default(), None, None);
        original_db.table1.insert(&key, &value).unwrap();
        original_db.table2.insert(&42, &value).unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Step 2: Reopen with type-erased `(), ()` — CF should be dropped cleanly
    {
        let db = DeprecatedTablesTypeErased::open_tables_read_write(
            dbdir.clone(),
            MetricConf::default(),
            None,
            None,
        );
        assert_eq!(db.table1.get(&key), Ok(Some(value.clone())));
        assert!(
            db.table2.is_none(),
            "deprecated field should be None after cleanup"
        );
    }

    // Verify table2 CF was actually removed from disk
    let tables = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
    assert!(
        !tables.contains(&"table2".to_string()),
        "table2 CF should have been dropped even with type-erased (), ()"
    );

    // Step 3: Reopen again — must not panic when CF is already gone
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    {
        let db = DeprecatedTablesTypeErased::open_tables_read_write(
            dbdir.clone(),
            MetricConf::default(),
            None,
            None,
        );
        assert_eq!(db.table1.get(&key), Ok(Some(value.clone())));
        assert!(
            db.table2.is_none(),
            "deprecated field should be None when CF never existed"
        );
    }
}

#[derive(DBMapUtils)]
struct TablesWithMigration {
    new_table: DBMap<String, String>,
    #[allow(dead_code)]
    #[deprecated_db_map(migration = "migrate_old_to_new")]
    old_table: Option<DBMap<String, String>>,
}

fn migrate_old_to_new(
    db: &std::sync::Arc<typed_store::database::Database>,
) -> Result<(), typed_store::TypedStoreError> {
    use typed_store::traits::Map;
    let old = typed_store::rocks::DBMap::<String, String>::reopen(
        db,
        Some("old_table"),
        &typed_store::rocks::ReadWriteOptions::default(),
        true,
    )?;
    let new = typed_store::rocks::DBMap::<String, String>::reopen(
        db,
        Some("new_table"),
        &typed_store::rocks::ReadWriteOptions::default(),
        false,
    )?;
    let mut batch = new.batch();
    for item in old.safe_iter() {
        let (k, v) = item?;
        batch.insert_batch(&new, std::iter::once((k, v)))?;
    }
    batch.write()?;
    Ok(())
}

#[tokio::test]
async fn migration_test() {
    let dbdir = temp_dir();
    let key = "migrate_key".to_string();
    let value = "migrate_value".to_string();

    // Step 1: Write data directly to old_table by opening with the full set of CFs
    // (new_table + old_table) that TablesWithMigration uses.
    {
        use typed_store::rocks::open_cf_opts;
        let opt_cfs: Vec<(&str, typed_store::rocksdb::Options)> = vec![
            (
                "new_table",
                typed_store::rocks::default_db_options().options,
            ),
            (
                "old_table",
                typed_store::rocks::default_db_options().options,
            ),
        ];
        let db = open_cf_opts(&dbdir, None, MetricConf::default(), &opt_cfs)
            .expect("Failed to open DB for setup");
        let old = typed_store::rocks::DBMap::<String, String>::reopen(
            &db,
            Some("old_table"),
            &typed_store::rocks::ReadWriteOptions::default(),
            false,
        )
        .unwrap();
        old.insert(&key, &value).unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Verify that the old_table exists
    let tables = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
    assert!(
        tables.contains(&"old_table".to_string()),
        "old_table should exist before migration"
    );

    // Step 2: Open with TablesWithMigration — migration should run and old_table
    // should be dropped
    let db = TablesWithMigration::open_tables_read_write(
        dbdir.clone(),
        MetricConf::default(),
        None,
        None,
    );

    // The migration should have copied key→value from old_table into new_table
    assert_eq!(db.new_table.get(&key), Ok(Some(value.clone())));

    // old_table CF should have been dropped — verify by checking table list
    drop(db);
    let tables = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
    assert!(
        !tables.contains(&"old_table".to_string()),
        "old_table should have been dropped"
    );

    // Step 3: Reopen after migration — old_table no longer exists on disk.
    // This must not panic and migrated data must still be in new_table.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let db = TablesWithMigration::open_tables_read_write(
        dbdir.clone(),
        MetricConf::default(),
        None,
        None,
    );
    assert_eq!(
        db.new_table.get(&key),
        Ok(Some(value.clone())),
        "migrated data should survive restart"
    );
    assert!(
        db.old_table.is_none(),
        "deprecated field should be None when CF doesn't exist"
    );
}

#[tokio::test]
async fn read_only_with_deprecated_and_migration_test() {
    let dbdir = temp_dir();
    let old_key = "old_key".to_string();
    let old_value = "old_value".to_string();

    // Seed data into old_table only (new_table is empty)
    {
        use typed_store::rocks::open_cf_opts;
        let opt_cfs: Vec<(&str, typed_store::rocksdb::Options)> = vec![
            (
                "new_table",
                typed_store::rocks::default_db_options().options,
            ),
            (
                "old_table",
                typed_store::rocks::default_db_options().options,
            ),
        ];
        let db = open_cf_opts(&dbdir, None, MetricConf::default(), &opt_cfs)
            .expect("Failed to open DB for setup");
        let old = typed_store::rocks::DBMap::<String, String>::reopen(
            &db,
            Some("old_table"),
            &typed_store::rocks::ReadWriteOptions::default(),
            false,
        )
        .unwrap();
        old.insert(&old_key, &old_value).unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Open read-only with a migration-bearing struct.
    // Secondary mode must NOT run migration or drop the CF.
    let db =
        TablesWithMigration::get_read_only_handle(dbdir.clone(), None, None, MetricConf::default());
    // Deprecated CF still exists on disk (no cleanup in secondary mode),
    // so the field should be Some
    assert!(
        db.old_table.is_some(),
        "deprecated field should be Some in read-only mode when CF exists"
    );
    // Migration did NOT run — new_table should be empty
    assert_eq!(
        db.new_table.get(&old_key),
        Ok(None),
        "migration must not run in read-only mode"
    );
    drop(db);

    // Verify old_table was NOT dropped from disk
    let tables = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
    assert!(
        tables.contains(&"old_table".to_string()),
        "read-only mode must not drop deprecated CFs"
    );

    // Now do the actual read-write open to trigger migration + cleanup
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let db = TablesWithMigration::open_tables_read_write(
        dbdir.clone(),
        MetricConf::default(),
        None,
        None,
    );
    // Migration should have copied data from old_table into new_table
    assert_eq!(db.new_table.get(&old_key), Ok(Some(old_value.clone())));
    assert!(db.old_table.is_none());
    drop(db);

    // Verify old_table was dropped from disk
    let tables = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
    assert!(
        !tables.contains(&"old_table".to_string()),
        "old_table should have been dropped after read-write open"
    );

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Open read-only again — old_table is now gone
    let db =
        TablesWithMigration::get_read_only_handle(dbdir.clone(), None, None, MetricConf::default());
    assert!(
        db.old_table.is_none(),
        "deprecated field should be None in read-only mode when CF was dropped"
    );
}

/// We show that custom functions can be applied
#[derive(DBMapUtils)]
struct TablesCustomOptions {
    #[default_options_override_fn = "another_custom_fn_name"]
    table1: DBMap<String, String>,
    table2: DBMap<i32, String>,
    #[default_options_override_fn = "custom_fn_name"]
    table3: DBMap<i32, String>,
    #[default_options_override_fn = "another_custom_fn_name"]
    table4: DBMap<i32, String>,
}

static TABLE1_OPTIONS_SET_FLAG: Lazy<Mutex<Vec<bool>>> = Lazy::new(|| Mutex::new(vec![]));
static TABLE2_OPTIONS_SET_FLAG: Lazy<Mutex<Vec<bool>>> = Lazy::new(|| Mutex::new(vec![]));

fn custom_fn_name() -> typed_store::rocks::DBOptions {
    TABLE1_OPTIONS_SET_FLAG.lock().unwrap().push(false);
    typed_store::rocks::DBOptions::default()
}

fn another_custom_fn_name() -> typed_store::rocks::DBOptions {
    TABLE2_OPTIONS_SET_FLAG.lock().unwrap().push(false);
    TABLE2_OPTIONS_SET_FLAG.lock().unwrap().push(false);
    TABLE2_OPTIONS_SET_FLAG.lock().unwrap().push(false);
    typed_store::rocks::DBOptions::default()
}

/// We show that custom functions can be applied
#[derive(DBMapUtils)]
struct TablesMemUsage {
    table1: DBMap<String, String>,
    table2: DBMap<i32, String>,
    table3: DBMap<i32, String>,
    table4: DBMap<i32, String>,
}

#[tokio::test]
async fn test_sampling() {
    let sampling_interval = SamplingInterval::new(Duration::ZERO, 10);
    for _i in 0..10 {
        assert!(!sampling_interval.sample());
    }
    assert!(sampling_interval.sample());
    for _i in 0..10 {
        assert!(!sampling_interval.sample());
    }
    assert!(sampling_interval.sample());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_sampling_time() {
    let sampling_interval = SamplingInterval::new(Duration::from_secs(1), 10);
    for _i in 0..10 {
        assert!(!sampling_interval.sample());
    }
    assert!(!sampling_interval.sample());
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(sampling_interval.sample());
    for _i in 0..10 {
        assert!(!sampling_interval.sample());
    }
    assert!(!sampling_interval.sample());
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(sampling_interval.sample());
}
