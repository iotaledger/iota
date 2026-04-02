// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared test utilities for iota-grpc-server integration tests.
//! Not every test binary uses every item.
#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use iota_config::{local_ip_utils, node::GrpcApiConfig};
use iota_grpc_server::{GrpcReader, GrpcServerHandle, start_grpc_server};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    crypto::AuthorityStrongQuorumSignInfo,
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber, CheckpointSummary,
    },
    object::Object,
    transaction::VerifiedTransaction,
};

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

/// Create a mock `CertifiedCheckpointSummary` for the given sequence number.
pub fn mock_summary(
    sequence_number: u64,
    contents: &CheckpointContents,
) -> CertifiedCheckpointSummary {
    let summary = CheckpointSummary {
        epoch: 0,
        sequence_number,
        network_total_transactions: 0,
        content_digest: *contents.digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: Default::default(),
        timestamp_ms: 0,
        checkpoint_commitments: vec![],
        end_of_epoch_data: None,
        version_specific_data: vec![],
    };
    let sig = AuthorityStrongQuorumSignInfo {
        epoch: 0,
        signature: Default::default(),
        signers_map: Default::default(),
    };
    CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig)
}

// ---------------------------------------------------------------------------
// MockGrpcStateReader
// ---------------------------------------------------------------------------

/// A configurable mock `GrpcStateReader` for integration tests.
///
/// All fields default to empty / `None`. Tests set only the fields they need.
///
/// # Checkpoint modes
///
/// - **Fixed mode** (set `summary` + `contents` + `checkpoint_transactions`):
///   every sequence number returns the same summary/contents/transactions. Used
///   by the boundary-size chunking tests.
///
/// - **Set mode** (set `checkpoints`): only sequence numbers present in the set
///   are "available". A mock summary is generated on the fly for each. Used by
///   the checkpoint-streaming integration tests.
#[derive(Default)]
pub struct MockGrpcStateReader {
    // -- Fixed checkpoint mode --
    pub summary: Option<CertifiedCheckpointSummary>,
    pub contents: Option<CheckpointContents>,
    pub checkpoint_transactions: Vec<CheckpointTransaction>,

    // -- Set-based checkpoint mode (for streaming tests) --
    pub checkpoints: Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
    /// Sequence numbers whose `stream_checkpoint_transactions` should return
    /// `large_checkpoint_transactions` instead of the default empty vec.
    pub large_checkpoints: Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
    /// Transactions returned for "large" checkpoints.
    pub large_checkpoint_transactions: Vec<CheckpointTransaction>,

    // -- Objects --
    pub objects: HashMap<ObjectID, Object>,

    // -- Owned objects (for list_owned_objects pagination tests) --
    /// Pre-sorted in v2 key order. The iterator respects cursor-based seeking.
    pub owned_objects: Vec<(
        iota_types::storage::AccountOwnedObjectInfo,
        iota_types::storage::OwnedObjectV2Cursor,
    )>,

    // -- Transactions --
    pub transactions: HashMap<TransactionDigest, Arc<VerifiedTransaction>>,
    pub effects: HashMap<TransactionDigest, TransactionEffects>,

    // -- Pruning --
    pub lowest_available_checkpoint: u64,
}

/// Shared empty contents used when generating on-the-fly summaries.
pub(crate) static EMPTY_CHECKPOINT_CONTENTS: std::sync::LazyLock<CheckpointContents> =
    std::sync::LazyLock::new(|| CheckpointContents::new_with_digests_only_for_tests(vec![]));

impl MockGrpcStateReader {
    /// Create a `MockGrpcStateReader` in set mode from a checkpoint range.
    pub fn new_from_iter(iter: impl Iterator<Item = u64>) -> Self {
        Self {
            checkpoints: Arc::new(Mutex::new(iter.collect())),
            ..Default::default()
        }
    }

    /// Whether we are in "set mode" (at least one checkpoint in the set).
    fn is_set_mode(&self) -> bool {
        !self.checkpoints.lock().unwrap().is_empty()
    }

    /// Mark a checkpoint sequence number as using large data.
    pub fn mark_checkpoint_as_large(&self, seq: CheckpointSequenceNumber) {
        self.large_checkpoints.lock().unwrap().insert(seq);
    }

    fn is_large_checkpoint(&self, seq: CheckpointSequenceNumber) -> bool {
        self.large_checkpoints.lock().unwrap().contains(&seq)
    }

    /// Builder: set the lowest available checkpoint (for pruning tests).
    pub fn with_lowest_available_checkpoint(mut self, seq: u64) -> Self {
        self.lowest_available_checkpoint = seq;
        self
    }
}

impl iota_grpc_server::GrpcStateReader for MockGrpcStateReader {
    fn get_chain_identifier(&self) -> anyhow::Result<iota_types::digests::ChainIdentifier> {
        Ok(iota_types::digests::ChainIdentifier::default())
    }

    fn get_latest_checkpoint_sequence_number(&self) -> anyhow::Result<Option<u64>> {
        if self.is_set_mode() {
            Ok(self.checkpoints.lock().unwrap().iter().max().copied())
        } else {
            // Fixed mode: assume checkpoint 0 exists if summary is set.
            Ok(self.summary.as_ref().map(|s| s.sequence_number))
        }
    }

    fn get_checkpoint_summary(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        if self.is_set_mode() {
            let guard = self.checkpoints.lock().unwrap();
            if guard.contains(&seq) {
                Ok(Some(mock_summary(seq, &EMPTY_CHECKPOINT_CONTENTS)))
            } else {
                Ok(None)
            }
        } else {
            Ok(self.summary.clone())
        }
    }

    fn get_checkpoint_sequence_number_by_digest(
        &self,
        _digest: &iota_types::digests::CheckpointDigest,
    ) -> anyhow::Result<Option<u64>> {
        Ok(None)
    }

    fn get_checkpoint_data(&self, seq: u64) -> anyhow::Result<Option<CheckpointData>> {
        if self.is_set_mode() {
            let guard = self.checkpoints.lock().unwrap();
            if !guard.contains(&seq) {
                return Ok(None);
            }
            drop(guard);

            let transactions = if self.is_large_checkpoint(seq) {
                self.large_checkpoint_transactions.clone()
            } else {
                vec![]
            };
            Ok(Some(CheckpointData {
                checkpoint_summary: mock_summary(seq, &EMPTY_CHECKPOINT_CONTENTS),
                checkpoint_contents: EMPTY_CHECKPOINT_CONTENTS.clone(),
                transactions,
            }))
        } else {
            Ok(None)
        }
    }

    fn get_checkpoint_summary_and_contents(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<(CertifiedCheckpointSummary, CheckpointContents)>> {
        if self.is_set_mode() {
            let guard = self.checkpoints.lock().unwrap();
            if guard.contains(&seq) {
                Ok(Some((
                    mock_summary(seq, &EMPTY_CHECKPOINT_CONTENTS),
                    EMPTY_CHECKPOINT_CONTENTS.clone(),
                )))
            } else {
                Ok(None)
            }
        } else {
            match (&self.summary, &self.contents) {
                (Some(summary), Some(contents)) => Ok(Some((summary.clone(), contents.clone()))),
                _ => Ok(None),
            }
        }
    }

    fn stream_checkpoint_transactions(
        &self,
        _checkpoint_contents: CheckpointContents,
    ) -> std::pin::Pin<
        Box<dyn futures::Stream<Item = anyhow::Result<CheckpointTransaction>> + Send + '_>,
    > {
        // In set mode with large checkpoints, return large transactions.
        // Otherwise return the fixed checkpoint_transactions (may be empty).
        let transactions = self.checkpoint_transactions.clone();
        Box::pin(async_stream::stream! {
            for tx in transactions {
                yield Ok(tx);
            }
        })
    }

    fn get_epoch_last_checkpoint(
        &self,
        _epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        Ok(None)
    }

    fn get_lowest_available_checkpoint(&self) -> anyhow::Result<u64> {
        Ok(self.lowest_available_checkpoint)
    }

    fn get_lowest_available_checkpoint_objects(&self) -> anyhow::Result<u64> {
        Ok(0)
    }

    fn get_object(&self, object_id: &ObjectID) -> anyhow::Result<Option<Object>> {
        Ok(self.objects.get(object_id).cloned())
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        _version: SequenceNumber,
    ) -> anyhow::Result<Option<Object>> {
        Ok(self.objects.get(object_id).cloned())
    }

    fn get_committee(
        &self,
        _epoch: u64,
    ) -> anyhow::Result<Option<Arc<iota_types::committee::Committee>>> {
        Ok(None)
    }

    fn get_system_state(&self) -> anyhow::Result<iota_types::iota_system_state::IotaSystemState> {
        unimplemented!()
    }

    fn get_epoch_info(
        &self,
        _epoch: u64,
    ) -> anyhow::Result<Option<iota_types::storage::EpochInfo>> {
        Ok(None)
    }

    fn get_type_layout(
        &self,
        _type_tag: &iota_types::TypeTag,
    ) -> anyhow::Result<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        Ok(None)
    }

    fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<Arc<VerifiedTransaction>>> {
        Ok(self.transactions.get(digest).cloned())
    }

    fn get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> anyhow::Result<Option<TransactionEffects>> {
        Ok(self.effects.get(digest).cloned())
    }

    fn get_transaction_events(
        &self,
        _digest: &TransactionDigest,
    ) -> anyhow::Result<Option<TransactionEvents>> {
        Ok(None)
    }

    fn get_transaction_checkpoint(
        &self,
        _digest: &TransactionDigest,
    ) -> anyhow::Result<Option<u64>> {
        Ok(None)
    }

    fn account_owned_objects_info_iter(
        &self,
        _owner: iota_types::base_types::IotaAddress,
        _cursor: Option<ObjectID>,
        _object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = iota_grpc_server::OwnedObjectIterItem> + '_>> {
        Ok(Box::new(std::iter::empty()))
    }

    fn account_owned_objects_info_iter_v2(
        &self,
        owner: iota_types::base_types::IotaAddress,
        cursor: Option<&iota_types::storage::OwnedObjectV2Cursor>,
        object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = iota_grpc_server::OwnedObjectV2IterItem> + '_>>
    {
        // Find the start index: if cursor is provided, seek to its position
        // (inclusive — the GrpcReader wrapper handles skip(1)).
        let start = if let Some(c) = cursor {
            self.owned_objects
                .iter()
                .position(|(_, oc)| {
                    (
                        oc.object_type_identifier,
                        oc.object_type_params,
                        oc.inverted_balance,
                        oc.object_id,
                    ) >= (
                        c.object_type_identifier,
                        c.object_type_params,
                        c.inverted_balance,
                        c.object_id,
                    )
                })
                .unwrap_or(self.owned_objects.len())
        } else {
            0
        };

        let owner_filter = owner;
        let type_filter = object_type;
        let iter = self.owned_objects[start..]
            .iter()
            .filter(move |(info, _)| {
                info.owner == owner_filter
                    && type_filter.as_ref().is_none_or(|t| {
                        move_core_types::language_storage::StructTag::from(info.type_.clone()) == *t
                    })
            })
            .map(|(info, cursor)| Ok((info.clone(), cursor.clone())));

        Ok(Box::new(iter))
    }

    fn dynamic_field_iter(
        &self,
        _parent: ObjectID,
        _cursor: Option<ObjectID>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = iota_grpc_server::DynamicFieldIterItem> + '_>> {
        Ok(Box::new(std::iter::empty()))
    }

    fn get_coin_info(
        &self,
        _coin_type: &move_core_types::language_storage::StructTag,
    ) -> anyhow::Result<Option<iota_types::storage::CoinInfo>> {
        Ok(None)
    }

    fn get_coin_v2_info(
        &self,
        _coin_type: &move_core_types::language_storage::StructTag,
    ) -> anyhow::Result<Option<iota_types::storage::CoinInfoV2>> {
        Ok(None)
    }

    fn package_versions_iter(
        &self,
        _original_package_id: ObjectID,
        _cursor: Option<u64>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = iota_grpc_server::PackageVersionIterItem> + '_>>
    {
        Ok(Box::new(std::iter::empty()))
    }
}

// ---------------------------------------------------------------------------
// Server setup helpers
// ---------------------------------------------------------------------------

/// Start a gRPC server backed by the given `MockGrpcStateReader`.
///
/// Returns the server handle and the `GrpcReader` (callers may need it to
/// create different client types).
pub async fn start_test_server(
    state_reader: Arc<MockGrpcStateReader>,
    config_customizer: impl FnOnce(&mut GrpcApiConfig),
) -> (GrpcServerHandle, Arc<GrpcReader>) {
    let grpc_reader = Arc::new(GrpcReader::new(state_reader, Some("test".to_string())));
    let localhost = local_ip_utils::localhost_for_testing();
    let port = local_ip_utils::get_available_port(&localhost);
    let mut config = GrpcApiConfig {
        address: format!("{localhost}:{port}").parse().unwrap(),
        ..GrpcApiConfig::default()
    };
    config_customizer(&mut config);

    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let server_handle = start_grpc_server(
        grpc_reader.clone(),
        None,
        config,
        cancellation_token,
        iota_types::digests::ChainIdentifier::default(),
        None,
    )
    .await
    .expect("Failed to start gRPC server");

    (server_handle, grpc_reader)
}
