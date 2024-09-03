// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{prelude::*, select};
use iota_json_rpc_types::{
    BalanceChange, IotaTransactionBlock, IotaTransactionBlockEffects, IotaTransactionBlockEvents,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
};
use iota_types::{
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEvents},
    event::Event,
    transaction::SenderSignedData,
};
use move_bytecode_utils::module_cache::GetModule;

use crate::{
    db::PgConnectionPool,
    errors::{Context, IndexerError},
    models::large_objects::{lo_create, lo_get, lo_put},
    schema::transactions,
    types::{IndexedObjectChange, IndexedTransaction, IndexerResult, TransactionKind},
};

#[derive(Clone, Debug, Queryable, Insertable, QueryableByName)]
#[diesel(table_name = transactions)]
pub struct StoredTransaction {
    pub tx_sequence_number: i64,
    pub transaction_digest: Vec<u8>,
    // ==> these are the values that we need to handle
    pub raw_transaction: Vec<u8>, // oid or it can remain a bytea and we handle the conversion
    // between oid and bytea on the client side based on conditions,
    //
    // 1. is there an easy way to switch from oid to bytea and vice versa? Either on the client
    //    side, or on the server side.
    //
    //    it is probably feasible on the client.
    // 2. how do we recognize the genesis transaction?
    //
    //    if `checkpoint_sequence_number == 0 && transaction_kind == SystemTransaction`
    // 3. Writing
    //
    //    ```
    //    let mut stored = StoredTransaction::from(IndexedTransaction);
    //    if stored.is_genesis_transaction() {
    //      let raw_tx = std::mem::take(&mut stored.raw_transaction);
    //      let oid = lo_from_bytea();
    //      stored.raw_transaction = oid_to_bytea(oid);
    //    }
    //
    //    insert stored; run query with diesel
    //
    // 4. Read
    //
    //    ```
    //    let mut stored = StoredTransaction::read();
    //    if stored.is_genesis_transaction() {
    //      let oid_bytes = std::mem::take(&mut stored.raw_transaction);
    //      let oid = bytea_to_oid(oid_bytes);
    //      let raw_tx = lo_get(oid);
    //      stored.raw_transaction = raw_tx;
    //    }
    //
    //    * We need to implement the server-side functions.
    //    * We need to implement the client-side conversion function between oid and bytea.
    pub raw_effects: Vec<u8>,
    // <==
    pub checkpoint_sequence_number: i64, // this is zero for genesis
    pub timestamp_ms: i64,
    pub object_changes: Vec<Option<Vec<u8>>>,
    pub balance_changes: Vec<Option<Vec<u8>>>,
    pub events: Vec<Option<Vec<u8>>>,
    pub transaction_kind: i16, // system transaction
    pub success_command_count: i16,
}

#[derive(Debug, Queryable)]
pub struct TxSeq {
    pub seq: i64,
}

impl Default for TxSeq {
    fn default() -> Self {
        Self { seq: -1 }
    }
}

#[derive(Clone, Debug, Queryable)]
pub struct StoredTransactionTimestamp {
    pub tx_sequence_number: i64,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug, Queryable)]
pub struct StoredTransactionCheckpoint {
    pub tx_sequence_number: i64,
    pub checkpoint_sequence_number: i64,
}

#[derive(Clone, Debug, Queryable)]
pub struct StoredTransactionSuccessCommandCount {
    pub tx_sequence_number: i64,
    pub checkpoint_sequence_number: i64,
    pub success_command_count: i16,
    pub timestamp_ms: i64,
}

impl From<&IndexedTransaction> for StoredTransaction {
    fn from(tx: &IndexedTransaction) -> Self {
        StoredTransaction {
            tx_sequence_number: tx.tx_sequence_number as i64,
            transaction_digest: tx.tx_digest.into_inner().to_vec(),
            // raw_transaction: bcs::to_bytes(&tx.sender_signed_data).unwrap(),
            raw_transaction: vec![0; 2 * 1024 * 1024 * 1024],
            raw_effects: bcs::to_bytes(&tx.effects).unwrap(),
            checkpoint_sequence_number: tx.checkpoint_sequence_number as i64,
            object_changes: tx
                .object_changes
                .iter()
                .map(|oc| Some(bcs::to_bytes(&oc).unwrap()))
                .collect(),
            balance_changes: tx
                .balance_change
                .iter()
                .map(|bc| Some(bcs::to_bytes(&bc).unwrap()))
                .collect(),
            events: tx
                .events
                .iter()
                .map(|e| Some(bcs::to_bytes(&e).unwrap()))
                .collect(),
            timestamp_ms: tx.timestamp_ms as i64,
            transaction_kind: tx.transaction_kind.clone() as i16,
            success_command_count: tx.successful_tx_num as i16,
        }
    }
}

impl StoredTransaction {
    const LARGE_OBJECT_CHUNK: usize = 100 * 1024 * 1024;

    pub fn try_into_iota_transaction_block_response(
        self,
        options: &IotaTransactionBlockResponseOptions,
        module: &impl GetModule,
    ) -> IndexerResult<IotaTransactionBlockResponse> {
        let tx_digest =
            TransactionDigest::try_from(self.transaction_digest.as_slice()).map_err(|e| {
                IndexerError::PersistentStorageDataCorruptionError(format!(
                    "Can't convert {:?} as tx_digest. Error: {e}",
                    self.transaction_digest
                ))
            })?;

        let transaction = if options.show_input {
            let sender_signed_data = self.try_into_sender_signed_data()?;
            let tx_block = IotaTransactionBlock::try_from(sender_signed_data, module)?;
            Some(tx_block)
        } else {
            None
        };

        let effects = if options.show_effects {
            let effects = self.try_into_iota_transaction_effects()?;
            Some(effects)
        } else {
            None
        };

        let raw_transaction = if options.show_raw_input {
            self.raw_transaction
        } else {
            Vec::new()
        };

        let events = if options.show_events {
            let events = self
                .events
                .into_iter()
                .map(|event| match event {
                    Some(event) => {
                        let event: Event = bcs::from_bytes(&event).map_err(|e| {
                            IndexerError::PersistentStorageDataCorruptionError(format!(
                                "Can't convert event bytes into Event. tx_digest={:?} Error: {e}",
                                tx_digest
                            ))
                        })?;
                        Ok(event)
                    }
                    None => Err(IndexerError::PersistentStorageDataCorruptionError(format!(
                        "Event should not be null, tx_digest={:?}",
                        tx_digest
                    ))),
                })
                .collect::<Result<Vec<Event>, IndexerError>>()?;
            let timestamp = self.timestamp_ms as u64;
            let tx_events = TransactionEvents { data: events };
            let tx_events = IotaTransactionBlockEvents::try_from_using_module_resolver(
                tx_events,
                tx_digest,
                Some(timestamp),
                module,
            )?;
            Some(tx_events)
        } else {
            None
        };

        let object_changes = if options.show_object_changes {
            let object_changes = self.object_changes.into_iter().map(|object_change| {
                match object_change {
                    Some(object_change) => {
                        let object_change: IndexedObjectChange = bcs::from_bytes(&object_change)
                            .map_err(|e| IndexerError::PersistentStorageDataCorruptionError(
                                format!("Can't convert object_change bytes into IndexedObjectChange. tx_digest={:?} Error: {e}", tx_digest)
                            ))?;
                        Ok(ObjectChange::from(object_change))
                    }
                    None => Err(IndexerError::PersistentStorageDataCorruptionError(format!("object_change should not be null, tx_digest={:?}", tx_digest))),
                }
            }).collect::<Result<Vec<ObjectChange>, IndexerError>>()?;

            Some(object_changes)
        } else {
            None
        };

        let balance_changes = if options.show_balance_changes {
            let balance_changes = self.balance_changes.into_iter().map(|balance_change| {
                match balance_change {
                    Some(balance_change) => {
                        let balance_change: BalanceChange = bcs::from_bytes(&balance_change)
                            .map_err(|e| IndexerError::PersistentStorageDataCorruptionError(
                                format!("Can't convert balance_change bytes into BalanceChange. tx_digest={:?} Error: {e}", tx_digest)
                            ))?;
                        Ok(balance_change)
                    }
                    None => Err(IndexerError::PersistentStorageDataCorruptionError(format!("object_change should not be null, tx_digest={:?}", tx_digest))),
                }
            }).collect::<Result<Vec<BalanceChange>, IndexerError>>()?;

            Some(balance_changes)
        } else {
            None
        };

        Ok(IotaTransactionBlockResponse {
            digest: tx_digest,
            transaction,
            raw_transaction,
            effects,
            events,
            object_changes,
            balance_changes,
            timestamp_ms: Some(self.timestamp_ms as u64),
            checkpoint: Some(self.checkpoint_sequence_number as u64),
            confirmed_local_execution: None,
            errors: vec![],
            raw_effects: self.raw_effects,
        })
    }

    fn try_into_sender_signed_data(&self) -> IndexerResult<SenderSignedData> {
        let sender_signed_data: SenderSignedData =
            bcs::from_bytes(&self.raw_transaction).map_err(|e| {
                IndexerError::PersistentStorageDataCorruptionError(format!(
                    "Can't convert raw_transaction of {} into SenderSignedData. Error: {e}",
                    self.tx_sequence_number
                ))
            })?;
        Ok(sender_signed_data)
    }

    pub fn try_into_iota_transaction_effects(&self) -> IndexerResult<IotaTransactionBlockEffects> {
        let effects: TransactionEffects = bcs::from_bytes(&self.raw_effects).map_err(|e| {
            IndexerError::PersistentStorageDataCorruptionError(format!(
                "Can't convert raw_effects of {} into TransactionEffects. Error: {e}",
                self.tx_sequence_number
            ))
        })?;
        let effects = IotaTransactionBlockEffects::try_from(effects)?;
        Ok(effects)
    }

    /// Check if this is the genesis transaction.
    pub fn is_genesis(&self) -> bool {
        self.checkpoint_sequence_number == 0
            && self.transaction_kind == TransactionKind::SystemTransaction as i16
    }

    pub fn try_into_large_object(mut self, pool: &PgConnectionPool) -> Result<Self, IndexerError> {
        if self.is_genesis() {
            let mut conn = crate::db::get_pg_pool_connection(pool)?;
            let raw_tx = std::mem::take(&mut self.raw_transaction);
            let oid: u32 = select(lo_create(0))
                .get_result(&mut conn)
                .map_err(IndexerError::from)
                .context("failed to store large object")?;
            for (i, chunk) in raw_tx.chunks(Self::LARGE_OBJECT_CHUNK).enumerate() {
                let offset = i64::try_from(i * Self::LARGE_OBJECT_CHUNK)
                    .map_err(|e| IndexerError::GenericError(e.to_string()))?;
                tracing::trace!("storing large-object chunk at offset {}", offset);

                select(lo_put(oid, offset, chunk))
                    .execute(&mut conn)
                    .map_err(IndexerError::from)
                    .context("failed to insert large object chunk")?;
            }
            self.raw_transaction = oid.to_le_bytes().to_vec();
        }
        Ok(self)
    }

    pub fn try_get_from_storage(
        tx_digest: Vec<u8>,
        pool: &PgConnectionPool,
    ) -> Result<Self, IndexerError> {
        // 1: get the transaction wich matches the tx digest
        let mut conn = crate::db::get_pg_pool_connection(pool)?;
        let mut stored = transactions::table
            .filter(transactions::transaction_digest.eq(tx_digest))
            .first::<Self>(&mut conn)?;
        // 2: check if it's not a genesis transaction
        if !stored.is_genesis() {
            return Ok(stored);
        }
        // if it's a genesis transaction
        //
        // 3: get the OID from raw_transactions and convert ti to u32
        let raw_oid = std::mem::take(&mut stored.raw_transaction);
        let raw_oid: [u8; 4] = raw_oid.try_into().map_err(|_| {
            IndexerError::GenericError("invalid large object identifier".to_owned())
        })?;
        let oid = u32::from_le_bytes(raw_oid);
        // 4: fetch the chubks from large_obj_table
        let mut chunk_num = 0;
        loop {
            let offset = i64::try_from(chunk_num * Self::LARGE_OBJECT_CHUNK)
                .map_err(|e| IndexerError::GenericError(e.to_string()))?;
            let length = i32::try_from(Self::LARGE_OBJECT_CHUNK)
                .map_err(|e| IndexerError::GenericError(e.to_string()))?;
            let chunk = select(lo_get(oid, Some(offset), Some(length)))
                .get_result::<Vec<u8>>(&mut conn)
                .map_err(IndexerError::from)
                .context("failed to insert large object chunk")?;
            let chunk_len = chunk.len();
            stored.raw_transaction.extend(chunk);
            if chunk_len < Self::LARGE_OBJECT_CHUNK {
                break;
            }
            chunk_num += 1;
        }
        Ok(stored)
    }
}
