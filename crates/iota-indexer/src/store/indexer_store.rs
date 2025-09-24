// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{any::Any, collections::BTreeMap};

use async_trait::async_trait;
use diesel::PgConnection;

use crate::{
    errors::IndexerError,
    handlers::{EpochToCommit, TransactionObjectChangesToCommit},
    models::{
        display::StoredDisplay,
        obj_indices::StoredObjectVersion,
        objects::{StoredDeletedObject, StoredObject},
        transactions::{CheckpointTxGlobalOrder, OptimisticTransaction},
    },
    rolling::transform::CheckpointObjectChanges,
    types::{
        EventIndex, IndexedCheckpoint, IndexedEvent, IndexedPackage, IndexedTransaction, TxIndex,
        TxIndexV2,
    },
};

#[expect(clippy::large_enum_variant)]
pub enum ObjectsToCommit {
    MutatedObject(StoredObject),
    DeletedObject(StoredDeletedObject),
}

#[async_trait]
pub trait IndexerStore: Any + Clone + Sync + Send + 'static {
    async fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>, IndexerError>;

    async fn get_available_epoch_range(&self) -> Result<(u64, u64), IndexerError>;

    async fn get_available_checkpoint_range(&self) -> Result<(u64, u64), IndexerError>;

    async fn get_latest_object_snapshot_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<u64>, IndexerError>;

    async fn get_chain_identifier(&self) -> Result<Option<Vec<u8>>, IndexerError>;

    fn persist_protocol_configs_and_feature_flags(
        &self,
        chain_id: Vec<u8>,
    ) -> Result<(), IndexerError>;

    async fn persist_objects(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;

    async fn persist_object_history(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;

    async fn persist_object_versions(
        &self,
        object_versions: Vec<StoredObjectVersion>,
    ) -> Result<(), IndexerError>;

    async fn persist_objects_snapshot(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;

    async fn persist_checkpoints(
        &self,
        checkpoints: Vec<IndexedCheckpoint>,
    ) -> Result<(), IndexerError>;

    async fn persist_transactions(
        &self,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<(), IndexerError>;

    fn persist_optimistic_transaction_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        transaction: OptimisticTransaction,
    ) -> Result<(), IndexerError>;

    async fn persist_tx_indices(&self, indices: Vec<TxIndex>) -> Result<(), IndexerError>;

    async fn persist_events(&self, events: Vec<IndexedEvent>) -> Result<(), IndexerError>;

    async fn persist_event_indices(
        &self,
        event_indices: Vec<EventIndex>,
    ) -> Result<(), IndexerError>;

    async fn persist_displays(
        &self,
        display_updates: BTreeMap<String, StoredDisplay>,
    ) -> Result<(), IndexerError>;

    async fn persist_packages(&self, packages: Vec<IndexedPackage>) -> Result<(), IndexerError>;

    async fn persist_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError>;

    async fn advance_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError>;

    async fn prune_epoch(&self, epoch: u64) -> Result<(), IndexerError>;

    async fn get_network_total_transactions_by_end_of_epoch(
        &self,
        epoch: u64,
    ) -> Result<Option<u64>, IndexerError>;

    async fn refresh_participation_metrics(&self) -> Result<(), IndexerError>;

    fn as_any(&self) -> &dyn Any;

    fn persist_displays_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        display_updates: Vec<&StoredDisplay>,
    ) -> Result<(), IndexerError>;

    fn persist_objects_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError>;
}

#[async_trait]
pub(crate) trait IndexerStoreExt: IndexerStore {
    async fn persist_checkpoint_objects(
        &self,
        objects: Vec<CheckpointObjectChanges>,
    ) -> Result<(), IndexerError>;

    async fn update_status_for_checkpoint_transactions(
        &self,
        tx_order: Vec<CheckpointTxGlobalOrder>,
    ) -> Result<(), IndexerError>;

    async fn persist_tx_global_order(
        &self,
        tx_order: Vec<CheckpointTxGlobalOrder>,
    ) -> Result<(), IndexerError>;

    async fn persist_tx_indices_v2(&self, indices: Vec<TxIndexV2>) -> Result<(), IndexerError>;
}
