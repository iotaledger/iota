// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Do not reuse these tags. Mark them as deprecated if a table is removed.
pub const DB_PREFIX_HISTORIC_TX_ORDER: u8 = 0;

pub const DB_PREFIX_HISTORIC_TXS_FROM_ADDR: u8 = 2;

pub const DB_PREFIX_HISTORIC_TXS_TO_ADDR: u8 = 3;

pub const DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID: u8 = 4;

pub const DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION: u8 = 6;

pub const DB_PREFIX_HISTORIC_EVENT_ORDER: u8 = 7;

pub const DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE: u8 = 8;

pub const DB_PREFIX_HISTORIC_EVENT_BY_MOVE_EVENT: u8 = 9;

pub const DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE: u8 = 10;

pub const DB_PREFIX_HISTORIC_EVENT_BY_SENDER: u8 = 11;

pub const DB_PREFIX_HISTORIC_EVENT_BY_TIME: u8 = 12;

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub struct TotalBalance {
    pub balance: i128,
    pub num_coins: i64,
}

/// Whether the object is a `Field` object of a dynamic field — the only
/// objects the dynamic-field index stores.
pub(crate) fn is_dynamic_field(object: &Object) -> bool {
    object
        .data
        .as_opt_struct()
        .is_some_and(|move_object| move_object.struct_tag().is_dynamic_field())
}

/// Per-transaction inputs for the history tables of the index batch. Unlike
/// the live-state tables (owner, coin, dynamic field), these need only the
/// transaction, its effects, and its events — no object contents.
pub(crate) struct TransactionIndexData {
    pub(crate) digest: TransactionDigest,
    pub(crate) sender: Address,
    pub(crate) active_inputs: Vec<ObjectId>,
    pub(crate) mutated_objects: Vec<(ObjectReference, Owner)>,
    pub(crate) move_functions: Vec<(ObjectId, String, String)>,
    pub(crate) events: TransactionEvents,
}

/// Extracts one transaction's history-table index inputs.
pub(crate) fn transaction_index_data(
    transaction: &TransactionEnvelope,
    effects: &TransactionEffects,
    events: Option<&TransactionEvents>,
) -> IotaResult<TransactionIndexData> {
    let tx_data = &transaction.intent_message().value;

    Ok(TransactionIndexData {
        digest: *effects.transaction_digest(),
        sender: tx_data.sender(),
        active_inputs: tx_data
            .input_objects()?
            .iter()
            .map(|o| o.object_id())
            .collect(),
        mutated_objects: effects
            .all_changed_objects()
            .into_iter()
            .map(|(changed, _kind)| (changed.reference, changed.owner))
            .collect(),
        move_functions: tx_data
            .move_calls()
            .into_iter()
            .map(|(package, module, function)| (*package, module.to_owned(), function.to_owned()))
            .collect(),
        events: events.cloned().unwrap_or_default(),
    })
}

impl HistoryBucket {
    pub(crate) fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        // The tags are each table's identity within the shared column
        // family; never change or reuse them for existing data. Per-epoch
        // column families skip the periodic metrics reporter task: with up
        // to ~100 retained epochs, one task per column family adds up.
        fn map<K, V>(
            db: &Arc<Database>,
            cf_name: &str,
            tag: u8,
        ) -> Result<TaggedDBMap<K, V>, TypedStoreError>
        where
            K: Clone + Serialize + DeserializeOwned,
            V: Serialize + DeserializeOwned,
        {
            TaggedDBMap::reopen(db, cf_name, tag, &ReadWriteOptions::default(), true)
        }
        Ok(Self {
            tx_order: map(db, cf_name, DB_PREFIX_HISTORIC_TX_ORDER)?,
            txs_seq: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_SEQ)?,
            txs_from_addr: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_FROM_ADDR)?,
            txs_to_addr: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_TO_ADDR)?,
            txs_by_input_object_id: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID)?,
            txs_by_mutated_object_id: map(db, cf_name, DB_PREFIX_HIST_TXS_BY_MUTATED_OBJECT_ID)?,
            txs_by_move_function: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION)?,
            event_order: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_ORDER)?,
            event_by_move_module: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE)?,
            event_by_move_event: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_MOVE_EVENT)?,
            event_by_event_module: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE)?,
            event_by_sender: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_SENDER)?,
            event_by_time: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_TIME)?,
        })
    }

    /// Appends one transaction's history-table rows to a checkpoint's batch.
    pub(crate) fn index_tx(
        &self,
        batch: &mut DBBatch,
        sequence: TxSequenceNumber,
        timestamp_ms: u64,
        tx: TransactionIndexData,
    ) -> IotaResult {
        let TransactionIndexData {
            digest,
            sender,
            active_inputs,
            mutated_objects,
            move_functions,
            events,
        } = tx;

        batch.insert_batch_tagged(&self.tx_order, std::iter::once((sequence, digest)))?;

        batch.insert_batch_tagged(&self.txs_seq, std::iter::once((digest, sequence)))?;

        batch.insert_batch_tagged(
            &self.txs_from_addr,
            std::iter::once(((sender, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_by_input_object_id,
            active_inputs.into_iter().map(|id| ((id, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_by_mutated_object_id,
            mutated_objects
                .iter()
                .map(|(obj_ref, _)| ((obj_ref.object_id, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_by_move_function,
            move_functions
                .into_iter()
                .map(|(obj_id, module, function)| ((obj_id, module, function, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_to_addr,
            mutated_objects.iter().filter_map(|(_, owner)| {
                owner
                    .into_opt_address()
                    .map(|addr| ((addr, sequence), digest))
            }),
        )?;

        // events
        let event_digest = events.digest();
        batch.insert_batch_tagged(
            &self.event_order,
            events
                .iter()
                .enumerate()
                .map(|(i, _)| ((sequence, i), (event_digest, digest, timestamp_ms))),
        )?;
        batch.insert_batch_tagged(
            &self.event_by_move_module,
            events
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    (
                        i,
                        ModuleId::new(
                            AccountAddress::new(e.package_id.into_bytes()),
                            Identifier::new(e.module.as_str()).unwrap(),
                        ),
                    )
                })
                .map(|(i, m)| ((m, (sequence, i)), (event_digest, digest, timestamp_ms))),
        )?;
        batch.insert_batch_tagged(
            &self.event_by_sender,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (e.sender, (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;
        batch.insert_batch_tagged(
            &self.event_by_move_event,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (e.struct_tag.clone(), (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch_tagged(
            &self.event_by_time,
            events.iter().enumerate().map(|(i, _)| {
                (
                    (timestamp_ms, (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch_tagged(
            &self.event_by_event_module,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (
                        ModuleId::new(
                            AccountAddress::new(e.struct_tag.address().into_bytes()),
                            Identifier::new(e.struct_tag.module().as_str()).unwrap(),
                        ),
                        (sequence, i),
                    ),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MetadataInfo {
    /// Version of the Database
    pub(crate) version: u64,
}

pub fn history_cf_name(epoch: EpochId) -> String {
    rpc_index_history::bucket_cf_name(HISTORY_CF_PREFIX, epoch)
}

/// The epoch of a history column family, `None` for other names.
pub fn history_cf_epoch(cf_name: &str) -> Option<EpochId> {
    rpc_index_history::bucket_cf_epoch(HISTORY_CF_PREFIX, cf_name)
}
