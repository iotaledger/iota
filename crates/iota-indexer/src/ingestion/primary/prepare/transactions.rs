// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use async_trait::async_trait;
use iota_json_rpc::{ObjectProvider, get_balance_changes_from_effect, get_object_changes};
use iota_json_rpc_types::IotaTransactionKind;
use iota_sdk_types::{
    CheckpointSequenceNumber, CheckpointTimestamp, ObjectId, Owner, Transaction, TransactionDigest,
    TransactionEffects, TransactionEvents, Version,
};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    full_checkpoint_content::CheckpointTransaction,
    object::Object,
    transaction::TransactionAPI,
};
use itertools::Itertools;

use crate::{
    errors::IndexerError,
    ingestion::common::{orchestration::OperationalLevel, prepare::ValidatedCheckpoint},
    metrics::IndexerMetrics,
    types::{
        IndexedBalanceChange, IndexedObjectChange, IndexedTransaction, IndexerResult, TxIndex,
    },
};

/// The builder of all transaction data to commit to the database.
#[derive(Clone, Copy)]
pub(super) struct TransactionTransformer<'chk> {
    checkpoint: ValidatedCheckpoint<'chk>,
}

impl<'chk> TransactionTransformer<'chk> {
    pub(super) fn new(checkpoint: ValidatedCheckpoint<'chk>) -> Self {
        Self { checkpoint }
    }

    pub(super) async fn transform(
        self,
        metrics: &IndexerMetrics,
        operational_level: OperationalLevel,
    ) -> IndexerResult<TransactionData> {
        let mut transaction_data = TransactionData::new(operational_level);

        for (sequence_number, checkpoint_transaction) in self.checkpoint.enumerate_transactions() {
            let transaction = self
                .build_transaction(checkpoint_transaction, sequence_number, metrics)
                .await?;
            transaction_data.transactions.push(transaction);

            if let Some(indices) = &mut transaction_data.transaction_indices {
                indices.push(self.build_tx_index(checkpoint_transaction, sequence_number)?);
            }
        }
        Ok(transaction_data)
    }

    async fn build_transaction(
        self,
        tx: &CheckpointTransaction,
        tx_sequence_number: u64,
        metrics: &IndexerMetrics,
    ) -> IndexerResult<IndexedTransaction> {
        index_transaction(
            tx,
            tx_sequence_number,
            self.checkpoint.sequence_number(),
            self.checkpoint.timestamp_ms(),
            metrics,
        )
        .await
    }

    fn build_tx_index(
        self,
        tx: &CheckpointTransaction,
        sequence_number: u64,
    ) -> IndexerResult<TxIndex> {
        let inner_tx = tx.transaction.transaction();

        let input_objects = inner_tx
            .input_objects()
            .expect("committed txns have been validated")
            .into_iter()
            .map(|obj_kind| obj_kind.object_id())
            .collect::<Vec<_>>();

        let changed_objects = tx
            .effects
            .all_changed_objects()
            .into_iter()
            .map(|(changed, _write_kind)| changed.reference().object_id)
            .collect::<Vec<_>>();

        let wrapped_or_deleted_objects = tx
            .effects
            .all_tombstones()
            .into_iter()
            .map(|(object_id, _)| object_id)
            .chain(tx.effects.created_then_wrapped_objects())
            .collect::<Vec<_>>();

        let payers = vec![inner_tx.gas_owner()];

        let sender = inner_tx.sender();

        let recipients = tx
            .effects
            .all_changed_objects()
            .into_iter()
            .filter_map(|(changed, _write_kind)| match changed.owner() {
                Owner::Address(address) => Some(*address),
                _ => None,
            })
            .unique()
            .collect::<Vec<_>>();

        let move_calls = inner_tx
            .move_calls()
            .iter()
            .map(|(p, m, f)| (*<&ObjectId>::clone(p), m.to_string(), f.to_string()))
            .collect();

        Ok(TxIndex {
            tx_sequence_number: sequence_number,
            transaction_digest: *tx.transaction.digest(),
            checkpoint_sequence_number: self.checkpoint.sequence_number(),
            input_objects,
            changed_objects,
            sender,
            payers,
            recipients,
            move_calls,
            tx_kind: IotaTransactionKind::try_from(inner_tx.kind())?,
            wrapped_or_deleted_objects,
        })
    }
}

#[derive(Default)]
pub(super) struct TransactionData {
    pub(super) transactions: Vec<IndexedTransaction>,
    /// The indices of transaction events.
    ///
    /// [`Some`] only on the `FilteredQueries` operational level and above.
    pub(super) transaction_indices: Option<Vec<TxIndex>>,
}

impl TransactionData {
    /// Creates empty transaction data with the collections that the
    /// `operational_level` includes.
    fn new(operational_level: OperationalLevel) -> Self {
        Self {
            transactions: Default::default(),
            transaction_indices: operational_level
                .includes(OperationalLevel::FilteredQueries)
                .then(Default::default),
        }
    }
}
pub(crate) async fn index_transaction(
    tx: &CheckpointTransaction,
    tx_sequence_number: u64,
    checkpoint_sequence_number: CheckpointSequenceNumber,
    checkpoint_timestamp_ms: CheckpointTimestamp,
    metrics: &IndexerMetrics,
) -> IndexerResult<IndexedTransaction> {
    let tx_digest = tx.transaction.digest();

    let txn = tx.transaction.transaction();

    let events = tx
        .events
        .as_ref()
        .map(|TransactionEvents(events)| events.clone())
        .unwrap_or_default();

    let transaction_kind = IotaTransactionKind::try_from(txn.kind())?;

    let objects = tx
        .input_objects
        .iter()
        .chain(tx.output_objects.iter())
        .collect::<Vec<_>>();

    let (balance_change, object_changes) = InMemTxChanges::new(&objects, metrics)
        .get_changes(txn, &tx.effects, tx_digest)
        .await?;

    Ok(IndexedTransaction {
        tx_sequence_number,
        tx_digest: *tx_digest,
        checkpoint_sequence_number,
        timestamp_ms: checkpoint_timestamp_ms,
        sender_signed_data: tx.transaction.data().clone(),
        successful_tx_num: if tx.effects.status().is_success() {
            txn.kind().num_transactions() as u64
        } else {
            0
        },
        effects: tx.effects.clone(),
        object_changes,
        balance_change,
        events,
        transaction_kind,
    })
}

pub struct InMemObjectCache {
    id_map: HashMap<ObjectId, Object>,
    seq_map: HashMap<(ObjectId, Version), Object>,
}

impl InMemObjectCache {
    pub fn new() -> Self {
        Self {
            id_map: HashMap::new(),
            seq_map: HashMap::new(),
        }
    }

    pub fn insert_object(&mut self, obj: Object) {
        self.id_map.insert(obj.id(), obj.clone());
        self.seq_map.insert((obj.id(), obj.version()), obj);
    }

    pub fn get(&self, id: &ObjectId, version: Option<&Version>) -> Option<&Object> {
        if let Some(version) = version {
            self.seq_map.get(&(*id, *version))
        } else {
            self.id_map.get(id)
        }
    }
}

impl Default for InMemObjectCache {
    fn default() -> Self {
        Self::new()
    }
}

/// In memory object provider to use during ingestion of transaction.
///
/// Gets object/balance changes from transaction data.
pub struct InMemTxChanges<'m> {
    object_cache: InMemObjectCache,
    metrics: &'m IndexerMetrics,
}

impl<'m> InMemTxChanges<'m> {
    pub fn new(objects: &[&Object], metrics: &'m IndexerMetrics) -> Self {
        let mut object_cache = InMemObjectCache::new();
        for obj in objects {
            object_cache.insert_object(<&Object>::clone(obj).clone());
        }
        Self {
            object_cache,
            metrics,
        }
    }

    pub(crate) async fn get_changes(
        &self,
        tx: &Transaction,
        effects: &TransactionEffects,
        tx_digest: &TransactionDigest,
    ) -> IndexerResult<(Vec<IndexedBalanceChange>, Vec<IndexedObjectChange>)> {
        let _timer = self
            .metrics
            .indexing_tx_object_changes_latency
            .start_timer();
        let object_change: Vec<_> = get_object_changes(
            self,
            tx.sender(),
            effects.modified_at_versions(),
            effects.all_changed_objects(),
            effects.all_removed_objects(),
        )
        .await?
        .into_iter()
        .map(IndexedObjectChange::from)
        .collect();
        let balance_change = get_balance_changes_from_effect(
            self,
            effects,
            tx.input_objects().unwrap_or_else(|e| {
                panic!("checkpointed tx {tx_digest} has invalid input objects: {e}")
            }),
            None,
        )
        .await?
        .into_iter()
        .map(IndexedBalanceChange::from)
        .collect();
        Ok((balance_change, object_change))
    }
}

#[async_trait]
impl ObjectProvider for InMemTxChanges<'_> {
    type Error = IndexerError;

    async fn get_object(&self, id: &ObjectId, version: &Version) -> Result<Object, Self::Error> {
        let object = self
            .object_cache
            .get(id, Some(version))
            .as_ref()
            .map(|o| <&Object>::clone(o).clone());
        if let Some(o) = object {
            self.metrics.indexing_get_object_in_mem_hit.inc();
            return Ok(o);
        }

        panic!(
            "object {id} is not found in TxChangesProcessor as an ObjectProvider (fn get_object)"
        );
    }

    async fn find_object_lt_or_eq_version(
        &self,
        id: &ObjectId,
        version: &Version,
    ) -> Result<Option<Object>, Self::Error> {
        // First look up the exact version in object_cache.
        let object = self
            .object_cache
            .get(id, Some(version))
            .as_ref()
            .map(|o| <&Object>::clone(o).clone());
        if let Some(o) = object {
            self.metrics.indexing_get_object_in_mem_hit.inc();
            return Ok(Some(o));
        }

        // Second look up the latest version in object_cache. This may be
        // called when the object is deleted hence the version at deletion
        // is given.
        let object = self
            .object_cache
            .get(id, None)
            .as_ref()
            .map(|o| <&Object>::clone(o).clone());
        if let Some(o) = object {
            if o.version() > *version {
                panic!(
                    "found a higher version {} for object {id}, expected lt_or_eq {version}",
                    o.version(),
                );
            }
            if o.version() <= *version {
                self.metrics.indexing_get_object_in_mem_hit.inc();
                return Ok(Some(o));
            }
        }

        panic!(
            "object {id} is not found in TxChangesProcessor as an ObjectProvider (fn find_object_lt_or_eq_version)"
        );
    }
}
