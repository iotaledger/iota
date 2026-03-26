// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use diesel::prelude::*;
use iota_json_rpc_types::{
    BalanceChange, IotaEvent, IotaTransactionBlock, IotaTransactionBlockEffects,
    IotaTransactionBlockEvents, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    ObjectChange,
};
use iota_package_resolver::{PackageStore, Resolver};
use iota_types::{
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEvents},
    event::Event,
    transaction::SenderSignedData,
};
use move_core_types::{
    annotated_value::{MoveDatatypeLayout, MoveTypeLayout},
    language_storage::TypeTag,
};
#[cfg(feature = "shared_test_runtime")]
use serde::Deserialize;

use crate::{
    errors::IndexerError,
    schema::{optimistic_transactions, transactions, tx_global_order},
    types::{IndexedObjectChange, IndexedTransaction, IndexerResult},
};

#[derive(Clone, Debug, Queryable, Insertable, QueryableByName, Selectable)]
#[diesel(table_name = tx_global_order)]
pub struct TxGlobalOrder {
    /// Sequence number of transaction according to checkpoint ordering.
    /// Set after transaction is checkpoint-indexed.
    pub chk_tx_sequence_number: Option<i64>,
    /// Number that represents the global ordering between optimistic and
    /// checkpointed transactions.
    ///
    /// Optimistic transactions will share the same number as checkpointed
    /// transactions. In this case, ties are resolved by the
    /// `(global_sequence_number, optimistic_sequence_number)` pair that
    /// guarantees deterministic ordering.
    pub global_sequence_number: i64,
    pub tx_digest: Vec<u8>,
    /// Monotonically increasing number that represents the order
    /// of execution of optimistic transactions.
    ///
    /// Checkpointed transactions use [`CHECKPOINT_TX_OPTIMISTIC_SEQ`] (-1).
    /// Optimistic transactions should set this value to `None`,
    /// so that it is auto-generated on the database.
    #[diesel(deserialize_as = i64)]
    pub optimistic_sequence_number: Option<i64>,
}

/// Value stored in `optimistic_sequence_number` for checkpointed
/// transactions, to distinguish them from optimistic transactions
/// which use positive auto-generated values.
pub const CHECKPOINT_TX_OPTIMISTIC_SEQ: i64 = -1;

impl From<&IndexedTransaction> for TxGlobalOrder {
    fn from(tx: &IndexedTransaction) -> Self {
        Self {
            chk_tx_sequence_number: Some(tx.tx_sequence_number as i64),
            global_sequence_number: tx.tx_sequence_number as i64,
            tx_digest: tx.tx_digest.into_inner().to_vec(),
            optimistic_sequence_number: Some(CHECKPOINT_TX_OPTIMISTIC_SEQ),
        }
    }
}

#[derive(Clone, Debug, Queryable, Insertable, QueryableByName, Selectable)]
#[diesel(table_name = transactions)]
#[cfg_attr(feature = "shared_test_runtime", derive(Deserialize))]
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

#[derive(Clone, Debug, Queryable, Insertable, QueryableByName, Selectable)]
#[diesel(table_name = optimistic_transactions)]
pub struct OptimisticTransaction {
    pub global_sequence_number: i64,
    pub optimistic_sequence_number: i64,
    pub transaction_digest: Vec<u8>,
    pub raw_transaction: Vec<u8>,
    pub raw_effects: Vec<u8>,
    pub object_changes: Vec<Option<Vec<u8>>>,
    pub balance_changes: Vec<Option<Vec<u8>>>,
    pub events: Vec<Option<Vec<u8>>>,
    pub transaction_kind: i16,
    pub success_command_count: i16,
}

impl From<OptimisticTransaction> for StoredTransaction {
    fn from(tx: OptimisticTransaction) -> Self {
        StoredTransaction {
            tx_sequence_number: tx.optimistic_sequence_number,
            transaction_digest: tx.transaction_digest,
            raw_transaction: tx.raw_transaction,
            raw_effects: tx.raw_effects,
            checkpoint_sequence_number: -1,
            timestamp_ms: -1,
            object_changes: tx.object_changes,
            balance_changes: tx.balance_changes,
            events: tx.events,
            transaction_kind: tx.transaction_kind,
            success_command_count: tx.success_command_count,
        }
    }
}

impl OptimisticTransaction {
    pub fn from_stored(global_sequence_number: i64, stored: StoredTransaction) -> Self {
        OptimisticTransaction {
            global_sequence_number,
            optimistic_sequence_number: stored.tx_sequence_number,
            transaction_digest: stored.transaction_digest,
            raw_transaction: stored.raw_transaction,
            raw_effects: stored.raw_effects,
            object_changes: stored.object_changes,
            balance_changes: stored.balance_changes,
            events: stored.events,
            transaction_kind: stored.transaction_kind,
            success_command_count: stored.success_command_count,
        }
    }

    pub fn get_balance_len(&self) -> usize {
        self.balance_changes.len()
    }

    pub fn get_balance_at_idx(&self, idx: usize) -> Option<Vec<u8>> {
        self.balance_changes.get(idx).cloned().flatten()
    }

    pub fn get_object_len(&self) -> usize {
        self.object_changes.len()
    }

    pub fn get_object_at_idx(&self, idx: usize) -> Option<Vec<u8>> {
        self.object_changes.get(idx).cloned().flatten()
    }

    pub fn get_event_len(&self) -> usize {
        self.events.len()
    }

    pub fn get_event_at_idx(&self, idx: usize) -> Option<Vec<u8>> {
        self.events.get(idx).cloned().flatten()
    }
}

pub type StoredTransactionEvents = Vec<Option<Vec<u8>>>;

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
            transaction_kind: tx.transaction_kind as i16,
            success_command_count: tx.successful_tx_num as i16,
        }
    }
}

impl StoredTransaction {
    pub fn get_balance_len(&self) -> usize {
        self.balance_changes.len()
    }

    pub fn get_balance_at_idx(&self, idx: usize) -> Option<Vec<u8>> {
        self.balance_changes.get(idx).cloned().flatten()
    }

    pub fn get_object_len(&self) -> usize {
        self.object_changes.len()
    }

    pub fn get_object_at_idx(&self, idx: usize) -> Option<Vec<u8>> {
        self.object_changes.get(idx).cloned().flatten()
    }

    pub fn get_event_len(&self) -> usize {
        self.events.len()
    }

    pub fn get_event_at_idx(&self, idx: usize) -> Option<Vec<u8>> {
        self.events.get(idx).cloned().flatten()
    }

    /// True for checkpointed transactions, False for optimistically indexed
    /// transactions
    pub fn is_checkpointed_transaction(&self) -> bool {
        self.checkpoint_sequence_number >= 0
    }

    pub async fn try_into_iota_transaction_block_response(
        self,
        options: IotaTransactionBlockResponseOptions,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<IotaTransactionBlockResponse> {
        let options = options.clone();
        let tx_digest =
            TransactionDigest::try_from(self.transaction_digest.as_slice()).map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Can't convert {:?} as tx_digest. Error: {e}",
                    self.transaction_digest
                ))
            })?;

        let timestamp_ms = self
            .is_checkpointed_transaction()
            .then_some(self.timestamp_ms as u64);
        let checkpoint = self
            .is_checkpointed_transaction()
            .then_some(self.checkpoint_sequence_number as u64);

        let transaction = if options.show_input {
            let sender_signed_data = self.try_into_sender_signed_data()?;
            let tx_block = IotaTransactionBlock::try_from_with_package_resolver(
                sender_signed_data,
                package_resolver,
                tx_digest,
            )
            .await?;
            Some(tx_block)
        } else {
            None
        };

        let effects = if options.show_effects {
            Some(
                self.try_into_iota_transaction_effects(package_resolver)
                    .await?,
            )
        } else {
            None
        };

        let raw_transaction = if options.show_raw_input {
            self.raw_transaction
        } else {
            Default::default()
        };

        let events = if options.show_events {
            let events = {
                self
                        .events
                        .into_iter()
                        .map(|event| match event {
                            Some(event) => {
                                let event: Event = bcs::from_bytes(&event).map_err(|e| {
                                    IndexerError::PersistentStorageDataCorruption(format!(
                                        "Can't convert event bytes into Event. tx_digest={tx_digest:?} Error: {e}"
                                    ))
                                })?;
                                Ok(event)
                            }
                            None => Err(IndexerError::PersistentStorageDataCorruption(format!(
                                "Event should not be null, tx_digest={tx_digest:?}"
                            ))),
                        })
                        .collect::<Result<Vec<Event>, IndexerError>>()?
            };
            let tx_events = TransactionEvents { data: events };

            Some(
                tx_events_to_iota_tx_events(tx_events, package_resolver, tx_digest, timestamp_ms)
                    .await?,
            )
        } else {
            None
        };

        let object_changes = if options.show_object_changes {
            let object_changes = {
                self.object_changes.into_iter().map(|object_change| {
                        match object_change {
                            Some(object_change) => {
                                let object_change: IndexedObjectChange = bcs::from_bytes(&object_change)
                                    .map_err(|e| IndexerError::PersistentStorageDataCorruption(
                                        format!("Can't convert object_change bytes into IndexedObjectChange. tx_digest={tx_digest:?} Error: {e}")
                                    ))?;
                                Ok(ObjectChange::from(object_change))
                            }
                            None => Err(IndexerError::PersistentStorageDataCorruption(format!("object_change should not be null, tx_digest={tx_digest:?}"))),
                        }
                    }).collect::<Result<Vec<ObjectChange>, IndexerError>>()?
            };
            Some(object_changes)
        } else {
            None
        };

        let balance_changes = if options.show_balance_changes {
            let balance_changes = {
                self.balance_changes.into_iter().map(|balance_change| {
                        match balance_change {
                            Some(balance_change) => {
                                let balance_change: BalanceChange = bcs::from_bytes(&balance_change)
                                    .map_err(|e| IndexerError::PersistentStorageDataCorruption(
                                        format!("Can't convert balance_change bytes into BalanceChange. tx_digest={tx_digest:?} Error: {e}")
                                    ))?;
                                Ok(balance_change)
                            }
                            None => Err(IndexerError::PersistentStorageDataCorruption(format!("object_change should not be null, tx_digest={tx_digest:?}"))),
                        }
                    }).collect::<Result<Vec<BalanceChange>, IndexerError>>()?
            };
            Some(balance_changes)
        } else {
            None
        };

        let raw_effects = if options.show_raw_effects {
            self.raw_effects
        } else {
            Default::default()
        };

        Ok(IotaTransactionBlockResponse {
            digest: tx_digest,
            transaction,
            raw_transaction,
            effects,
            events,
            object_changes,
            balance_changes,
            timestamp_ms,
            checkpoint,
            confirmed_local_execution: None,
            errors: vec![],
            raw_effects,
        })
    }

    pub fn try_into_sender_signed_data(&self) -> IndexerResult<SenderSignedData> {
        let sender_signed_data: SenderSignedData =
            bcs::from_bytes(&self.raw_transaction).map_err(|e| {
                IndexerError::PersistentStorageDataCorruption(format!(
                    "Can't convert raw_transaction of {} into SenderSignedData. Error: {e}",
                    self.tx_sequence_number
                ))
            })?;
        Ok(sender_signed_data)
    }

    pub async fn try_into_iota_transaction_effects(
        &self,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<IotaTransactionBlockEffects> {
        let effects: TransactionEffects = bcs::from_bytes(&self.raw_effects).map_err(|e| {
            IndexerError::PersistentStorageDataCorruption(format!(
                "Can't convert raw_effects of {} into TransactionEffects. Error: {e}",
                self.tx_sequence_number
            ))
        })?;
        let effects =
            IotaTransactionBlockEffects::from_native_with_clever_error(effects, package_resolver)
                .await;
        Ok(effects)
    }

    /// Check if this is the genesis transaction relying on the global ordering.
    pub fn is_genesis(&self) -> bool {
        self.tx_sequence_number == 0
    }
}

pub fn stored_events_to_events(
    stored_events: StoredTransactionEvents,
) -> Result<Vec<Event>, IndexerError> {
    stored_events
        .into_iter()
        .map(|event| match event {
            Some(event) => {
                let event: Event = bcs::from_bytes(&event).map_err(|e| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "Can't convert event bytes into Event. Error: {e}",
                    ))
                })?;
                Ok(event)
            }
            None => Err(IndexerError::PersistentStorageDataCorruption(
                "Event should not be null".to_string(),
            )),
        })
        .collect::<Result<Vec<Event>, IndexerError>>()
}

pub async fn tx_events_to_iota_tx_events(
    tx_events: TransactionEvents,
    package_resolver: &Arc<Resolver<impl PackageStore>>,
    tx_digest: TransactionDigest,
    timestamp: Option<u64>,
) -> Result<IotaTransactionBlockEvents, IndexerError> {
    let mut iota_event_futures = vec![];
    let tx_events_data_len = tx_events.data.len();
    for tx_event in tx_events.data.clone() {
        let package_resolver_clone = package_resolver.clone();
        iota_event_futures.push(tokio::task::spawn(async move {
            let resolver = package_resolver_clone;
            resolver
                .type_layout(TypeTag::Struct(Box::new(tx_event.type_.clone())))
                .await
        }));
    }
    let event_move_type_layouts = futures::future::join_all(iota_event_futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            IndexerError::ResolveMoveStruct(format!(
                "Failed to convert to iota event with Error: {e}",
            ))
        })?;
    let event_move_datatype_layouts = event_move_type_layouts
        .into_iter()
        .filter_map(|move_type_layout| match move_type_layout {
            MoveTypeLayout::Struct(s) => Some(MoveDatatypeLayout::Struct(s)),
            MoveTypeLayout::Enum(e) => Some(MoveDatatypeLayout::Enum(e)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(tx_events_data_len == event_move_datatype_layouts.len());
    let iota_events = tx_events
        .data
        .into_iter()
        .enumerate()
        .zip(event_move_datatype_layouts)
        .map(|((seq, tx_event), move_datatype_layout)| {
            IotaEvent::try_from(
                tx_event,
                tx_digest,
                seq as u64,
                timestamp,
                move_datatype_layout,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let iota_tx_events = IotaTransactionBlockEvents { data: iota_events };
    Ok(iota_tx_events)
}
