// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use futures::StreamExt;
use grpc_ledger_service::checkpoint_data::Progress;
use iota_grpc_types::{
    field::FieldMaskTree,
    proto::timestamp_ms_to_proto,
    v1::{
        checkpoint as grpc_checkpoint, event as grpc_event,
        ledger_service::{self as grpc_ledger_service},
        transaction as grpc_transaction,
    },
};
use iota_types::{
    base_types::{ObjectID, VersionNumber},
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    full_checkpoint_content::{
        CheckpointData as IotaTypesCheckpointData,
        CheckpointTransaction as IotaTypesCheckpointTransaction,
    },
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents},
    object::Object,
    storage::{ObjectStore, ReadStore, RestStateReader, error::Kind},
    transaction::VerifiedTransaction,
};
use prost::Message;
use tokio::sync::broadcast::{Receiver, Sender, error::RecvError};
use tokio_util::sync::CancellationToken;
use tonic::Status;
use tracing::debug;

use crate::{error::RpcError, merge::Merge};

/// Flags indicating which optional transaction fields to fetch from storage.
/// Derived from a `FieldMaskTree` to skip unnecessary storage reads.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransactionReadFields {
    pub include_transaction: bool,
    pub include_signatures: bool,
    pub include_effects: bool,
    pub include_events: bool,
    pub include_checkpoint: bool,
    pub include_timestamp: bool,
    pub include_input_objects: bool,
    pub include_output_objects: bool,
}

impl TransactionReadFields {
    /// Derive which fields to fetch from an `ExecutedTransaction` field mask.
    pub fn from_mask(mask: &FieldMaskTree) -> Self {
        use iota_grpc_types::v1::transaction::ExecutedTransaction;

        Self {
            include_transaction: mask.contains(ExecutedTransaction::TRANSACTION_FIELD.name),
            include_signatures: mask.contains(ExecutedTransaction::SIGNATURES_FIELD.name),
            include_effects: mask.contains(ExecutedTransaction::EFFECTS_FIELD.name),
            include_events: mask.contains(ExecutedTransaction::EVENTS_FIELD.name),
            include_checkpoint: mask.contains(ExecutedTransaction::CHECKPOINT_FIELD.name),
            include_timestamp: mask.contains(ExecutedTransaction::TIMESTAMP_FIELD.name),
            include_input_objects: mask.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name),
            include_output_objects: mask.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name),
        }
    }
}

pub type GetObjectsStream = Pin<Box<dyn futures::Stream<Item = ObjectsStreamResult> + Send>>;
pub type GetTransactionsStream =
    Pin<Box<dyn futures::Stream<Item = TransactionsStreamResult> + Send>>;

/// Server streaming response type for the GetCheckpoint method.
pub type GetCheckpointStream = Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>;

/// Server streaming response type for the StreamCheckpoints method.
pub type StreamCheckpointsStream =
    Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>;

/// Wrapper that converts native CheckpointData to gRPC type before broadcasting
#[derive(Clone)]
pub struct GrpcCheckpointDataBroadcaster {
    sender: Sender<Arc<IotaTypesCheckpointData>>,
}

impl GrpcCheckpointDataBroadcaster {
    pub fn new(sender: Sender<Arc<IotaTypesCheckpointData>>) -> Self {
        Self { sender }
    }

    /// Subscribe to checkpoint data broadcasts
    pub fn subscribe(&self) -> Receiver<Arc<IotaTypesCheckpointData>> {
        self.sender.subscribe()
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Send with integrated tracing and error handling
    pub fn send_traced(&self, data: &IotaTypesCheckpointData) {
        // Only send if there are active subscribers
        if self.receiver_count() == 0 {
            return;
        }

        match self.sender.send(Arc::new(data.clone())) {
            Ok(_) => {
                debug!(
                    "Sent checkpoint data #{} to {} gRPC subscriber(s)",
                    data.checkpoint_summary.data().sequence_number,
                    self.receiver_count()
                );
            }
            Err(_) => {
                debug!(
                    "No gRPC clients subscribed for checkpoint data #{}",
                    data.checkpoint_summary.data().sequence_number
                );
            }
        }
    }
}

// Type aliases and utility types
pub type ObjectsStreamResult = Result<grpc_ledger_service::GetObjectsResponse, Status>;
pub type TransactionsStreamResult = Result<grpc_ledger_service::GetTransactionsResponse, Status>;
pub type CheckpointStreamResult = Result<grpc_ledger_service::CheckpointData, Status>;

// Iterator item types for state reader methods.
//
// These mirror the `iota_types::storage` item types but use `anyhow::Result`
// so that different storage backends (RocksDB, mock, simulacrum) can map
// their concrete errors into a uniform error type.

/// A dynamic-field index entry (key + index info).
pub type DynamicFieldIterItem = anyhow::Result<(
    iota_types::storage::DynamicFieldKey,
    iota_types::storage::DynamicFieldIndexInfo,
)>;

/// An owned-object from the legacy `owner` (v1) index.
pub type OwnedObjectIterItem = anyhow::Result<iota_types::storage::AccountOwnedObjectInfo>;

pub use iota_types::storage::OwnedObjectV2Cursor;

/// An owned-object together with the v2 seek cursor for the position it
/// occupies in the index.
///
/// Unlike [`OwnedObjectIterItem`], this carries the full v2 key components
/// so that page tokens can encode an exact seek position.
pub type OwnedObjectV2IterItem = anyhow::Result<(
    iota_types::storage::AccountOwnedObjectInfo,
    iota_types::storage::OwnedObjectV2Cursor,
)>;

/// A package-version index entry (key + storage info).
pub type PackageVersionIterItem = anyhow::Result<(
    iota_types::storage::PackageVersionKey,
    iota_types::storage::PackageVersionInfo,
)>;

/// Result of [`GrpcReader::match_checkpoint_filter_or_report_progress`].
enum FilterCheckResult {
    /// The checkpoint contains matching data; proceed with full processing.
    Matched,
    /// The checkpoint should be skipped, with an optional progress message to
    /// yield before returning.
    Skipped(Option<grpc_ledger_service::CheckpointData>),
}

// Storage abstraction traits for gRPC access
// These traits provide an abstraction layer over the storage backend,
// making it easier to implement gRPC services with different storage types
// (e.g., production database vs simulacrum for testing).

/// Trait for reading checkpoint data from storage.
///
/// All methods return `anyhow::Result<Option<T>>` for consistency:
/// - `Ok(Some(value))`: The item was found
/// - `Ok(None)`: The item does not exist (expected case)
/// - `Err(e)`: A storage or other error occurred (unexpected)
pub trait GrpcStateReader: Send + Sync + 'static {
    /// Get the chain identifier.
    /// Returns `Err` on storage errors.
    fn get_chain_identifier(&self) -> anyhow::Result<iota_types::digests::ChainIdentifier>;

    /// Get the latest checkpoint sequence number.
    /// Returns `Ok(None)` if no checkpoints exist yet (e.g., during startup).
    fn get_latest_checkpoint_sequence_number(&self) -> anyhow::Result<Option<u64>>;

    /// Get checkpoint summary by sequence number.
    /// Returns `Ok(None)` if the checkpoint doesn't exist.
    fn get_checkpoint_summary(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>>;

    /// Get checkpoint sequence number by digest.
    /// Returns `Ok(None)` if the checkpoint doesn't exist.
    fn get_checkpoint_sequence_number_by_digest(
        &self,
        digest: &iota_types::digests::CheckpointDigest,
    ) -> anyhow::Result<Option<u64>>;

    /// Get full checkpoint data by sequence number.
    /// Returns `Ok(None)` if the checkpoint doesn't exist.
    fn get_checkpoint_data(&self, seq: u64) -> anyhow::Result<Option<IotaTypesCheckpointData>>;

    /// Get checkpoint summary and contents by sequence number.
    /// Returns `Ok(None)` if the checkpoint doesn't exist.
    fn get_checkpoint_summary_and_contents(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<(CertifiedCheckpointSummary, CheckpointContents)>>;

    /// Stream checkpoint transactions individually to avoid large memory
    /// footprint. Returns a stream of individual CheckpointTransaction items
    /// along with metadata.
    fn stream_checkpoint_transactions(
        &self,
        checkpoint_contents: CheckpointContents,
    ) -> std::pin::Pin<
        Box<dyn futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send + '_>,
    >;

    /// Get epoch's last checkpoint for epoch boundary calculations.
    /// Returns `Ok(None)` if the epoch doesn't exist or hasn't ended.
    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>>;

    /// Get the lowest available checkpoint for which checkpoint and transaction
    /// data are available.
    fn get_lowest_available_checkpoint(&self) -> anyhow::Result<u64>;

    /// Get the lowest available checkpoint for which object data is available.
    fn get_lowest_available_checkpoint_objects(&self) -> anyhow::Result<u64>;

    /// Get an object by its ObjectID.
    /// Returns `Ok(None)` if the object doesn't exist.
    fn get_object(&self, object_id: &ObjectID) -> anyhow::Result<Option<Object>>;

    /// Get an object by its ObjectID and version.
    /// Returns `Ok(None)` if the object at that version doesn't exist.
    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> anyhow::Result<Option<Object>>;

    /// Get committee for a specific epoch.
    /// Returns `Ok(None)` if the epoch doesn't exist.
    fn get_committee(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<Arc<iota_types::committee::Committee>>>;

    /// Get the IOTA system state.
    /// This loads the system state including its dynamic fields.
    fn get_system_state(&self) -> anyhow::Result<iota_types::iota_system_state::IotaSystemState>;

    /// Get indexed epoch information.
    /// Returns `Ok(None)` if the epoch is not found, `Err` on storage errors.
    fn get_epoch_info(&self, epoch: u64) -> anyhow::Result<Option<iota_types::storage::EpochInfo>>;

    /// Get the Move type layout for a given TypeTag.
    /// Returns `Ok(None)` if the layout is not available.
    fn get_type_layout(
        &self,
        type_tag: &iota_types::TypeTag,
    ) -> anyhow::Result<Option<move_core_types::annotated_value::MoveTypeLayout>>;

    /// Get a transaction by its digest.
    /// Returns `Ok(None)` if the transaction doesn't exist.
    fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<Arc<VerifiedTransaction>>>;

    /// Get transaction effects by digest.
    /// Returns `Ok(None)` if the effects don't exist.
    fn get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<TransactionEffects>>;

    /// Get transaction events by event digest.
    /// Returns `Ok(None)` if the events don't exist.
    fn get_transaction_events(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<TransactionEvents>>;

    /// Get checkpoint sequence number for a transaction.
    /// Returns `Ok(None)` if the transaction is not found, `Err` on storage
    /// errors.
    fn get_transaction_checkpoint(&self, digest: &TransactionDigest)
    -> anyhow::Result<Option<u64>>;

    /// Iterate over objects owned by an account address using the legacy
    /// `owner` (v1) table, optionally filtered by type.
    ///
    /// Used as a fallback when the `owner_v2` backfill is still in progress.
    ///
    /// **Cursor contract (raw):** bounds are *inclusive*.
    fn account_owned_objects_info_iter(
        &self,
        owner: iota_types::base_types::IotaAddress,
        cursor: Option<ObjectID>,
        object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = OwnedObjectIterItem> + '_>>;

    /// Iterate over objects owned by an account address using the `owner_v2`
    /// table, optionally filtered by type.
    ///
    /// When `cursor` is `Some`, the iterator starts at (and includes) the
    /// cursor position — callers must `.skip(1)` to get exclusive semantics.
    fn account_owned_objects_info_iter_v2(
        &self,
        owner: iota_types::base_types::IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = OwnedObjectV2IterItem> + '_>>;

    /// Iterate over dynamic fields of a parent object.
    ///
    /// **Cursor contract (raw):** bounds are *inclusive*. When `cursor` is
    /// `Some`, the iterator starts at (and includes) the cursor item.
    /// Prefer using the `GrpcReader` wrapper methods which automatically
    /// skip the cursor item.
    fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = DynamicFieldIterItem> + '_>>;

    /// Get coin info (metadata and treasury object IDs) for a coin type.
    fn get_coin_info(
        &self,
        coin_type: &move_core_types::language_storage::StructTag,
    ) -> anyhow::Result<Option<iota_types::storage::CoinInfo>>;

    /// Get unified coin info from the `coin_v2` table.
    fn get_coin_v2_info(
        &self,
        coin_type: &move_core_types::language_storage::StructTag,
    ) -> anyhow::Result<Option<iota_types::storage::CoinInfoV2>>;

    /// Iterate over all versions of a package by its original package ID.
    fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = PackageVersionIterItem> + '_>>;

    /// Returns `true` once the `owner_v2` backfill has completed.
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_owner_v2_index_ready(&self) -> bool {
        true
    }

    /// Returns `true` once the `coin_v2` backfill has completed.
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_coin_v2_index_ready(&self) -> bool {
        true
    }

    /// Returns `true` once the `package_version` backfill has completed.
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_package_version_index_ready(&self) -> bool {
        true
    }
}

/// Adapter that implements GrpcStateReader for RestStateReader
pub struct RestStateReaderAdapter {
    inner: Arc<dyn RestStateReader>,
}

impl RestStateReaderAdapter {
    /// Get the indexes or return an error if they are not available on this
    /// node.
    ///
    /// New index-dependent endpoints use this (hard fail) rather than the
    /// `Ok(None)` pattern used by `get_transaction_checkpoint` and
    /// `get_epoch_info`, because those older methods have fallback behavior
    /// when indexes are absent, while these endpoints cannot function at all
    /// without indexes.
    fn require_indexes(&self) -> anyhow::Result<&dyn iota_types::storage::RestIndexes> {
        self.inner
            .indexes()
            .ok_or_else(|| crate::error::MissingIndexesError.into())
    }
}

impl GrpcStateReader for RestStateReaderAdapter {
    fn get_chain_identifier(&self) -> anyhow::Result<iota_types::digests::ChainIdentifier> {
        self.inner.get_chain_identifier().map_err(Into::into)
    }

    fn get_latest_checkpoint_sequence_number(&self) -> anyhow::Result<Option<u64>> {
        match self.inner.try_get_latest_checkpoint() {
            Ok(checkpoint) => Ok(Some(*checkpoint.sequence_number())),
            Err(e) => match e.kind() {
                // Expected during server initialization when no checkpoints have been executed yet
                // Return None to indicate service is not ready
                Kind::Missing => Ok(None),
                // Unexpected storage errors - propagate instead of panicking
                _ => Err(anyhow::anyhow!(
                    "Storage error getting latest checkpoint: {e}"
                )),
            },
        }
    }

    fn get_checkpoint_summary(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        Ok(self
            .inner
            .get_checkpoint_by_sequence_number(seq)
            .map(CertifiedCheckpointSummary::from))
    }

    fn get_checkpoint_sequence_number_by_digest(
        &self,
        digest: &iota_types::digests::CheckpointDigest,
    ) -> anyhow::Result<Option<u64>> {
        Ok(self
            .inner
            .get_checkpoint_by_digest(digest)
            .map(|checkpoint| *checkpoint.sequence_number()))
    }

    fn get_checkpoint_summary_and_contents(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<(CertifiedCheckpointSummary, CheckpointContents)>> {
        let Some(summary) = self.inner.get_checkpoint_by_sequence_number(seq) else {
            return Ok(None);
        };
        let Some(contents) = self.inner.get_checkpoint_contents_by_sequence_number(seq) else {
            return Ok(None);
        };
        Ok(Some((CertifiedCheckpointSummary::from(summary), contents)))
    }

    fn get_checkpoint_data(&self, seq: u64) -> anyhow::Result<Option<IotaTypesCheckpointData>> {
        let Some(summary) = self.inner.get_checkpoint_by_sequence_number(seq) else {
            return Ok(None);
        };
        let Some(contents) = self.inner.get_checkpoint_contents_by_sequence_number(seq) else {
            return Ok(None);
        };
        Ok(Some(self.inner.get_checkpoint_data(summary, contents)))
    }

    fn stream_checkpoint_transactions(
        &self,
        checkpoint_contents: CheckpointContents,
    ) -> std::pin::Pin<
        Box<dyn futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send + '_>,
    > {
        self.inner
            .stream_checkpoint_transactions(checkpoint_contents)
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        match self.inner.get_epoch_last_checkpoint(epoch) {
            Ok(Some(checkpoint)) => Ok(Some(CertifiedCheckpointSummary::from(checkpoint))),
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn get_lowest_available_checkpoint(&self) -> anyhow::Result<u64> {
        self.inner
            .try_get_lowest_available_checkpoint()
            .map_err(Into::into)
    }

    fn get_lowest_available_checkpoint_objects(&self) -> anyhow::Result<u64> {
        self.inner
            .get_lowest_available_checkpoint_objects()
            .map_err(Into::into)
    }

    fn get_object(&self, object_id: &ObjectID) -> anyhow::Result<Option<Object>> {
        Ok(self.inner.get_object(object_id))
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> anyhow::Result<Option<Object>> {
        Ok(self.inner.get_object_by_key(object_id, version))
    }

    fn get_committee(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<Arc<iota_types::committee::Committee>>> {
        self.inner.try_get_committee(epoch).map_err(Into::into)
    }

    fn get_system_state(&self) -> anyhow::Result<iota_types::iota_system_state::IotaSystemState> {
        // self.inner is Arc<dyn RestStateReader>
        // RestStateReader extends ObjectStore, so we can pass it directly
        iota_types::iota_system_state::get_iota_system_state(self.inner.as_ref())
            .map_err(Into::into)
    }

    fn get_epoch_info(&self, epoch: u64) -> anyhow::Result<Option<iota_types::storage::EpochInfo>> {
        match self.inner.indexes() {
            Some(indexes) => indexes.get_epoch_info(epoch).map_err(Into::into),
            None => Ok(None),
        }
    }

    fn get_type_layout(
        &self,
        type_tag: &iota_types::TypeTag,
    ) -> anyhow::Result<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        self.inner.get_type_layout(type_tag).map_err(Into::into)
    }

    fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<Arc<VerifiedTransaction>>> {
        Ok(self.inner.get_transaction(digest))
    }

    fn get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<TransactionEffects>> {
        Ok(self.inner.get_transaction_effects(digest))
    }

    fn get_transaction_events(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<TransactionEvents>> {
        Ok(self.inner.get_events(digest))
    }

    fn get_transaction_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<u64>> {
        match self.inner.indexes() {
            Some(indexes) => indexes
                .get_transaction_info(digest)
                .map(|opt| opt.map(|info| info.checkpoint))
                .map_err(Into::into),
            None => Ok(None),
        }
    }

    fn account_owned_objects_info_iter(
        &self,
        owner: iota_types::base_types::IotaAddress,
        cursor: Option<ObjectID>,
        object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = OwnedObjectIterItem> + '_>> {
        let indexes = self.require_indexes()?;
        let iter = indexes.account_owned_objects_info_iter(owner, cursor, object_type)?;
        Ok(Box::new(iter.map(|r| r.map_err(Into::into))))
    }

    fn account_owned_objects_info_iter_v2(
        &self,
        owner: iota_types::base_types::IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = OwnedObjectV2IterItem> + '_>> {
        let indexes = self.require_indexes()?;
        let iter = indexes.account_owned_objects_info_iter_v2(owner, cursor, object_type)?;
        Ok(Box::new(iter.map(|r| r.map_err(Into::into))))
    }

    fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = DynamicFieldIterItem> + '_>> {
        let indexes = self.require_indexes()?;
        let iter = indexes.dynamic_field_iter(parent, cursor)?;
        Ok(Box::new(iter.map(|r| r.map_err(Into::into))))
    }

    fn get_coin_info(
        &self,
        coin_type: &move_core_types::language_storage::StructTag,
    ) -> anyhow::Result<Option<iota_types::storage::CoinInfo>> {
        let indexes = self.require_indexes()?;
        indexes.get_coin_info(coin_type).map_err(Into::into)
    }

    fn get_coin_v2_info(
        &self,
        coin_type: &move_core_types::language_storage::StructTag,
    ) -> anyhow::Result<Option<iota_types::storage::CoinInfoV2>> {
        let indexes = self.require_indexes()?;
        indexes.get_coin_v2_info(coin_type).map_err(Into::into)
    }

    fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = PackageVersionIterItem> + '_>> {
        let indexes = self.require_indexes()?;
        let iter = indexes
            .package_versions_iter(original_package_id, cursor)?
            .map(|r| r.map_err(Into::into));
        Ok(Box::new(iter))
    }

    fn is_owner_v2_index_ready(&self) -> bool {
        self.inner
            .indexes()
            .is_none_or(|i| i.is_owner_v2_index_ready())
    }

    fn is_coin_v2_index_ready(&self) -> bool {
        self.inner
            .indexes()
            .is_none_or(|i| i.is_coin_v2_index_ready())
    }

    fn is_package_version_index_ready(&self) -> bool {
        self.inner
            .indexes()
            .is_none_or(|i| i.is_package_version_index_ready())
    }
}

/// Central gRPC data reader that provides unified access to checkpoint data.
/// It provides methods for streaming both full checkpoint data and checkpoint
/// summaries.
#[derive(Clone)]
pub struct GrpcReader {
    state_reader: Arc<dyn GrpcStateReader>,
    server_version: Option<String>,
}

impl GrpcReader {
    pub fn new(state_reader: Arc<dyn GrpcStateReader>, server_version: Option<String>) -> Self {
        Self {
            state_reader,
            server_version,
        }
    }

    pub fn from_rest_state_reader(
        state_reader: Arc<dyn RestStateReader>,
        server_version: Option<String>,
    ) -> Self {
        Self {
            state_reader: Arc::new(RestStateReaderAdapter {
                inner: state_reader,
            }),
            server_version,
        }
    }

    pub fn server_version(&self) -> Option<String> {
        self.server_version.clone()
    }

    pub fn get_chain_identifier(&self) -> anyhow::Result<iota_types::digests::ChainIdentifier> {
        self.state_reader.get_chain_identifier()
    }

    /// Get checkpoint summary by sequence number.
    pub fn get_checkpoint_summary(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        self.state_reader.get_checkpoint_summary(seq)
    }

    /// Get checkpoint sequence number by digest
    pub fn get_checkpoint_sequence_number_by_digest(
        &self,
        digest: &iota_types::digests::CheckpointDigest,
    ) -> anyhow::Result<Option<u64>> {
        self.state_reader
            .get_checkpoint_sequence_number_by_digest(digest)
    }

    /// Get the last checkpoint of a given epoch, if any
    pub fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        self.state_reader.get_epoch_last_checkpoint(epoch)
    }

    /// Get a single checkpoint as chunked messages stream
    pub fn get_checkpoint_data(
        &self,
        sequence_number: u64,
        checkpoint_mask: FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: u32,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>> {
        let state_reader = self.state_reader.clone();
        match state_reader.get_checkpoint_summary_and_contents(sequence_number) {
            Ok(Some((checkpoint_summary, checkpoint_contents))) => {
                Box::pin(async_stream::stream! {
                    let transaction_stream = state_reader.stream_checkpoint_transactions(checkpoint_contents.clone());
                    let mut checkpoint_stream = Box::pin(Self::create_checkpoint_messages_stream(
                        checkpoint_summary,
                        checkpoint_contents,
                        transaction_stream,
                        &checkpoint_mask,
                        transactions_mask,
                        events_mask,
                        max_message_size_bytes as usize,
                        transaction_filter,
                        event_filter,
                    ));

                    while let Some(result) = checkpoint_stream.next().await {
                        yield result;
                    }
                })
            }
            Ok(None) => Box::pin(async_stream::stream! {
                yield Err(Status::not_found(format!(
                    "Checkpoint {sequence_number} not found"
                )));
            }),
            Err(e) => Box::pin(async_stream::stream! {
                yield Err(Status::internal(format!(
                    "Failed to get checkpoint {sequence_number}: {e}"
                )));
            }),
        }
    }

    /// Helper function to create checkpoint messages from checkpoint data as a
    /// stream. Sends Checkpoint first (based on checkpoint_mask), then
    /// Transactions (batched by size), then Events (if requested), and
    /// finally EndMarker.
    ///
    /// Generic over transaction stream source - works for both historical and
    /// live data.
    fn create_checkpoint_messages_stream<S>(
        checkpoint_summary: CertifiedCheckpointSummary,
        checkpoint_contents: CheckpointContents,
        transaction_stream: S,
        checkpoint_mask: &FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: usize,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
    ) -> impl futures::Stream<Item = Result<grpc_ledger_service::CheckpointData, Status>> + Send
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        use grpc_ledger_service::checkpoint_data::EndMarker;

        // Clone values needed across the async boundary
        let checkpoint_mask = checkpoint_mask.clone();

        async_stream::stream! {
            let sequence_number = checkpoint_summary.data().sequence_number;

            // 1. Send Checkpoint message (controlled by checkpoint_mask)
            // Build the Checkpoint proto message using Merge

            // We need the sequence number to reassemble the checkpoint on client side.
            let mut checkpoint_proto = grpc_checkpoint::Checkpoint::default()
                .with_sequence_number(sequence_number);

            // Convert to iota_sdk_types for Merge compatibility
            let sdk_summary: iota_sdk_types::CheckpointSummary = checkpoint_summary
                .data()
                .clone()
                .try_into()
                .map_err(|e| Status::internal(format!("failed to convert checkpoint summary: {e}")))?;

            let sdk_contents: iota_sdk_types::CheckpointContents = checkpoint_contents
                .clone()
                .try_into()
                .map_err(|e| Status::internal(format!("failed to convert checkpoint contents: {e}")))?;

            let sdk_signature = iota_sdk_types::ValidatorAggregatedSignature::from(checkpoint_summary.auth_sig().clone());

            // Use Merge to populate based on mask
            Merge::merge(&mut checkpoint_proto, &sdk_summary, &checkpoint_mask)
                .map_err(|e| e.with_context("failed to merge summary"))?;
            Merge::merge(&mut checkpoint_proto, sdk_contents, &checkpoint_mask)
                .map_err(|e| e.with_context("failed to merge contents"))?;
            Merge::merge(&mut checkpoint_proto, sdk_signature, &checkpoint_mask)
                .map_err(|e| e.with_context("failed to merge signature"))?;

            yield Ok(grpc_ledger_service::CheckpointData::default().with_checkpoint(checkpoint_proto));

            // 2. Stream transactions and events if requested (interleaved)
            if transactions_mask.is_some() || events_mask.is_some() {
                let tx_mask = transactions_mask.clone().unwrap_or_else(FieldMaskTree::new_wildcard);
                let should_collect_events = events_mask.is_some();
                let events_submask = events_mask
                    .as_ref()
                    .and_then(|m| m.subtree("events"))
                    .unwrap_or_else(FieldMaskTree::new_wildcard);

                let mut transaction_stream = Box::pin(transaction_stream);
                let mut current_batch: Vec<grpc_transaction::ExecutedTransaction> = Vec::new();
                let mut current_batch_size = 0usize;

                // Event batching state
                let mut events_batch: Vec<grpc_event::Event> = Vec::new();
                let mut events_batch_size = 0usize;

                while let Some(result) = transaction_stream.next().await {
                    match result {
                        Ok(checkpoint_transaction) => {
                            // Collect and yield events as they reach size limits
                            if should_collect_events {
                                if let Some(ref tx_events) = checkpoint_transaction.events {
                                    // Filter raw events before SDK conversion
                                    for raw_event in &tx_events.data {
                                        // Apply event filter if present
                                        if let Some(ref evt_filter) = event_filter {
                                            if !evt_filter.matches_event(raw_event) {
                                                continue; // Skip non-matching events
                                            }
                                        }

                                        // Convert matching event to SDK type
                                        let sdk_event: iota_sdk_types::Event = raw_event
                                            .clone()
                                            .try_into()
                                            .map_err(|e| Status::internal(format!("event conversion error: {e}")))?;
                                        let grpc_event = grpc_event::Event::merge_from(&sdk_event, &events_submask)
                                            .map_err(|e| e.with_context("failed to merge event"))?;
                                        let event_encoded_len = grpc_event.encoded_len();
                                        let event_size = event_encoded_len + crate::utils::repeated_field_item_overhead(event_encoded_len);

                                        // Check if a single event exceeds the message size limit
                                        let event_total = event_size + crate::utils::checkpoint_data_wrapper_overhead(event_size);
                                        if event_total > max_message_size_bytes {
                                            yield Err(Status::invalid_argument(format!(
                                                "Single event size ({event_total} bytes) exceeds max message size ({max_message_size_bytes} bytes)"
                                            )));
                                            return;
                                        }

                                        // Check if adding this event would exceed limit
                                        // (batch content + wrapper overhead for CheckpointData oneof)
                                        let candidate_size = events_batch_size + event_size;
                                        if candidate_size + crate::utils::checkpoint_data_wrapper_overhead(candidate_size) > max_message_size_bytes && !events_batch.is_empty() {
                                            // Yield current event batch
                                            yield Ok(grpc_ledger_service::CheckpointData::default()
                                                .with_events(grpc_event::Events::default().with_events(events_batch)));

                                            // Reset event batch
                                            events_batch = vec![grpc_event];
                                            events_batch_size = event_size;
                                        } else {
                                            events_batch.push(grpc_event);
                                            events_batch_size += event_size;
                                        }
                                    }
                                }
                            }

                            // Build transaction only if transactions_mask is requested
                            if transactions_mask.is_some() {
                                // Apply transaction filter if present
                                if let Some(ref tx_filter) = transaction_filter {
                                    if !tx_filter.matches_transaction(&checkpoint_transaction) {
                                        continue; // Skip non-matching transactions
                                    }
                                }

                                let checkpoint_tx_ctx = CheckpointTransactionWithContext::new(
                                    checkpoint_transaction,
                                    Some(sequence_number),
                                    Some(checkpoint_summary.data().timestamp_ms),
                                );
                                let executed_tx = grpc_transaction::ExecutedTransaction::merge_from(
                                    checkpoint_tx_ctx,
                                    &tx_mask,
                                )
                                .map_err(|e| e.with_context("failed to merge transaction"))?;
                                let tx_encoded_len = executed_tx.encoded_len();
                                let tx_size = tx_encoded_len + crate::utils::repeated_field_item_overhead(tx_encoded_len);

                                // Check if a single transaction exceeds the message size limit
                                let tx_total = tx_size + crate::utils::checkpoint_data_wrapper_overhead(tx_size);
                                if tx_total > max_message_size_bytes {
                                    yield Err(Status::invalid_argument(format!(
                                        "Single transaction size ({tx_total} bytes) exceeds max message size ({max_message_size_bytes} bytes)"
                                    )));
                                    return;
                                }

                                // Check if adding this tx would exceed limit
                                // (batch content + wrapper overhead for CheckpointData oneof)
                                let candidate_size = current_batch_size + tx_size;
                                if candidate_size + crate::utils::checkpoint_data_wrapper_overhead(candidate_size) > max_message_size_bytes && !current_batch.is_empty() {
                                    // Yield current transaction batch
                                    yield Ok(grpc_ledger_service::CheckpointData::default()
                                        .with_executed_transactions(grpc_transaction::ExecutedTransactions::default().with_executed_transactions(current_batch)));

                                    // Reset transaction batch
                                    current_batch = vec![executed_tx];
                                    current_batch_size = tx_size;
                                } else {
                                    current_batch.push(executed_tx);
                                    current_batch_size += tx_size;
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(Status::internal(format!("transaction stream error: {e}")));
                            return;
                        }
                    }
                }

                // Send final batch of transactions if any
                if transactions_mask.is_some() && !current_batch.is_empty() {
                    yield Ok(grpc_ledger_service::CheckpointData::default()
                        .with_executed_transactions(grpc_transaction::ExecutedTransactions::default().with_executed_transactions(current_batch)));
                }

                // Send final batch of events if any
                if should_collect_events && !events_batch.is_empty() {
                    yield Ok(grpc_ledger_service::CheckpointData::default()
                        .with_events(grpc_event::Events::default().with_events(events_batch)));
                }
            }

            // 3. Always send EndMarker at the end
            yield Ok(grpc_ledger_service::CheckpointData::default().with_end_marker(EndMarker::default().with_sequence_number(sequence_number)));
        }
    }

    /// Get the latest checkpoint sequence number
    pub fn get_latest_checkpoint_sequence_number(&self) -> anyhow::Result<Option<u64>> {
        self.state_reader.get_latest_checkpoint_sequence_number()
    }

    pub fn get_latest_checkpoint(&self) -> anyhow::Result<CertifiedCheckpointSummary> {
        let seq = self
            .state_reader
            .get_latest_checkpoint_sequence_number()?
            .ok_or_else(|| {
                anyhow::anyhow!("Unable to determine current epoch: no checkpoints available")
            })?;
        self.state_reader
            .get_checkpoint_summary(seq)?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint {seq} not found"))
    }

    pub fn get_lowest_available_checkpoint(&self) -> anyhow::Result<u64> {
        self.state_reader.get_lowest_available_checkpoint()
    }

    pub fn get_lowest_available_checkpoint_objects(&self) -> anyhow::Result<u64> {
        self.state_reader.get_lowest_available_checkpoint_objects()
    }

    pub fn get_object(&self, object_id: &ObjectID) -> anyhow::Result<Option<Object>> {
        self.state_reader.get_object(object_id)
    }

    pub fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> anyhow::Result<Option<Object>> {
        self.state_reader.get_object_by_key(object_id, version)
    }

    pub fn get_committee(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<Arc<iota_types::committee::Committee>>> {
        self.state_reader.get_committee(epoch)
    }

    pub fn get_system_state(
        &self,
    ) -> anyhow::Result<iota_types::iota_system_state::IotaSystemState> {
        self.state_reader.get_system_state()
    }

    pub fn get_system_state_summary(
        &self,
    ) -> anyhow::Result<
        iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    > {
        use iota_types::iota_system_state::IotaSystemStateTrait;

        let system_state = self.get_system_state()?;
        let summary = system_state.into_iota_system_state_summary();

        Ok(summary)
    }

    pub fn get_epoch_info(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<iota_types::storage::EpochInfo>> {
        self.state_reader.get_epoch_info(epoch)
    }

    pub fn get_type_layout(
        &self,
        type_tag: &iota_types::TypeTag,
    ) -> anyhow::Result<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        self.state_reader.get_type_layout(type_tag)
    }

    /// Iterate over objects owned by an account address.
    ///
    /// When the `owner_v2` backfill has not yet completed, falls back to the
    /// legacy `owner` table.
    ///
    /// The cursor is exclusive: items *after* the cursor position are returned.
    pub fn account_owned_objects_info_iter_v2(
        &self,
        owner: iota_types::base_types::IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = OwnedObjectV2IterItem> + '_>> {
        if self.state_reader.is_owner_v2_index_ready() {
            let skip = usize::from(cursor.is_some());
            let iter =
                self.state_reader
                    .account_owned_objects_info_iter_v2(owner, cursor, object_type)?;
            Ok(Box::new(iter.skip(skip)))
        } else {
            // Fallback: owner_v2 backfill in progress — use legacy owner table.
            // V1 natively supports cursor-based seeking by ObjectID.
            let cursor_id = cursor.map(|c| c.object_id);
            let skip = usize::from(cursor_id.is_some());
            let iter =
                self.state_reader
                    .account_owned_objects_info_iter(owner, cursor_id, object_type)?;
            // Wrap v1 items with a dummy cursor (only object_id is meaningful
            // for v1; the other fields are unused if v1 is still active on the
            // next request).
            let iter = iter.map(|result| {
                result.map(|info| {
                    let c = OwnedObjectV2Cursor {
                        object_type_identifier: 0,
                        object_type_params: 0,
                        inverted_balance: None,
                        object_id: info.object_id,
                    };
                    (info, c)
                })
            });
            Ok(Box::new(iter.skip(skip)))
        }
    }

    /// Iterate over dynamic fields of a parent object.
    ///
    /// When `cursor` is `Some`, the cursor item itself is automatically skipped
    /// so callers get items *after* the cursor (exclusive lower bound).
    pub fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = DynamicFieldIterItem> + '_>> {
        let skip = usize::from(cursor.is_some());
        let iter = self.state_reader.dynamic_field_iter(parent, cursor)?;
        Ok(Box::new(iter.skip(skip)))
    }

    /// Get unified coin info from the `coin_v2` table.
    ///
    /// When the `coin_v2` backfill has not yet completed, falls back to the
    /// legacy `coin` table and returns `regulated_available = false` so
    /// callers know that `regulated_coin_metadata_object_id` is absent due
    /// to the backfill rather than the coin being unregulated.
    pub fn get_coin_v2_info(
        &self,
        coin_type: &move_core_types::language_storage::StructTag,
    ) -> Result<(Option<iota_types::storage::CoinInfoV2>, bool), crate::error::RpcError> {
        if self.state_reader.is_coin_v2_index_ready() {
            let info = self
                .state_reader
                .get_coin_v2_info(coin_type)
                .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
            Ok((info, true))
        } else {
            // Fallback: coin_v2 backfill in progress — serve base coin info
            // from the legacy table; regulated metadata is unavailable.
            let info = self
                .state_reader
                .get_coin_info(coin_type)
                .map_err(|e| crate::error::RpcError::internal().with_context(e))?
                .map(|ci| iota_types::storage::CoinInfoV2 {
                    coin_metadata_object_id: ci.coin_metadata_object_id,
                    treasury_object_id: ci.treasury_object_id,
                    regulated_coin_metadata_object_id: None,
                });
            Ok((info, false))
        }
    }

    /// Iterate over all versions of a package by its original package ID.
    ///
    /// Returns `Err(IndexBackfillInProgressError)` when the backfill has not
    /// yet completed so callers receive a retryable `Unavailable` gRPC status.
    pub fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = PackageVersionIterItem> + '_>, crate::error::RpcError> {
        if !self.state_reader.is_package_version_index_ready() {
            return Err(crate::error::IndexBackfillInProgressError {
                index_name: "package_version",
            }
            .into());
        }
        let skip = usize::from(cursor.is_some());
        let iter = self
            .state_reader
            .package_versions_iter(original_package_id, cursor)
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
        Ok(Box::new(iter.skip(skip)))
    }

    /// Generic stream implementation for checkpoints
    fn create_generic_checkpoint_stream<T, S, R>(
        &self,
        mut rx: Receiver<Arc<T>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        cancellation_token: CancellationToken,
        data_type_name: &'static str,
        fetch_historical: impl Fn(Arc<dyn GrpcStateReader>, u64) -> Result<Option<Arc<S>>, Status>
        + Send,
        get_sequence_number_live: impl Fn(&Arc<T>) -> u64 + Send,
        process_item_historical: impl Fn(
            Arc<S>,
        ) -> std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<R, Status>> + Send>,
        > + Send,
        process_item_live: impl Fn(
            Arc<T>,
        ) -> std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<R, Status>> + Send>,
        > + Send,
    ) -> impl futures::Stream<Item = Result<R, Status>> + Send
    where
        T: Send + Sync + 'static,
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        let state_reader = self.state_reader.clone();
        async_stream::try_stream! {
            let mut latest = state_reader
                .get_latest_checkpoint_sequence_number()
                .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?
                .unwrap_or(0);
            debug!("[profile][grpc] Latest checkpoint index: {latest}.");
            let (mut start, end) = match (start_sequence_number, end_sequence_number) {
                (None, None) => (latest, u64::MAX),
                (None, Some(end)) => (end, end),
                (Some(start), None) => (start, u64::MAX),
                (Some(start), Some(end)) => (start, end),
            };

            while start <= end {
                // Try fetching historical data from the DB first
                if start <= latest {
                    // Check if the checkpoint has been pruned since we started
                    // (e.g. genesis checkpoint 0 is always in DB but may be
                    // below the pruning watermark).
                    let lowest_available = state_reader
                        .get_lowest_available_checkpoint()
                        .map_err(|e| Status::internal(format!("Failed to get lowest available checkpoint: {e}")))?;
                    if start < lowest_available {
                        Err(Status::not_found(format!(
                            "Checkpoint {data_type_name} {start} is below the lowest available checkpoint {lowest_available}"
                        )))?;
                    }

                    match fetch_historical(state_reader.clone(), start)? {
                        Some(item) => {
                            debug!("[profile][grpc] Fetched checkpoint {data_type_name} for index {start} from DB.");

                            // Process the item and yield all results
                            let mut item_stream = process_item_historical(item);
                            while let Some(result) = item_stream.next().await {
                                yield result?;
                            }

                            if start == end {
                                break;
                            }
                            start += 1;
                            continue;
                        }
                        None => {
                            Err(Status::not_found(format!("Historical checkpoint {data_type_name} missing/pruned: index={start} latest={latest}.")))?;
                        }
                    }
                }

                // Live phase - wait for broadcast or cancellation
                let item_result = tokio::select! {
                    recv_result = rx.recv() => Some(recv_result),
                    _ = cancellation_token.cancelled() => {
                        debug!("[profile][grpc] Checkpoint {data_type_name} stream cancelled");
                        None
                    }
                };

                match item_result {
                    Some(Ok(item)) => {
                        debug!("[profile][grpc] Get checkpoint {data_type_name} for index {} from broadcast channel", get_sequence_number_live(&item));
                        let sequence_number = get_sequence_number_live(&item);
                        if start == sequence_number {
                            // Process the item and yield all results
                            let mut item_stream = process_item_live(item);
                            while let Some(result) = item_stream.next().await {
                                yield result?;
                            }

                            if start == end {
                                break;
                            }
                            start += 1;
                            continue;
                        }
                        // else item sequence doesn't match, drop it and continue
                    }
                    Some(Err(RecvError::Lagged(_))) => {
                        // continue, lagged item should be picked up from history DB
                    }
                    Some(Err(RecvError::Closed)) => {
                        Err(Status::internal(format!("Checkpoint {data_type_name} channel closed.")))?;
                        break;
                    }
                    None => {
                        // Cancellation was triggered
                        break;
                    }
                }
                latest = state_reader
                    .get_latest_checkpoint_sequence_number()
                    .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?
                    .unwrap_or(start);
                debug!("[profile][grpc] Updating latest checkpoint index to {latest}.");
            }
        }
    }

    /// Lightweight check to determine if a checkpoint has any matching data
    /// without performing full SDK conversion or Merge operations.
    /// Returns true on first match (OR semantics when both filters are set).
    async fn has_matching_data<S>(
        transaction_stream: S,
        transaction_filter: &Option<crate::transaction_filter::TransactionFilter>,
        event_filter: &Option<crate::event_filter::EventFilter>,
    ) -> Result<bool, Status>
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        let mut transaction_stream = std::pin::pin!(transaction_stream);
        while let Some(result) = transaction_stream.next().await {
            let checkpoint_transaction =
                result.map_err(|e| Status::internal(format!("failed to read transaction: {e}")))?;

            if let Some(ref tx_filter) = transaction_filter {
                if tx_filter.matches_transaction(&checkpoint_transaction) {
                    return Ok(true);
                }
            }

            if let Some(ref evt_filter) = event_filter {
                if let Some(ref tx_events) = checkpoint_transaction.events {
                    for event in &tx_events.data {
                        if evt_filter.matches_event(event) {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    /// Tests whether any transaction in a checkpoint matches the active
    /// filters (transaction and/or event).
    ///
    /// Returns [`FilterCheckResult::Matched`] if at least one transaction
    /// matches, signalling that the checkpoint should be fully processed.
    /// Returns [`FilterCheckResult::Skipped`] otherwise, attaching a
    /// progress heartbeat when `progress_interval` has elapsed since the
    /// last emitted message.
    async fn match_checkpoint_filter_or_report_progress<S>(
        transaction_stream: S,
        transaction_filter: &Option<crate::transaction_filter::TransactionFilter>,
        event_filter: &Option<crate::event_filter::EventFilter>,
        last_msg_time: &std::sync::Mutex<tokio::time::Instant>,
        progress_interval: std::time::Duration,
        seq: u64,
    ) -> Result<FilterCheckResult, Status>
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        if Self::has_matching_data(transaction_stream, transaction_filter, event_filter).await? {
            *last_msg_time.lock().unwrap() = tokio::time::Instant::now();
            Ok(FilterCheckResult::Matched)
        } else {
            let progress = {
                let mut guard = last_msg_time.lock().unwrap();
                if guard.elapsed() >= progress_interval {
                    *guard = tokio::time::Instant::now();
                    Some(
                        grpc_ledger_service::CheckpointData::default().with_progress(
                            Progress::default().with_latest_scanned_sequence_number(seq),
                        ),
                    )
                } else {
                    None
                }
            };
            Ok(FilterCheckResult::Skipped(progress))
        }
    }

    /// Create a checkpoint stream implementation
    pub fn create_checkpoint_data_stream(
        &self,
        rx: Receiver<Arc<IotaTypesCheckpointData>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        checkpoint_mask: FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: u32,
        cancellation_token: CancellationToken,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
        filter_checkpoints: bool,
        progress_interval: std::time::Duration,
    ) -> Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send + Unpin> {
        let reader = self.clone();
        let state_reader_clone = self.state_reader.clone();

        // Shared timer for progress messages (used only when filter_checkpoints is
        // true)
        let last_message_time = Arc::new(std::sync::Mutex::new(tokio::time::Instant::now()));

        // Clone for closures
        let checkpoint_mask_historical = checkpoint_mask.clone();
        let transactions_mask_historical = transactions_mask.clone();
        let events_mask_historical = events_mask.clone();
        let transaction_filter_historical = transaction_filter.clone();
        let event_filter_historical = event_filter.clone();

        Box::new(Box::pin(reader.create_generic_checkpoint_stream(
            rx,
            start_sequence_number,
            end_sequence_number,
            cancellation_token,
            "data",
            // Historical data fetcher - returns (summary, contents)
            |reader, seq| {
                reader
                    .get_checkpoint_summary_and_contents(seq)
                    .map(|opt| opt.map(Arc::new))
                    .map_err(|e| {
                        Status::internal(format!("Failed to get checkpoint {seq}: {e}"))
                    })
            },
            |item| *item.checkpoint_summary.sequence_number(),
            // Historical data processor - uses transaction stream from DB
            {
                let state_reader_historical = state_reader_clone.clone();
                let last_message_time_historical = last_message_time.clone();
                move |item: Arc<(CertifiedCheckpointSummary, CheckpointContents)>| {
                    let state_reader_inner = state_reader_historical.clone();
                    let checkpoint_summary = item.0.clone();
                    let checkpoint_contents = item.1.clone();
                    let cp_mask = checkpoint_mask_historical.clone();
                    let tx_mask = transactions_mask_historical.clone();
                    let ev_mask = events_mask_historical.clone();
                    let tx_filter = transaction_filter_historical.clone();
                    let ev_filter = event_filter_historical.clone();
                    let last_msg_time = last_message_time_historical.clone();
                    {
                        Box::pin(async_stream::stream! {
                            let seq = checkpoint_summary.data().sequence_number;

                            // Pass 1: lightweight filter check when filter_checkpoints is enabled
                            if filter_checkpoints {
                                let scan_stream = state_reader_inner.stream_checkpoint_transactions(checkpoint_contents.clone());
                                match Self::match_checkpoint_filter_or_report_progress(
                                    scan_stream,
                                    &tx_filter,
                                    &ev_filter,
                                    &last_msg_time,
                                    progress_interval,
                                    seq,
                                ).await? {
                                    FilterCheckResult::Matched => {}
                                    FilterCheckResult::Skipped(progress) => {
                                        if let Some(msg) = progress {
                                            yield Ok(msg);
                                        }

                                        // no filter match, skip processing this checkpoint
                                        return;
                                    }
                                }
                            }

                            // Pass 2 (or normal mode): full processing
                            let transaction_stream = state_reader_inner.stream_checkpoint_transactions(checkpoint_contents.clone());
                            let mut stream = Box::pin(Self::create_checkpoint_messages_stream(
                                checkpoint_summary,
                                checkpoint_contents,
                                transaction_stream,
                                &cp_mask,
                                tx_mask,
                                ev_mask,
                                max_message_size_bytes as usize,
                                tx_filter,
                                ev_filter,
                            ));

                            while let Some(item) = stream.next().await {
                                yield item;
                            }
                        })
                    }
                }
            },
            // Live data processor - extracts transactions from CheckpointData
            {
                let last_message_time_live = last_message_time;
                move |item: Arc<IotaTypesCheckpointData>| {
                    let cp_mask = checkpoint_mask.clone();
                    let tx_mask = transactions_mask.clone();
                    let ev_mask = events_mask.clone();
                    let tx_filter = transaction_filter.clone();
                    let ev_filter = event_filter.clone();
                    let last_msg_time = last_message_time_live.clone();
                    Box::pin(async_stream::stream! {
                        let seq = *item.checkpoint_summary.sequence_number();

                        // Pass 1: lightweight filter check when filter_checkpoints is enabled
                        if filter_checkpoints {
                            // Convert the transactions Vec to a stream
                            let scan_stream = futures::stream::iter(
                                item.transactions.clone().into_iter().map(Ok)
                            );
                            match Self::match_checkpoint_filter_or_report_progress(
                                scan_stream,
                                &tx_filter,
                                &ev_filter,
                                &last_msg_time,
                                progress_interval,
                                seq,
                            ).await? {
                                FilterCheckResult::Matched => {}
                                FilterCheckResult::Skipped(progress) => {
                                    if let Some(msg) = progress {
                                        yield Ok(msg);
                                    }

                                    // no filter match, skip processing this checkpoint
                                    return;
                                }
                            }
                        }

                        // Pass 2 (or normal mode): full processing

                        // Convert the transactions Vec to a stream
                        let transaction_stream = futures::stream::iter(
                            item.transactions.clone().into_iter().map(Ok)
                        );

                        // Use the unified streaming function
                        let mut stream = Box::pin(Self::create_checkpoint_messages_stream(
                            item.checkpoint_summary.clone(),
                            item.checkpoint_contents.clone(),
                            transaction_stream,
                            &cp_mask,
                            tx_mask,
                            ev_mask,
                            max_message_size_bytes as usize,
                            tx_filter,
                            ev_filter,
                        ));

                        while let Some(item) = stream.next().await {
                            yield item;
                        }
                    })
                }
            },
        )))
    }

    /// Get transaction data for a single transaction digest.
    ///
    /// Only fetches data from storage when indicated by `fields`, enabling
    /// callers to skip unnecessary reads. Effects are fetched when any of
    /// effects/events/input_objects/output_objects are requested since they
    /// provide the digests and references needed to fetch those fields.
    #[tracing::instrument(skip(self))]
    pub fn get_transaction_read(
        &self,
        digest: &TransactionDigest,
        fields: &TransactionReadFields,
    ) -> Result<TransactionReadData, crate::error::RpcError> {
        let (transaction, signatures) = if fields.include_transaction || fields.include_signatures {
            // Get the transaction if transaction data or signatures are requested
            let transaction = self
                .state_reader
                .get_transaction(digest)?
                .ok_or(crate::error::TransactionNotFoundError(*digest))?;

            let transaction_data = fields
                .include_transaction
                .then(|| transaction.transaction_data().clone().try_into())
                .transpose()?;

            let signatures_data = fields
                .include_signatures
                .then(|| {
                    transaction
                        .tx_signatures()
                        .iter()
                        .map(|sig| sig.clone().try_into())
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;

            (transaction_data, signatures_data)
        } else {
            (None, None)
        };

        let (checkpoint, timestamp_ms) = if fields.include_checkpoint || fields.include_timestamp {
            let checkpoint = self.state_reader.get_transaction_checkpoint(digest)?;

            let timestamp_ms = if fields.include_timestamp {
                match checkpoint {
                    Some(checkpoint_seq) => {
                        let summary = self
                            .state_reader
                            .get_checkpoint_summary(checkpoint_seq)?
                            .ok_or_else(|| {
                                crate::error::RpcError::new(
                                    tonic::Code::Internal,
                                    format!(
                                        "Checkpoint summary {} not found for transaction {}",
                                        checkpoint_seq, digest
                                    ),
                                )
                            })?;
                        Some(summary.data().timestamp_ms)
                    }
                    // Transaction not yet included in a checkpoint
                    None => None,
                }
            } else {
                None
            };
            (checkpoint, timestamp_ms)
        } else {
            (None, None)
        };

        // Get the effects if any of the following are requested: effects, events,
        // checkpoint/timestamp, input/output objects
        let (effects, events, input_objects, output_objects) = if fields.include_effects
            || fields.include_events
            || fields.include_input_objects
            || fields.include_output_objects
        {
            // Effects are required for events and input/output objects, so we fetch them if
            // any of those are requested
            let effects = self
                .state_reader
                .get_transaction_effects(digest)?
                .ok_or(crate::error::TransactionNotFoundError(*digest))?;

            // Get events only if requested
            let events = if fields.include_events {
                match effects.events_digest() {
                    Some(_) => self.state_reader.get_transaction_events(digest)?,
                    None => None,
                }
            } else {
                None
            };

            // Get input objects only if requested
            let input_objects = if fields.include_input_objects {
                let mut objects = Vec::new();
                for (object_id, version) in effects.modified_at_versions() {
                    if let Some(obj) = self.state_reader.get_object_by_key(&object_id, version)? {
                        objects.push(obj);
                    }
                }
                Some(objects)
            } else {
                None
            };

            // Get output objects only if requested
            let output_objects = if fields.include_output_objects {
                let mut objects = Vec::new();
                for ((object_id, version, _digest), _owner) in effects
                    .created()
                    .into_iter()
                    .chain(effects.mutated())
                    .chain(effects.unwrapped())
                {
                    if let Some(obj) = self.state_reader.get_object_by_key(&object_id, version)? {
                        objects.push(obj);
                    }
                }
                Some(objects)
            } else {
                None
            };

            (Some(effects), events, input_objects, output_objects)
        } else {
            // If none of the above are requested, we can skip fetching effects entirely
            (None, None, None, None)
        };

        Ok(TransactionReadData {
            digest: *digest,
            transaction,
            signatures,
            effects,
            events,
            checkpoint,
            timestamp_ms,
            input_objects,
            output_objects,
        })
    }
}

/// Internal struct to hold all transaction-related data fetched from storage.
///
/// This struct holds owned data from storage, which is then converted to
/// `iota-sdk-types` types and used with `Merge` trait to populate gRPC
/// responses.
///
/// Optional fields are `None` when the corresponding data was not requested
/// via `TransactionReadFields`, meaning the storage read was skipped entirely.
#[derive(Debug)]
pub struct TransactionReadData {
    pub digest: TransactionDigest,
    pub transaction: Option<iota_sdk_types::transaction::Transaction>,
    pub signatures: Option<Vec<iota_sdk_types::UserSignature>>,
    pub effects: Option<TransactionEffects>,
    pub events: Option<TransactionEvents>,
    pub checkpoint: Option<u64>,
    pub timestamp_ms: Option<u64>,
    pub input_objects: Option<Vec<Object>>,
    pub output_objects: Option<Vec<Object>>,
}

/// Wrapper type that includes checkpoint context for a CheckpointTransaction.
#[derive(Debug, Clone)]
pub struct CheckpointTransactionWithContext {
    pub transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
    pub checkpoint_sequence_number: Option<u64>,
    pub checkpoint_timestamp_ms: Option<u64>,
}

impl CheckpointTransactionWithContext {
    pub fn new(
        transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
        checkpoint_sequence_number: Option<u64>,
        checkpoint_timestamp_ms: Option<u64>,
    ) -> Self {
        Self {
            transaction,
            checkpoint_sequence_number,
            checkpoint_timestamp_ms,
        }
    }
}

impl Merge<CheckpointTransactionWithContext>
    for iota_grpc_types::v1::transaction::ExecutedTransaction
{
    type Error = RpcError;

    fn merge(
        &mut self,
        source: CheckpointTransactionWithContext,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        if let Some(submask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            self.transaction = Some(iota_grpc_types::v1::transaction::Transaction::merge_from(
                source.transaction.transaction.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = Some(iota_grpc_types::v1::signatures::UserSignatures::merge_from(
                source.transaction.transaction.clone(),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            self.effects = Some(
                iota_grpc_types::v1::transaction::TransactionEffects::merge_from(
                    source.transaction.effects.clone(),
                    &submask,
                )?,
            );
        }

        if let Some(submask) = mask.subtree(Self::EVENTS_FIELD.name) {
            // Use unwrap_or_default so that when no events were emitted we still
            // compute a real digest (hash of the empty list) and populate an empty
            // events vec — to distinguish between "no events" and "events
            // not requested in the mask".
            self.events = Some(grpc_transaction::TransactionEvents::merge_from(
                source.transaction.events.unwrap_or_default(),
                &submask,
            )?);
        }

        // Set checkpoint sequence number if requested
        if mask.contains(Self::CHECKPOINT_FIELD.name) {
            self.checkpoint = source.checkpoint_sequence_number;
        }

        // Set checkpoint timestamp if requested
        if mask.contains(Self::TIMESTAMP_FIELD.name) {
            self.timestamp = source.checkpoint_timestamp_ms.map(timestamp_ms_to_proto);
        }

        if let Some(submask) = mask.subtree(Self::INPUT_OBJECTS_FIELD.name) {
            self.input_objects = Some(iota_grpc_types::v1::object::Objects::merge_from(
                Some(source.transaction.input_objects),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            self.output_objects = Some(iota_grpc_types::v1::object::Objects::merge_from(
                Some(source.transaction.output_objects),
                &submask,
            )?);
        }

        Ok(())
    }
}
