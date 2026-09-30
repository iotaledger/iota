// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_json_rpc::{ObjectProvider, get_balance_changes_from_effect, get_object_changes};
use iota_json_rpc_types::IotaTransactionKind;
use iota_sdk_types::{
    CheckpointSequenceNumber, CheckpointTimestamp, Event, ObjectId, Owner, Transaction,
    TransactionDigest, TransactionEffects, TransactionEvents, Version,
};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    iota_system_state::{IotaSystemStateTrait, get_iota_system_state},
    object::Object,
    transaction::TransactionAPI,
};
use itertools::Itertools;
use tracing::{info, warn};

use crate::{
    db::ConnectionPool,
    errors::IndexerError,
    ingestion::{
        common::prepare::{CheckpointObjectChanges, ValidatedCheckpoint, extract_df_kind},
        primary::persist::{CheckpointDataToCommit, EpochToCommit},
    },
    metrics::IndexerMetrics,
    models::{
        display::{
            StoredDisplay, display_id_from_created_event, displayed_type_from_created_event,
        },
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate, extract_epoch_info_event},
        obj_indices::StoredObjectVersion,
        objects::StoredBackwardHistoryObject,
    },
    store::{IndexerStore, PgIndexerStore},
    types::{
        EventIndex, IndexedBalanceChange, IndexedCheckpoint, IndexedEvent, IndexedObject,
        IndexedObjectChange, IndexedPackage, IndexedTransaction, IndexerResult, ObjectStatus,
        TxIndex,
    },
};

pub struct PrimaryWorker {
    metrics: IndexerMetrics,
    indexed_checkpoint_sender: iota_metrics::metered_channel::Sender<CheckpointDataToCommit>,
}

#[async_trait]
impl Worker for PrimaryWorker {
    type Message = ();
    type Error = IndexerError;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        self.metrics
            .latest_fullnode_checkpoint_sequence_number
            .set(checkpoint.checkpoint_summary.sequence_number as i64);
        let time_now_ms = chrono::Utc::now().timestamp_millis();
        let cp_download_lag = time_now_ms - checkpoint.checkpoint_summary.timestamp_ms as i64;
        info!(
            "checkpoint download lag for cp {}: {} ms",
            checkpoint.checkpoint_summary.sequence_number, cp_download_lag
        );
        self.metrics.download_lag_ms.set(cp_download_lag);
        self.metrics
            .max_downloaded_checkpoint_sequence_number
            .set(checkpoint.checkpoint_summary.sequence_number as i64);
        self.metrics
            .downloaded_checkpoint_timestamp_ms
            .set(checkpoint.checkpoint_summary.timestamp_ms as i64);
        info!(
            "Indexer lag: downloaded checkpoint {} with time now {} and checkpoint time {}",
            checkpoint.checkpoint_summary.sequence_number,
            time_now_ms,
            checkpoint.checkpoint_summary.timestamp_ms
        );

        let validated_checkpoint = ValidatedCheckpoint::new(&checkpoint)?;
        let transformer = Transformer::new(validated_checkpoint, &self.metrics);
        self.indexed_checkpoint_sender
            .send(transformer.transform().await?)
            .await
            .map_err(|_| {
                IndexerError::MpscChannel(
                    "failed to send checkpoint data, receiver half closed".into(),
                )
            })?;
        Ok(())
    }
}

impl PrimaryWorker {
    pub(crate) fn new(
        metrics: IndexerMetrics,
        indexed_checkpoint_sender: iota_metrics::metered_channel::Sender<CheckpointDataToCommit>,
    ) -> Self {
        Self {
            metrics,
            indexed_checkpoint_sender,
        }
    }

    pub(crate) fn pg_blocking_cp(state: PgIndexerStore) -> Result<ConnectionPool, IndexerError> {
        let state_as_any = state.as_any();
        if let Some(pg_state) = state_as_any.downcast_ref::<PgIndexerStore>() {
            return Ok(pg_state.blocking_cp());
        }
        Err(IndexerError::Uncategorized(anyhow::anyhow!(
            "failed to downcast state to PgIndexerStore"
        )))
    }
}

struct Transformer<'chk, 'm> {
    checkpoint: ValidatedCheckpoint<'chk>,
    metrics: &'m IndexerMetrics,
    events: Vec<IndexedEvent>,
    event_indices: Vec<EventIndex>,
    displays: BTreeMap<String, StoredDisplay>,
}

impl<'chk, 'm> Transformer<'chk, 'm> {
    fn new(checkpoint: ValidatedCheckpoint<'chk>, metrics: &'m IndexerMetrics) -> Self {
        Self {
            checkpoint,
            metrics,
            events: Default::default(),
            event_indices: Default::default(),
            displays: Default::default(),
        }
    }

    async fn transform(mut self) -> IndexerResult<CheckpointDataToCommit> {
        info!(
            checkpoint_seq = self.checkpoint.sequence_number(),
            "Indexing checkpoint data blob"
        );

        let transaction_data = TransactionTransformer::new(self.checkpoint)
            .transform(self.metrics)
            .await?;

        for (sequence_number, checkpoint_transaction) in self.checkpoint.enumerate_transactions() {
            self.extend_event_data(checkpoint_transaction, sequence_number);
        }

        let object_data = ObjectsTransformer::new(self.checkpoint).transform(self.metrics);

        let epoch = self.build_epoch()?;

        let total_successful_transactions: u64 = transaction_data
            .transactions
            .iter()
            .map(|tx| tx.successful_tx_num)
            .sum();
        let checkpoint = self.build_checkpoint(total_successful_transactions as usize);

        let time_now_ms = chrono::Utc::now().timestamp_millis();
        self.metrics
            .index_lag_ms
            .set(time_now_ms - checkpoint.timestamp_ms as i64);
        self.metrics
            .max_indexed_checkpoint_sequence_number
            .set(checkpoint.sequence_number as i64);
        self.metrics
            .indexed_checkpoint_timestamp_ms
            .set(checkpoint.timestamp_ms as i64);
        info!(
            "Indexer lag: indexed checkpoint {} with time now {} and checkpoint time {}",
            checkpoint.sequence_number, time_now_ms, checkpoint.timestamp_ms
        );

        Ok(CheckpointDataToCommit {
            checkpoint,
            transactions: transaction_data.transactions,
            events: self.events,
            event_indices: self.event_indices,
            tx_indices: transaction_data.transaction_indices,
            displays: self.displays,
            object_changes: object_data.checkpoint_objects,
            backward_history_changes: object_data.history_objects,
            object_versions: object_data.object_versions,
            packages: object_data.packages,
            epoch,
        })
    }

    fn build_checkpoint(&self, total_successful_transactions: usize) -> IndexedCheckpoint {
        let CheckpointData {
            checkpoint_summary,
            checkpoint_contents,
            ..
        } = self.checkpoint.data();

        IndexedCheckpoint::from_iota_checkpoint(
            checkpoint_summary,
            checkpoint_contents,
            total_successful_transactions,
        )
    }

    fn build_epoch(&self) -> IndexerResult<Option<EpochToCommit>> {
        let checkpoint_object_store = EpochEndIndexingObjectStore::new(self.checkpoint.data());

        let CheckpointData {
            transactions,
            checkpoint_summary,
            checkpoint_contents: _,
        } = self.checkpoint.data();

        // Genesis epoch
        if checkpoint_summary.sequence_number() == 0 {
            info!("Processing genesis epoch");
            let system_state =
                get_iota_system_state(&checkpoint_object_store)?.into_iota_system_state_summary();
            return Ok(Some(EpochToCommit {
                last_epoch: None,
                new_epoch: StartOfEpochUpdate::new(
                    &system_state,
                    0, // first_checkpoint_id
                    0, // first_tx_sequence_number
                    None,
                ),
            }));
        }

        // If not end of epoch, return
        if checkpoint_summary.end_of_epoch_data.is_none() {
            return Ok(None);
        }

        let event = transactions
            .iter()
            .find_map(|t| t.events.as_ref().and_then(extract_epoch_info_event));

        let system_state = get_iota_system_state(&checkpoint_object_store)?;
        if event.is_none() {
            warn!(
                "no SystemEpochInfoEvent found at end of epoch {}, some epoch data will be set to default.",
                checkpoint_summary.epoch,
            );
            assert!(
                system_state.safe_mode(),
                "iota is not in safe mode but no SystemEpochInfoEvent found at end of epoch {}",
                checkpoint_summary.epoch
            );
        }

        let event = event.unwrap_or_default();
        let new_epoch_first_checkpoint_id = checkpoint_summary.sequence_number + 1;
        let new_epoch_first_tx_sequence_number = checkpoint_summary.network_total_transactions;
        Ok(Some(EpochToCommit {
            last_epoch: Some(EndOfEpochUpdate::new(checkpoint_summary, &event)),
            new_epoch: StartOfEpochUpdate::new(
                &system_state.into_iota_system_state_summary(),
                new_epoch_first_checkpoint_id,
                new_epoch_first_tx_sequence_number,
                Some(&event),
            ),
        }))
    }

    fn extend_event_data(&mut self, transaction: &CheckpointTransaction, sequence_number: u64) {
        let transformer = EventsTransformer::new(
            transaction,
            sequence_number,
            self.checkpoint.sequence_number(),
            self.checkpoint.timestamp_ms(),
        );
        let event_data = transformer.transform();
        self.displays.extend(event_data.displays);
        self.events.extend(event_data.events);
        self.event_indices.extend(event_data.event_indices);
    }
}

/// The builder of all transaction data to commit to the database.
#[derive(Clone, Copy)]
struct TransactionTransformer<'chk> {
    checkpoint: ValidatedCheckpoint<'chk>,
}

impl<'chk> TransactionTransformer<'chk> {
    fn new(checkpoint: ValidatedCheckpoint<'chk>) -> Self {
        Self { checkpoint }
    }

    async fn transform(self, metrics: &IndexerMetrics) -> IndexerResult<TransactionData> {
        let mut transaction_data = TransactionData::default();
        for (sequence_number, checkpoint_transaction) in self.checkpoint.enumerate_transactions() {
            let transaction = self
                .build_transaction(checkpoint_transaction, sequence_number, metrics)
                .await?;
            transaction_data.transactions.push(transaction);
            let transaction_index = self.build_tx_index(checkpoint_transaction, sequence_number);
            transaction_data.transaction_indices.push(transaction_index);
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
            metrics.clone(),
        )
        .await
    }

    fn build_tx_index(self, tx: &CheckpointTransaction, sequence_number: u64) -> TxIndex {
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

        TxIndex {
            tx_sequence_number: sequence_number,
            transaction_digest: *tx.transaction.digest(),
            checkpoint_sequence_number: self.checkpoint.sequence_number(),
            input_objects,
            changed_objects,
            sender,
            payers,
            recipients,
            move_calls,
            tx_kind: IotaTransactionKind::from(inner_tx.kind()),
            wrapped_or_deleted_objects,
        }
    }
}

#[derive(Default)]
struct TransactionData {
    transactions: Vec<IndexedTransaction>,
    transaction_indices: Vec<TxIndex>,
}

pub(crate) async fn index_transaction(
    tx: &CheckpointTransaction,
    tx_sequence_number: u64,
    checkpoint_sequence_number: CheckpointSequenceNumber,
    checkpoint_timestamp_ms: CheckpointTimestamp,
    metrics: IndexerMetrics,
) -> IndexerResult<IndexedTransaction> {
    let tx_digest = tx.transaction.digest();

    let txn = tx.transaction.transaction();

    let events = tx
        .events
        .as_ref()
        .map(|TransactionEvents(events)| events.clone())
        .unwrap_or_default();

    let transaction_kind = IotaTransactionKind::from(txn.kind());

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

#[derive(Debug)]
pub(crate) struct EventsTransformer<'tx> {
    transaction: &'tx CheckpointTransaction,
    tx_sequence_number: u64,
    checkpoint_sequence_number: CheckpointSequenceNumber,
    checkpoint_timestamp_ms: CheckpointTimestamp,
}

impl<'tx> EventsTransformer<'tx> {
    pub(crate) fn new(
        transaction: &'tx CheckpointTransaction,
        tx_sequence_number: u64,
        checkpoint_sequence_number: CheckpointSequenceNumber,
        checkpoint_timestamp_ms: CheckpointTimestamp,
    ) -> Self {
        Self {
            transaction,
            tx_sequence_number,
            checkpoint_sequence_number,
            checkpoint_timestamp_ms,
        }
    }

    pub(crate) fn transform(self) -> EventData {
        let mut derived_data = EventData::default();
        let Some(events) = self.transaction.events.as_ref() else {
            return derived_data;
        };
        for (event_sequence_number, chain_event) in events.iter().enumerate() {
            if let Some((display_type, display)) = Self::build_display(chain_event) {
                derived_data.displays.insert(display_type, display);
            }
            let event = self.build_event(chain_event, event_sequence_number as u64);
            derived_data.events.push(event);
            let event_index = self.build_event_index(chain_event, event_sequence_number as u64);
            derived_data.event_indices.push(event_index);
        }
        // complement any displays created without emitting a DisplayUpdatedEvent
        let display_created_events = events.iter().filter_map(|event| {
            displayed_type_from_created_event(event).map(|display_type| (display_type, event))
        });
        for (display_type, display_created_event) in display_created_events {
            if derived_data.displays.contains_key(&display_type) {
                // display is already indexed through a DisplayUpdatedEvent
                continue;
            }
            let Some(display_id) = display_id_from_created_event(display_created_event) else {
                continue;
            };
            if let Some(display) = self.build_display_from_objects(display_id) {
                derived_data.displays.insert(display_type, display);
            }
        }
        derived_data
    }

    fn build_display(event: &Event) -> Option<(String, StoredDisplay)> {
        StoredDisplay::try_from_event(event).map(|display| (display.object_type.clone(), display))
    }

    fn build_event(&self, event: &Event, event_sequence_number: u64) -> IndexedEvent {
        IndexedEvent::from_event(
            self.tx_sequence_number,
            event_sequence_number,
            self.checkpoint_sequence_number,
            *self.transaction.transaction.digest(),
            event,
            self.checkpoint_timestamp_ms,
        )
    }

    fn build_event_index(&self, event: &Event, event_sequence_number: u64) -> EventIndex {
        EventIndex::from_event(self.tx_sequence_number, event_sequence_number, event)
    }

    fn build_display_from_objects(&self, display_id: ObjectId) -> Option<StoredDisplay> {
        self.transaction
            .output_objects
            .iter()
            .find(|object| object.id() == display_id)
            .and_then(StoredDisplay::try_from_object)
    }
}

#[derive(Debug, Default)]
pub(crate) struct EventData {
    pub(crate) displays: BTreeMap<String, StoredDisplay>,
    pub(crate) events: Vec<IndexedEvent>,
    pub(crate) event_indices: Vec<EventIndex>,
}

#[derive(Debug, Clone, Copy)]
struct ObjectsTransformer<'chk> {
    checkpoint: ValidatedCheckpoint<'chk>,
}

impl<'chk> ObjectsTransformer<'chk> {
    fn new(checkpoint: ValidatedCheckpoint<'chk>) -> Self {
        Self { checkpoint }
    }

    fn transform(self, metrics: &IndexerMetrics) -> ObjectData {
        let checkpoint_objects = {
            let _timer = metrics.indexing_objects_latency.start_timer();
            self.checkpoint.into()
        };

        ObjectData {
            checkpoint_objects,
            object_versions: self.build_object_versions(),
            packages: self.build_packages(metrics),
            history_objects: self.build_history_objects(),
        }
    }

    fn build_packages(self, metrics: &IndexerMetrics) -> Vec<IndexedPackage> {
        let _timer = metrics.indexing_packages_latency.start_timer();
        let checkpoint_sequence_number = self.checkpoint.sequence_number();
        self.checkpoint
            .iter_transactions()
            .flat_map(|tx| &tx.output_objects)
            .filter_map(|object| {
                let iota_sdk_types::ObjectData::Package(package) = object.data() else {
                    return None;
                };
                Some(IndexedPackage::new(
                    package.clone(),
                    checkpoint_sequence_number,
                ))
            })
            .collect()
    }

    /// Builds backward history entries for a checkpoint.
    ///
    /// For each transaction, records the *previous* state of every object that
    /// was superseded, so that a consistent view at an earlier checkpoint can
    /// be reconstructed by applying backward diffs to the current
    /// `checkpointed_objects` snapshot.
    ///
    /// The logic is split into three categories:
    ///
    /// 1. **Input objects that were mutated or removed** (mutate, delete,
    ///    wrap): these had an active prior state available in `input_objects` →
    ///    `ACTIVE` entries with full data.
    ///
    /// 2. **Created objects**: did not exist before → `NOT_YET_CREATED`.
    ///
    /// 3. **Unwrapped / unwrapped-then-deleted objects**: were previously
    ///    wrapped so no prior data is available → `WRAPPED_OR_DELETED` with a
    ///    lamport version approximation.
    fn build_history_objects(self) -> Vec<StoredBackwardHistoryObject> {
        let checkpoint_seq = self.checkpoint.sequence_number() as i64;
        let mut history_objects = Vec::new();

        for tx in self.checkpoint.iter_transactions() {
            let effects = &tx.effects;

            // 1. Input objects that were mutated or removed (deleted/wrapped) had an active
            //    prior state — record it from input_objects. Collect the affected IDs so we
            //    can iterate input_objects once.
            let superseded_ids: HashSet<ObjectId> = effects
                .mutated()
                .into_iter()
                .map(|mutated| mutated.reference().object_id)
                .chain(
                    effects
                        .all_removed_objects()
                        .into_iter()
                        .map(|(r, _)| r.object_id),
                )
                .collect();

            for input_obj in &tx.input_objects {
                if superseded_ids.contains(&input_obj.id()) {
                    let df_kind = extract_df_kind(input_obj);
                    let indexed = IndexedObject::from_object(
                        Some(checkpoint_seq as u64),
                        input_obj.clone(),
                        df_kind,
                    );
                    history_objects.push(StoredBackwardHistoryObject::try_from(indexed).expect(
                        "backward history conversion should not fail for active input objects",
                    ));
                }
            }

            // 2. Created objects did not exist before this transaction. Use lamport version
            //    - 1 so the version is monotonic with other backward-history rows for the
            //    same object.
            for created in effects.created() {
                history_objects.push(StoredBackwardHistoryObject::from_empty(
                    created.reference().object_id,
                    created.reference().version.as_u64() as i64 - 1,
                    ObjectStatus::NotYetCreated,
                    checkpoint_seq,
                ));
            }

            // 3. Unwrapped and unwrapped-then-deleted objects were previously wrapped — no
            //    data available. Use lamport version - 1 as approximation.
            let unwrapped_refs = effects
                .unwrapped()
                .into_iter()
                .map(|unwrapped| *unwrapped.reference());
            let unwrapped_then_deleted_refs = effects.unwrapped_then_deleted().into_iter();
            for r in unwrapped_refs.chain(unwrapped_then_deleted_refs) {
                history_objects.push(StoredBackwardHistoryObject::from_empty(
                    r.object_id,
                    r.version.as_u64() as i64 - 1,
                    ObjectStatus::WrappedOrDeleted,
                    checkpoint_seq,
                ));
            }
        }

        history_objects
    }

    fn build_object_versions(self) -> Vec<StoredObjectVersion> {
        let cp_sequence_number = self.checkpoint.sequence_number() as i64;
        let removed = self
            .checkpoint
            .iter_transactions()
            .flat_map(|tx| tx.removed_object_refs_post_version())
            .map(|obj_ref| StoredObjectVersion {
                object_id: obj_ref.object_id.as_bytes().to_vec(),
                object_version: obj_ref.version.as_u64() as i64,
                cp_sequence_number,
            });
        let output = self
            .checkpoint
            .iter_transactions()
            .flat_map(|tx| &tx.output_objects)
            .map(|o| StoredObjectVersion {
                object_id: o.id().as_bytes().to_vec(),
                object_version: o.version().as_u64() as i64,
                cp_sequence_number,
            });
        removed.chain(output).collect()
    }
}

#[derive(Debug)]
struct ObjectData {
    checkpoint_objects: CheckpointObjectChanges,
    object_versions: Vec<StoredObjectVersion>,
    history_objects: Vec<StoredBackwardHistoryObject>,
    packages: Vec<IndexedPackage>,
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

/// Along with InMemObjectCache, TxChangesProcessor implements ObjectProvider
/// so it can be used in indexing write path to get object/balance changes.
/// Its lifetime is per checkpoint.
pub struct InMemTxChanges {
    object_cache: InMemObjectCache,
    metrics: IndexerMetrics,
}

impl InMemTxChanges {
    pub fn new(objects: &[&Object], metrics: IndexerMetrics) -> Self {
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
impl ObjectProvider for InMemTxChanges {
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

/// Represents objects for end-of-epoch indexing.
/// Used to extract IotaSystemState and its dynamic children for end-of-epoch
/// indexing.
pub(crate) struct EpochEndIndexingObjectStore<'a> {
    objects: Vec<&'a Object>,
}

impl<'a> EpochEndIndexingObjectStore<'a> {
    pub fn new(data: &'a CheckpointData) -> Self {
        Self {
            objects: data.latest_live_output_objects(),
        }
    }
}

impl iota_types::storage::ObjectStore for EpochEndIndexingObjectStore<'_> {
    fn try_get_object(
        &self,
        object_id: &ObjectId,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self
            .objects
            .iter()
            .find(|o| o.id() == *object_id)
            .cloned()
            .cloned())
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: iota_types::base_types::VersionNumber,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self
            .objects
            .iter()
            .find(|o| o.id() == *object_id && o.version() == version)
            .cloned()
            .cloned())
    }
}
