// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Debug, path::PathBuf, str::FromStr, sync::Arc};

use anyhow::{anyhow, bail};
use iota_core::rpc_indexes::schema::{
    DB_PREFIX_HISTORIC_DIGESTS, DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE,
    DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE, DB_PREFIX_HISTORIC_EVENT_BY_SENDER,
    DB_PREFIX_HISTORIC_EVENT_ORDER, DB_PREFIX_HISTORIC_TX_ORDER,
    DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID, DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION,
    DB_PREFIX_HISTORIC_TXS_BY_MUTATED_OBJECT_ID, DB_PREFIX_HISTORIC_TXS_FROM_ADDR,
    DB_PREFIX_HISTORIC_TXS_TO_ADDR, IndexStoreTables, OwnerIndexKey, history_cf_epoch,
    history_cf_name,
};
use iota_sdk_types::{Address, ObjectId, TransactionDigest, TransactionEventsDigest};
use iota_types::{
    base_types::TxSequenceNumber, committee::EpochId,
    messages_checkpoint::CheckpointSequenceNumber, storage::DynamicFieldKey,
};
use move_core_types::{account_address::AccountAddress, language_storage::ModuleId};
use serde::{Serialize, de::DeserializeOwned};
use typed_store::{
    database::Database,
    rocks::{
        DBMap, MetricConf, ReadWriteOptions, TaggedDBMap, list_tables, open_cf_opts_secondary,
    },
    rocksdb,
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
    match table_name.as_str() {
        "owner" => {
            let db_read_only_handle =
                IndexStoreTables::get_read_only_handle(db_path, None, None, MetricConf::default());
            get_db_entries!(
                db_read_only_handle.owner,
                from_addr_owner_key,
                start,
                termination
            )
        }
        "dynamic_field" => {
            let db_read_only_handle =
                IndexStoreTables::get_read_only_handle(db_path, None, None, MetricConf::default());
            get_db_entries!(
                db_read_only_handle.dynamic_field,
                from_parent_field_key,
                start,
                termination
            )
        }
        // Names from the pre-unification JSON-RPC-only index store, kept
        // around only to point callers at the table's new name rather than
        // falling through to `search_history_table`'s generic "unsupported
        // table" error.
        "owner_index" => bail!("no such table \"owner_index\"; it was renamed to \"owner\""),
        "dynamic_field_index" => {
            bail!("no such table \"dynamic_field_index\"; it was renamed to \"dynamic_field\"")
        }
        "txs_seq" => bail!(
            "no such table \"txs_seq\"; transaction digest to sequence number lookups are now \
             served by the \"digests\" table"
        ),
        _ => search_history_table(db_path, &table_name, start, termination),
    }
}

/// Searches one of the transaction/event history tables, which live in
/// per-epoch column families sharing one table per tag byte. The buckets
/// partition the sequence order by epoch, so chaining the per-bucket scans
/// in epoch order yields globally ordered entries.
fn search_history_table(
    db_path: PathBuf,
    table_name: &str,
    start: &str,
    termination: SearchRange<String>,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    let cf_names = list_tables(db_path.clone())
        .map_err(|e| anyhow!("unable to list the column families of {db_path:?}: {e}"))?;
    let mut epochs: Vec<EpochId> = cf_names
        .iter()
        .filter_map(|name| history_cf_epoch(name))
        .collect();
    epochs.sort_unstable();
    if epochs.is_empty() {
        bail!("the database at {db_path:?} holds no history column families");
    }
    // Every existing column family must be listed for RocksDB to open the
    // database; the scan itself only touches the history buckets.
    let opt_cfs: Vec<(&str, rocksdb::Options)> = cf_names
        .iter()
        .map(|name| (name.as_str(), rocksdb::Options::default()))
        .collect();
    let db = open_cf_opts_secondary(
        &db_path,
        None,
        None,
        MetricConf::new("index_search"),
        &opt_cfs,
    )?;

    match table_name {
        "tx_order" => get_history_entries::<TxSequenceNumber, TransactionDigest>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_TX_ORDER,
            |s| Ok(TxSequenceNumber::from_str(s)?),
            start,
            termination,
        ),
        "digests" => {
            get_history_entries::<TransactionDigest, (TxSequenceNumber, CheckpointSequenceNumber)>(
                &db,
                &epochs,
                DB_PREFIX_HISTORIC_DIGESTS,
                |s| Ok(TransactionDigest::from_str(s)?),
                start,
                termination,
            )
        }
        "txs_from_addr" => get_history_entries::<_, TransactionDigest>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_TXS_FROM_ADDR,
            from_addr_seq,
            start,
            termination,
        ),
        "txs_to_addr" => get_history_entries::<_, TransactionDigest>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_TXS_TO_ADDR,
            from_addr_seq,
            start,
            termination,
        ),
        "txs_by_input_object_id" => get_history_entries::<_, TransactionDigest>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID,
            from_id_seq,
            start,
            termination,
        ),
        "txs_by_mutated_object_id" => get_history_entries::<_, TransactionDigest>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_TXS_BY_MUTATED_OBJECT_ID,
            from_id_seq,
            start,
            termination,
        ),
        "txs_by_move_function" => get_history_entries::<_, TransactionDigest>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION,
            from_id_module_function_txseq,
            start,
            termination,
        ),
        "event_order" => get_history_entries::<_, EventValue>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_EVENT_ORDER,
            from_event_id,
            start,
            termination,
        ),
        "event_by_move_module" => get_history_entries::<_, EventValue>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE,
            from_module_id_and_event_id,
            start,
            termination,
        ),
        "event_by_event_module" => get_history_entries::<_, EventValue>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE,
            from_module_id_and_event_id,
            start,
            termination,
        ),
        "event_by_sender" => get_history_entries::<_, EventValue>(
            &db,
            &epochs,
            DB_PREFIX_HISTORIC_EVENT_BY_SENDER,
            from_address_and_event_id,
            start,
            termination,
        ),
        _ => bail!("Invalid or unsupported table: {table_name}"),
    }
}

/// The value shared by every event history table.
type EventValue = (TransactionEventsDigest, TransactionDigest, u64);

/// [`get_entries`] over one history table's per-epoch maps, in epoch order.
fn get_history_entries<K, V>(
    db: &Arc<Database>,
    epochs: &[EpochId],
    tag: u8,
    key_converter: impl Fn(&str) -> Result<K, anyhow::Error>,
    start: &str,
    termination: SearchRange<String>,
) -> Result<Vec<(String, String)>, anyhow::Error>
where
    K: Serialize + DeserializeOwned + Clone + Debug,
    V: Serialize + DeserializeOwned + Debug,
{
    let start = key_converter(start)?;
    println!("Searching from key: {start:?}");
    let termination = match termination {
        SearchRange::ExclusiveLastKey(last_key) => {
            println!("Retrieving all keys up to (but not including) key: {last_key:?}");
            SearchRange::ExclusiveLastKey(key_converter(last_key.as_str())?)
        }
        SearchRange::Count(count) => {
            println!("Retrieving up to {count} keys");
            SearchRange::Count(count)
        }
    };

    let mut entries = Vec::new();
    for epoch in epochs {
        let map: TaggedDBMap<K, V> = TaggedDBMap::reopen(
            db,
            &history_cf_name(*epoch),
            tag,
            &ReadWriteOptions::default(),
            true,
        )?;
        map.try_catch_up_with_primary()?;
        let upper_bound = match &termination {
            SearchRange::ExclusiveLastKey(last_key) => Some(last_key.clone()),
            SearchRange::Count(_) => None,
        };
        for result in map.safe_iter_with_bounds(Some(start.clone()), upper_bound) {
            let (key, value) = result?;
            entries.push((format!("{key:?}"), format!("{value:?}")));
            if let SearchRange::Count(count) = &termination {
                if entries.len() as u64 >= *count {
                    return Ok(entries);
                }
            }
        }
    }
    Ok(entries)
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

fn from_addr_seq(s: &str) -> Result<(Address, TxSequenceNumber), anyhow::Error> {
    // Remove whitespaces
    let s = s.trim();
    let tokens = s.split(',').collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid address, sequence number pair");
    }
    let address = Address::from_str(tokens[0].trim())?;
    let sequence_number = TxSequenceNumber::from_str(tokens[1].trim())?;

    Ok((address, sequence_number))
}

fn from_id_seq(s: &str) -> Result<(ObjectId, TxSequenceNumber), anyhow::Error> {
    // Remove whitespaces
    let s = s.trim();
    let tokens = s.split(',').collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid object id, sequence number pair");
    }
    let oid = ObjectId::from_str(tokens[0].trim())?;
    let sequence_number = TxSequenceNumber::from_str(tokens[1].trim())?;

    Ok((oid, sequence_number))
}

fn from_id_module_function_txseq(
    s: &str,
) -> Result<(ObjectId, String, String, TxSequenceNumber), anyhow::Error> {
    // Remove whitespaces
    let s = s.trim();
    let tokens = s.split(',').collect::<Vec<&str>>();
    if tokens.len() != 4 {
        bail!("Invalid object id, module name, function name, TX sequence number quad");
    }
    let pid = ObjectId::from_str(tokens[0].trim())?;
    let module = iota_sdk_types::Identifier::from_str(tokens[1].trim())?;
    let func = iota_sdk_types::Identifier::from_str(tokens[2].trim())?;
    let seq: TxSequenceNumber = TxSequenceNumber::from_str(tokens[3].trim())?;

    Ok((pid, module.to_string(), func.to_string(), seq))
}

/// The owner index's lowest key for an address, so a scan can be bounded by
/// the address alone. The rest of the key (type hashes, inverted balance,
/// object id) is derived from the object, which the caller does not have.
fn from_addr_owner_key(s: &str) -> Result<OwnerIndexKey, anyhow::Error> {
    let owner = Address::from_str(s.trim())?;
    Ok(OwnerIndexKey {
        owner,
        object_type_identifier: 0,
        object_type_params: 0,
        inverted_balance: None,
        object_id: ObjectId::ZERO,
    })
}

fn from_parent_field_key(s: &str) -> Result<DynamicFieldKey, anyhow::Error> {
    // Remove whitespaces
    let s = s.trim();
    let tokens = s.split(',').collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid parent object id, field object id pair");
    }
    let parent = ObjectId::from_str(tokens[0].trim())?;
    let field_id = ObjectId::from_str(tokens[1].trim())?;

    Ok(DynamicFieldKey::new(parent, field_id))
}

fn from_module_id_and_event_id(
    s: &str,
) -> Result<(ModuleId, (TxSequenceNumber, usize)), anyhow::Error> {
    // Example: "0x1::Event 1234 5"
    let tokens = s.split(' ').collect::<Vec<&str>>();
    if tokens.len() != 3 {
        bail!("Invalid input");
    }
    let tx_seq = TxSequenceNumber::from_str(tokens[1])?;
    let event_seq = usize::from_str(tokens[2])?;
    let tokens = tokens[0].split("::").collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid module id");
    }
    let package = ObjectId::from_str(tokens[0].trim())?;

    Ok((
        ModuleId::new(
            AccountAddress::new(package.into_bytes()),
            move_core_types::identifier::Identifier::from_str(tokens[1].trim())?,
        ),
        (tx_seq, event_seq),
    ))
}

fn from_event_id(s: &str) -> Result<(TxSequenceNumber, usize), anyhow::Error> {
    // Example: "1234 5"
    let tokens = s.split(' ').collect::<Vec<&str>>();
    if tokens.len() != 2 {
        bail!("Invalid input");
    }
    let tx_seq = TxSequenceNumber::from_str(tokens[0])?;
    let event_seq = usize::from_str(tokens[1])?;
    Ok((tx_seq, event_seq))
}

fn from_address_and_event_id(
    s: &str,
) -> Result<(Address, (TxSequenceNumber, usize)), anyhow::Error> {
    // Example: "0x1 1234 5"
    let tokens = s.split(' ').collect::<Vec<&str>>();
    if tokens.len() != 3 {
        bail!("Invalid input");
    }
    let tx_seq = TxSequenceNumber::from_str(tokens[1])?;
    let event_seq = usize::from_str(tokens[2])?;
    let address = Address::from_str(tokens[0].trim())?;
    Ok((address, (tx_seq, event_seq)))
}
