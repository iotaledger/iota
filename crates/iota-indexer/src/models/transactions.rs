// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
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
    errors::IndexerError,
    models::large_objects::{get_large_object, put_large_object},
    schema::transactions,
    types::{IndexedObjectChange, IndexedTransaction, IndexerResult},
};

#[derive(Clone, Debug, Queryable, Insertable, QueryableByName)]
#[diesel(table_name = transactions)]
pub struct StoredTransaction {
    /// The index of the transaction in the global ordering that starts
    /// from genesis.
    pub tx_sequence_number: i64,
    pub transaction_digest: Vec<u8>,
    pub raw_transaction: Vec<u8>,
    pub raw_effects: Vec<u8>,
    pub checkpoint_sequence_number: i64,
    pub timestamp_ms: i64,
    pub object_changes: Vec<Option<Vec<u8>>>,
    pub balance_changes: Vec<Option<Vec<u8>>>,
    pub events: Vec<Option<Vec<u8>>>,
    pub transaction_kind: i16,
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
            raw_transaction: bcs::to_bytes(&tx.sender_signed_data).unwrap(),
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

    /// Check if this is the genesis transaction relying on the global ordering.
    pub fn is_genesis(&self) -> bool {
        self.tx_sequence_number == 0
    }

    /// Store the raw transaction data as a large object in postgres
    /// and replace `self.raw_transaction` with a pointer to the large object.
    fn store_inner_data_as_large_object(
        mut self,
        pool: &PgConnectionPool,
    ) -> Result<Self, IndexerError> {
        let raw_tx = std::mem::take(&mut self.raw_transaction);
        let oid = put_large_object(raw_tx, Self::LARGE_OBJECT_CHUNK, pool)?;
        self.raw_transaction = oid.to_le_bytes().to_vec();
        Ok(self)
    }

    /// This method checks if this is the genesis transaction,
    /// and if true it stores the raw data as a large object
    /// in postgres, replacing `self.raw_transaction` with
    /// a pointer to the large object data.
    pub fn store_inner_genesis_data_as_large_object(
        self,
        pool: &PgConnectionPool,
    ) -> Result<Self, IndexerError> {
        if !self.is_genesis() {
            return Ok(self);
        }
        self.store_inner_data_as_large_object(pool)
    }

    fn set_large_object_as_inner_data(
        mut self,
        pool: &PgConnectionPool,
    ) -> Result<Self, IndexerError> {
        let raw_oid = std::mem::take(&mut self.raw_transaction);
        let raw_oid: [u8; 4] = raw_oid.try_into().map_err(|_| {
            IndexerError::GenericError("invalid large object identifier".to_owned())
        })?;
        let oid = u32::from_le_bytes(raw_oid);

        self.raw_transaction = get_large_object(oid, Self::LARGE_OBJECT_CHUNK, pool)?;

        Ok(self)
    }

    /// The genesis transactions uses a pointer to the large object
    /// as inner data.
    ///
    /// This replaces the pointer with the actual large object data.
    pub fn set_genesis_large_object_as_inner_data(
        self,
        pool: &PgConnectionPool,
    ) -> Result<Self, IndexerError> {
        if !self.is_genesis() {
            return Ok(self);
        }
        self.set_large_object_as_inner_data(pool)
    }
}
