// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    slice,
    sync::Arc,
};

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_json_rpc::{ObjectProvider, get_balance_changes_from_effect, get_object_changes};
use iota_json_rpc_types::IotaTransactionKind;
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    event::{SystemEpochInfoEvent, SystemEpochInfoEventV1, SystemEpochInfoEventV2},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    iota_system_state::{IotaSystemStateTrait, get_iota_system_state},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::{Object, Owner},
    transaction::{TransactionData, TransactionDataAPI},
};
use itertools::Itertools;
use tracing::{info, warn};

use crate::{
    db::ConnectionPool,
    errors::IndexerError,
    ingestion::{
        common::prepare::{CheckpointObjectChanges, extract_df_kind},
        primary::persist::{
            CheckpointDataToCommit, EpochToCommit, TransactionObjectChangesToCommit,
        },
    },
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate},
        obj_indices::StoredObjectVersion,
    },
    store::{IndexerStore, PgIndexerStore},
    types::{
        EventIndex, IndexedCheckpoint, IndexedDeletedObject, IndexedEpochInfoEvent, IndexedEvent,
        IndexedObject, IndexedObjectChange, IndexedPackage, IndexedTransaction, IndexerResult,
        TxIndex,
    },
};

pub struct PrimaryWorker {
    metrics: IndexerMetrics,
    indexed_checkpoint_sender: iota_metrics::metered_channel::Sender<CheckpointDataToCommit>,
}

pub type IndexedTransactionComponents = (
    IndexedTransaction,
    TxIndex,
    Vec<IndexedEvent>,
    Vec<EventIndex>,
    BTreeMap<String, StoredDisplay>,
);

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

        let checkpoint_data = Self::index_checkpoint(
            &checkpoint,
            Arc::new(self.metrics.clone()),
            Self::index_packages(slice::from_ref(&checkpoint), &self.metrics),
        )
        .await?;
        self.indexed_checkpoint_sender
            .send(checkpoint_data)
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

    async fn index_epoch(data: &CheckpointData) -> Result<Option<EpochToCommit>, IndexerError> {
        let checkpoint_object_store = EpochEndIndexingObjectStore::new(data);

        let CheckpointData {
            transactions,
            checkpoint_summary,
            checkpoint_contents: _,
        } = data;

        // Genesis epoch
        if *checkpoint_summary.sequence_number() == 0 {
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
            .flat_map(|t| t.events.as_ref().map(|e| &e.data))
            .flatten()
            .find(|ev| ev.is_system_epoch_info_event_v1() || ev.is_system_epoch_info_event_v2())
            .map(|ev| {
                if ev.is_system_epoch_info_event_v2() {
                    SystemEpochInfoEvent::V2(
                        bcs::from_bytes::<SystemEpochInfoEventV2>(&ev.contents).expect(
                            "event deserialization should succeed as type was pre-validated",
                        ),
                    )
                } else {
                    SystemEpochInfoEvent::V1(
                        bcs::from_bytes::<SystemEpochInfoEventV1>(&ev.contents).expect(
                            "event deserialization should succeed as type was pre-validated",
                        ),
                    )
                }
            });

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

        let event = event
            .as_ref()
            .map_or_else(Default::default, IndexedEpochInfoEvent::from);
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

    fn derive_object_versions(
        object_history_changes: &TransactionObjectChangesToCommit,
    ) -> Vec<StoredObjectVersion> {
        let mut object_versions = vec![];
        for changed_obj in object_history_changes.changed_objects.iter() {
            object_versions.push(changed_obj.into());
        }
        for deleted_obj in object_history_changes.deleted_objects.iter() {
            object_versions.push(deleted_obj.into());
        }
        object_versions
    }

    async fn index_checkpoint(
        data: &CheckpointData,
        metrics: Arc<IndexerMetrics>,
        packages: Vec<IndexedPackage>,
    ) -> Result<CheckpointDataToCommit, IndexerError> {
        let checkpoint_seq = data.checkpoint_summary.sequence_number;
        info!(checkpoint_seq, "Indexing checkpoint data blob");

        // Index epoch
        let epoch = Self::index_epoch(data).await?;

        // Index Objects
        let object_changes = Self::index_checkpoint_objects(data, &metrics).await?;
        let object_history_changes: TransactionObjectChangesToCommit =
            Self::index_objects_history(data).await?;
        let object_versions = Self::derive_object_versions(&object_history_changes);

        let (checkpoint, db_transactions, db_events, db_tx_indices, db_event_indices, db_displays) = {
            let CheckpointData {
                transactions,
                checkpoint_summary,
                checkpoint_contents,
            } = data;

            let (db_transactions, db_events, db_tx_indices, db_event_indices, db_displays) =
                Self::index_transactions(
                    transactions,
                    checkpoint_summary,
                    checkpoint_contents,
                    &metrics,
                )
                .await?;

            let successful_tx_num: u64 = db_transactions.iter().map(|t| t.successful_tx_num).sum();
            (
                IndexedCheckpoint::from_iota_checkpoint(
                    checkpoint_summary,
                    checkpoint_contents,
                    successful_tx_num as usize,
                ),
                db_transactions,
                db_events,
                db_tx_indices,
                db_event_indices,
                db_displays,
            )
        };
        let time_now_ms = chrono::Utc::now().timestamp_millis();
        metrics
            .index_lag_ms
            .set(time_now_ms - checkpoint.timestamp_ms as i64);
        metrics
            .max_indexed_checkpoint_sequence_number
            .set(checkpoint.sequence_number as i64);
        metrics
            .indexed_checkpoint_timestamp_ms
            .set(checkpoint.timestamp_ms as i64);
        info!(
            "Indexer lag: indexed checkpoint {} with time now {} and checkpoint time {}",
            checkpoint.sequence_number, time_now_ms, checkpoint.timestamp_ms
        );

        Ok(CheckpointDataToCommit {
            checkpoint,
            transactions: db_transactions,
            events: db_events,
            tx_indices: db_tx_indices,
            event_indices: db_event_indices,
            display_updates: db_displays,
            object_changes,
            object_history_changes,
            object_versions,
            packages,
            epoch,
        })
    }

    async fn index_transactions(
        transactions: &[CheckpointTransaction],
        checkpoint_summary: &CertifiedCheckpointSummary,
        checkpoint_contents: &CheckpointContents,
        metrics: &IndexerMetrics,
    ) -> IndexerResult<(
        Vec<IndexedTransaction>,
        Vec<IndexedEvent>,
        Vec<TxIndex>,
        Vec<EventIndex>,
        BTreeMap<String, StoredDisplay>,
    )> {
        let checkpoint_seq = checkpoint_summary.sequence_number();

        let mut tx_seq_num_iter = checkpoint_contents
            .enumerate_transactions(checkpoint_summary)
            .map(|(seq, execution_digest)| (execution_digest.transaction, seq));

        if checkpoint_contents.size() != transactions.len() {
            return Err(IndexerError::FullNodeReading(format!(
                "checkpointContents has different size {} compared to Transactions {} for checkpoint {checkpoint_seq}",
                checkpoint_contents.size(),
                transactions.len()
            )));
        }

        let mut db_transactions = Vec::new();
        let mut db_events = Vec::new();
        let mut db_displays = BTreeMap::new();
        let mut db_tx_indices = Vec::new();
        let mut db_event_indices = Vec::new();

        for tx in transactions {
            // Unwrap safe - we checked they have equal length above
            let (tx_digest, tx_sequence_number) = tx_seq_num_iter.next().unwrap();
            let actual_tx_digest = tx.transaction.digest();
            if tx_digest != *actual_tx_digest {
                return Err(IndexerError::FullNodeReading(format!(
                    "transactions has different ordering from CheckpointContents, for checkpoint {checkpoint_seq}, Mismatch found at {tx_digest} v.s. {actual_tx_digest}",
                )));
            }

            let (indexed_tx, tx_indices, indexed_events, events_indices, stored_displays) =
                Self::index_transaction_components(
                    tx,
                    tx_sequence_number,
                    *checkpoint_seq,
                    checkpoint_summary.timestamp_ms,
                    metrics,
                )
                .await?;
            db_transactions.push(indexed_tx);
            db_tx_indices.push(tx_indices);
            db_events.extend(indexed_events);
            db_event_indices.extend(events_indices);
            db_displays.extend(stored_displays);
        }
        Ok((
            db_transactions,
            db_events,
            db_tx_indices,
            db_event_indices,
            db_displays,
        ))
    }

    pub(crate) async fn index_transaction_components(
        tx: &CheckpointTransaction,
        tx_sequence_number: u64,
        checkpoint_seq: CheckpointSequenceNumber,
        checkpoint_timestamp_ms: u64,
        metrics: &IndexerMetrics,
    ) -> IndexerResult<IndexedTransactionComponents> {
        let db_txn = Self::index_transaction(
            tx,
            tx_sequence_number,
            checkpoint_seq,
            checkpoint_timestamp_ms,
            metrics,
        )
        .await?;

        let CheckpointTransaction {
            transaction: sender_signed_data,
            effects: fx,
            events,
            ..
        } = tx;

        let tx_digest = sender_signed_data.digest();
        let tx = sender_signed_data.transaction_data();
        let events = events
            .as_ref()
            .map(|events| events.data.clone())
            .unwrap_or_default();

        let transaction_kind = IotaTransactionKind::from(tx.kind());

        let db_events = events
            .iter()
            .enumerate()
            .map(|(idx, event)| {
                IndexedEvent::from_event(
                    tx_sequence_number,
                    idx as u64,
                    checkpoint_seq,
                    *tx_digest,
                    event,
                    checkpoint_timestamp_ms,
                )
            })
            .collect();

        let db_event_indices = events
            .iter()
            .enumerate()
            .map(|(idx, event)| EventIndex::from_event(tx_sequence_number, idx as u64, event))
            .collect();

        let db_displays = events
            .iter()
            .flat_map(StoredDisplay::try_from_event)
            .map(|display| (display.object_type.clone(), display))
            .collect();

        // Input Objects
        let input_objects = tx
            .input_objects()
            .expect("committed txns have been validated")
            .into_iter()
            .map(|obj_kind| obj_kind.object_id())
            .collect::<Vec<_>>();

        // Changed Objects
        let changed_objects = fx
            .all_changed_objects()
            .into_iter()
            .map(|(object_ref, _owner, _write_kind)| object_ref.0)
            .collect::<Vec<_>>();

        // Wrapped or deleted objects
        let wrapped_or_deleted_objects = fx
            .all_tombstones()
            .into_iter()
            .chain(fx.created_then_wrapped_objects())
            .map(|(object_id, _)| object_id)
            .collect::<Vec<_>>();

        // Payers
        let payers = vec![tx.gas_owner()];

        // Sender
        let sender = tx.sender();

        // Recipients
        let recipients = fx
            .all_changed_objects()
            .into_iter()
            .filter_map(|(_object_ref, owner, _write_kind)| match owner {
                Owner::AddressOwner(address) => Some(address),
                _ => None,
            })
            .unique()
            .collect::<Vec<_>>();

        // Move Calls
        let move_calls = tx
            .move_calls()
            .iter()
            .map(|(p, m, f)| (*<&ObjectID>::clone(p), m.to_string(), f.to_string()))
            .collect();

        let db_tx_indices = TxIndex {
            tx_sequence_number,
            transaction_digest: *tx_digest,
            checkpoint_sequence_number: checkpoint_seq,
            input_objects,
            changed_objects,
            sender,
            payers,
            recipients,
            move_calls,
            tx_kind: transaction_kind,
            wrapped_or_deleted_objects,
        };

        Ok((
            db_txn,
            db_tx_indices,
            db_events,
            db_event_indices,
            db_displays,
        ))
    }

    /// Creates a new [`IndexedTransaction`]
    pub(crate) async fn index_transaction(
        tx: &CheckpointTransaction,
        tx_sequence_number: u64,
        checkpoint_seq: CheckpointSequenceNumber,
        checkpoint_timestamp_ms: u64,
        metrics: &IndexerMetrics,
    ) -> IndexerResult<IndexedTransaction> {
        let tx_digest = tx.transaction.digest();
        let tx_data = tx.transaction.transaction_data();

        let events = tx
            .events
            .as_ref()
            .map(|events| events.data.clone())
            .unwrap_or_default();

        let transaction_kind = IotaTransactionKind::from(tx_data.kind());

        let objects = tx
            .input_objects
            .iter()
            .chain(tx.output_objects.iter())
            .collect::<Vec<_>>();

        let (balance_change, object_changes) = InMemTxChanges::new(&objects, metrics.clone())
            .get_changes(tx_data, &tx.effects, tx_digest)
            .await?;

        Ok(IndexedTransaction {
            tx_sequence_number,
            tx_digest: *tx_digest,
            checkpoint_sequence_number: checkpoint_seq,
            timestamp_ms: checkpoint_timestamp_ms,
            sender_signed_data: tx.transaction.data().clone(),
            successful_tx_num: if tx.effects.status().is_ok() {
                tx_data.kind().tx_count() as u64
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

    pub(crate) async fn index_checkpoint_objects(
        data: &CheckpointData,
        metrics: &IndexerMetrics,
    ) -> Result<CheckpointObjectChanges, IndexerError> {
        let _timer = metrics.indexing_objects_latency.start_timer();
        data.try_into()
    }

    pub(crate) async fn index_objects(
        data: &CheckpointData,
        metrics: &IndexerMetrics,
    ) -> Result<TransactionObjectChangesToCommit, IndexerError> {
        let _timer = metrics.indexing_objects_latency.start_timer();
        let checkpoint_seq = data.checkpoint_summary.sequence_number;

        let eventually_removed_object_refs_post_version =
            data.eventually_removed_object_refs_post_version();
        let indexed_eventually_removed_objects = eventually_removed_object_refs_post_version
            .into_iter()
            .map(|obj_ref| IndexedDeletedObject {
                object_id: obj_ref.0,
                object_version: obj_ref.1.into(),
                checkpoint_sequence_number: checkpoint_seq,
            })
            .collect();

        let latest_live_output_objects = data.latest_live_output_objects();
        let changed_objects = latest_live_output_objects
            .into_iter()
            .map(|o| {
                let df_kind = extract_df_kind(o);
                IndexedObject::from_object(checkpoint_seq, o.clone(), df_kind)
            })
            .collect::<Vec<_>>();
        Ok(TransactionObjectChangesToCommit {
            changed_objects,
            deleted_objects: indexed_eventually_removed_objects,
        })
    }

    // similar to index_objects, but objects_history keeps all versions of objects
    async fn index_objects_history(
        data: &CheckpointData,
    ) -> Result<TransactionObjectChangesToCommit, IndexerError> {
        let checkpoint_seq = data.checkpoint_summary.sequence_number;
        let deleted_objects = data
            .transactions
            .iter()
            .flat_map(|tx| tx.removed_object_refs_post_version())
            .collect::<Vec<_>>();
        let indexed_deleted_objects: Vec<IndexedDeletedObject> = deleted_objects
            .into_iter()
            .map(|obj_ref| IndexedDeletedObject {
                object_id: obj_ref.0,
                object_version: obj_ref.1.into(),
                checkpoint_sequence_number: checkpoint_seq,
            })
            .collect();

        let output_objects: Vec<_> = data
            .transactions
            .iter()
            .flat_map(|tx| &tx.output_objects)
            .collect();
        // TODO(gegaowp): the current df_info implementation is not correct,
        // but we have decided remove all df_* except df_kind.
        let changed_objects = output_objects
            .into_iter()
            .map(|o| {
                let df_kind = extract_df_kind(o);
                IndexedObject::from_object(checkpoint_seq, o.clone(), df_kind)
            })
            .collect::<Vec<_>>();

        Ok(TransactionObjectChangesToCommit {
            changed_objects,
            deleted_objects: indexed_deleted_objects,
        })
    }

    fn index_packages(
        checkpoint_data: &[CheckpointData],
        metrics: &IndexerMetrics,
    ) -> Vec<IndexedPackage> {
        let _timer = metrics.indexing_packages_latency.start_timer();
        checkpoint_data
            .iter()
            .flat_map(|data| {
                let checkpoint_sequence_number = data.checkpoint_summary.sequence_number;
                data.transactions
                    .iter()
                    .flat_map(|tx| &tx.output_objects)
                    .filter_map(|o| {
                        if let iota_types::object::Data::Package(p) = &o.data {
                            Some(IndexedPackage {
                                package_id: o.id(),
                                move_package: p.clone(),
                                checkpoint_sequence_number,
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
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

pub struct InMemObjectCache {
    id_map: HashMap<ObjectID, Object>,
    seq_map: HashMap<(ObjectID, SequenceNumber), Object>,
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

    pub fn get(&self, id: &ObjectID, version: Option<&SequenceNumber>) -> Option<&Object> {
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
        tx: &TransactionData,
        effects: &TransactionEffects,
        tx_digest: &TransactionDigest,
    ) -> IndexerResult<(
        Vec<iota_json_rpc_types::BalanceChange>,
        Vec<IndexedObjectChange>,
    )> {
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
                panic!("checkpointed tx {tx_digest:?} has invalid input objects: {e}")
            }),
            None,
        )
        .await?;
        Ok((balance_change, object_change))
    }
}

#[async_trait]
impl ObjectProvider for InMemTxChanges {
    type Error = IndexerError;

    async fn get_object(
        &self,
        id: &ObjectID,
        version: &SequenceNumber,
    ) -> Result<Object, Self::Error> {
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
        id: &ObjectID,
        version: &SequenceNumber,
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
        object_id: &ObjectID,
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
        object_id: &ObjectID,
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
