// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Debug, path::PathBuf, str::FromStr};

use anyhow::{anyhow, bail};
use iota_core::jsonrpc_index::IndexStoreTables;
use iota_sdk_types::{Address, ObjectId};
use serde::{Serialize, de::DeserializeOwned};
use typed_store::{
    rocks::{DBMap, MetricConf},
    traits::Map,
};

use crate::get_db_entries;

#[derive(Clone, Debug)]
pub enum SearchRange<T: Serialize + Clone + Debug> {
    ExclusiveLastKey(T),
    Count(u64),
}

impl<T: Serialize + Clone + Debug + FromStr> FromStr for SearchRange<T>
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let last_key = T::from_str(s).map_err(|e| anyhow!("Failed to parse last_key: {e:?}"))?;
        Ok(SearchRange::ExclusiveLastKey(last_key))
    }
}

/// Until we use a proc macro to auto derive this, we have to make sure to
/// update the `search_index` function below when adding new tables.
pub fn search_index(
    db_path: PathBuf,
    table_name: String,
    start: String,
    termination: SearchRange<String>,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    let start = start.as_str();
    println!("Opening db at {db_path:?} ...");
    let db_read_only_handle =
        IndexStoreTables::get_read_only_handle(db_path, None, None, MetricConf::default());
    match table_name.as_str() {
        "owner_index" => {
            get_db_entries!(
                db_read_only_handle.owner_index,
                from_addr_oid,
                start,
                termination
            )
        }
        "dynamic_field_index" => {
            get_db_entries!(
                db_read_only_handle.dynamic_field_index,
                from_oid_oid,
                start,
                termination
            )
        }
        _ => bail!("Invalid or unsupported table: {table_name}"),
    }
}

#[macro_export]
macro_rules! get_db_entries {
    ($db_map:expr, $key_converter:expr, $start:expr, $term:expr) => {{
        let key = $key_converter($start)?;
        println!("Searching from key: {:?}", key);
        let termination = match $term {
            SearchRange::ExclusiveLastKey(last_key) => {
                println!(
                    "Retrieving all keys up to (but not including) key: {:?}",
                    key
                );
                SearchRange::ExclusiveLastKey($key_converter(last_key.as_str())?)
            }
            SearchRange::Count(count) => {
                println!("Retrieving up to {} keys", count);
                SearchRange::Count(count)
            }
        };

        $db_map.try_catch_up_with_primary().unwrap();
        get_entries_to_str(&$db_map, key, termination)
    }};
}

fn get_entries_to_str<K, V>(
    db_map: &DBMap<K, V>,
    start: K,
    termination: SearchRange<K>,
) -> Result<Vec<(String, String)>, anyhow::Error>
where
    K: Serialize + serde::de::DeserializeOwned + Clone + Debug,
    V: serde::Serialize + DeserializeOwned + Clone + Debug,
{
    get_entries(db_map, start, termination).map(|entries| {
        entries
            .into_iter()
            .map(|(k, v)| (format!("{k:?}"), format!("{v:?}")))
            .collect()
    })
}

fn get_entries<K, V>(
    db_map: &DBMap<K, V>,
    start: K,
    termination: SearchRange<K>,
) -> Result<Vec<(K, V)>, anyhow::Error>
where
    K: Serialize + serde::de::DeserializeOwned + Clone + std::fmt::Debug,
    V: serde::Serialize + DeserializeOwned + Clone,
{
    let mut entries = Vec::new();
    match termination {
        SearchRange::ExclusiveLastKey(exclusive_last_key) => {
            let iter = db_map.safe_iter_with_bounds(Some(start), Some(exclusive_last_key));

            for result in iter {
                let (key, value) = result?;
                entries.push((key.clone(), value.clone()));
            }
        }
        SearchRange::Count(mut count) => {
            let mut iter = db_map.safe_iter_with_bounds(Some(start), None);

            while count > 0 {
                if let Some(result) = iter.next() {
                    let (key, value) = result?;
                    entries.push((key.clone(), value.clone()));
                } else {
                    break;
                }
                count -= 1;
            }
        }
    }
    Ok(entries)
}

fn from_addr_oid(s: &str) -> Result<(Address, ObjectId), anyhow::Error> {
    // Remove whitespaces
    let s = s.trim();
    let tokens = s.split(',').collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid address, object id pair");
    }
    let addr = Address::from_str(tokens[0].trim())?;
    let oid = ObjectId::from_str(tokens[1].trim())?;

    Ok((addr, oid))
}

fn from_oid_oid(s: &str) -> Result<(ObjectId, ObjectId), anyhow::Error> {
    // Remove whitespaces
    let s = s.trim();
    let tokens = s.split(',').collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid object id, object id triplet");
    }
    let oid1 = ObjectId::from_str(tokens[0].trim())?;
    let oid2: ObjectId = ObjectId::from_str(tokens[1].trim())?;

    Ok((oid1, oid2))
}
