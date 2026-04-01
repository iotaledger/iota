// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC State Reader for Simulacrum
//!
//! This module provides a GrpcStateReader implementation that can read from
//! simulacrum state without requiring mutable access in most cases.

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_server::GrpcStateReader;
use iota_types::{
    TypeTag,
    base_types::{ObjectID, VersionNumber},
    committee::Committee,
    digests::{ChainIdentifier, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    iota_system_state::{IotaSystemState, IotaSystemStateTrait},
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents},
    object::Object,
    storage::{
        EpochInfo, ReadStore, RestStateReader, get_transaction_input_objects,
        get_transaction_output_objects,
    },
    transaction::VerifiedTransaction,
};
use move_core_types::annotated_value::MoveTypeLayout;

use crate::Simulacrum;

/// GrpcStateReader implementation that works with simulacrum
pub struct SimulacrumGrpcReader {
    simulacrum: Arc<Simulacrum>,
    chain_id: ChainIdentifier,
}

impl SimulacrumGrpcReader {
    pub fn new(simulacrum: Arc<Simulacrum>, chain_id: ChainIdentifier) -> Self {
        Self {
            simulacrum,
            chain_id,
        }
    }

    /// Try to get the system state for a specific epoch.
    /// This method retrieves historical system state data if available.
    fn get_system_state_for_epoch(&self, epoch: u64) -> Result<IotaSystemState> {
        self.simulacrum.with_store(|store| {
            // First try to get historical system state for the requested epoch
            if let Some(historical_state) = store.get_system_state_by_epoch(epoch) {
                return Ok(historical_state.clone());
            }

            // If we're asking for the current epoch, return current system state
            let current_system_state = store.get_system_state();
            if epoch == current_system_state.epoch() {
                return Ok(current_system_state);
            }

            // Historical system state not found
            Err(anyhow::anyhow!("Historical system state for epoch {} not available. System states are only stored when epochs end.", epoch))
        })
    }
}

impl GrpcStateReader for SimulacrumGrpcReader {
    fn get_chain_identifier(&self) -> Result<ChainIdentifier> {
        Ok(self.chain_id)
    }

    fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>> {
        Ok(self.simulacrum.with_store(|store| {
            store
                .get_highest_checkpoint()
                .map(|checkpoint| *checkpoint.sequence_number())
        }))
    }

    fn get_checkpoint_summary(&self, seq: u64) -> Result<Option<CertifiedCheckpointSummary>> {
        Ok(self.simulacrum.with_store(|store| {
            store
                .get_checkpoint_by_sequence_number(seq)
                .cloned()
                .map(CertifiedCheckpointSummary::from)
        }))
    }

    fn get_checkpoint_sequence_number_by_digest(
        &self,
        digest: &iota_types::digests::CheckpointDigest,
    ) -> Result<Option<u64>> {
        Ok(self.simulacrum.with_store(|store| {
            store
                .get_checkpoint_by_digest(digest)
                .map(|checkpoint| *checkpoint.sequence_number())
        }))
    }

    fn get_checkpoint_data(&self, seq: u64) -> Result<Option<CheckpointData>> {
        self.simulacrum
            .with_store(|store| match store.get_checkpoint_by_sequence_number(seq) {
                None => Ok(None),
                Some(checkpoint) => {
                    let Some(contents) = store
                        .get_checkpoint_contents(&checkpoint.content_digest)
                        .cloned()
                    else {
                        return Ok(None);
                    };
                    store
                        .try_get_checkpoint_data(checkpoint.clone(), contents)
                        .map(Some)
                }
            })
    }

    fn get_epoch_last_checkpoint(&self, epoch: u64) -> Result<Option<CertifiedCheckpointSummary>> {
        let summary = self.simulacrum.with_store(|store| {
            store
                .get_last_checkpoint_of_epoch(epoch)
                .and_then(|seq| store.get_checkpoint_by_sequence_number(seq).cloned())
                .map(CertifiedCheckpointSummary::from)
        });
        Ok(summary)
    }

    fn get_lowest_available_checkpoint(&self) -> Result<u64> {
        // Simulacrum starts from checkpoint 0
        Ok(0)
    }

    fn get_lowest_available_checkpoint_objects(&self) -> Result<u64> {
        // Simulacrum has all objects from the beginning
        Ok(0)
    }

    fn get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        Ok(self
            .simulacrum
            .with_store(|store| store.get_object(object_id).cloned()))
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        Ok(self
            .simulacrum
            .with_store(|store| store.get_object_at_version(object_id, version).cloned()))
    }

    fn get_committee(&self, epoch: u64) -> Result<Option<Arc<Committee>>> {
        Ok(self
            .simulacrum
            .with_store(|store| store.get_committee_by_epoch(epoch).cloned())
            .map(Arc::new))
    }

    fn get_system_state(&self) -> Result<IotaSystemState> {
        Ok(self.simulacrum.with_store(|store| store.get_system_state()))
    }

    fn get_epoch_info(&self, epoch: u64) -> Result<Option<EpochInfo>> {
        Ok(self.simulacrum.with_store(|store| {
            // Get the start checkpoint of the epoch
            let start_checkpoint_seq = if epoch != 0 {
                store
                    .get_last_checkpoint_of_epoch(epoch - 1)
                    .map(|seq| Some(seq + 1))
                    .unwrap_or(None)?
            } else {
                0
            };

            let start_checkpoint = store
                .get_checkpoint_by_sequence_number(start_checkpoint_seq)
                .cloned()?;

            // Try to get the system state for the specific epoch
            let system_state = self
                .get_system_state_for_epoch(epoch)
                .expect("valid system state should exist");

            // Try to get the next epoch's system state to determine if current epoch is
            // completed
            let (end_timestamp_ms, end_checkpoint) =
                if let Ok(next_epoch_state) = self.get_system_state_for_epoch(epoch + 1) {
                    (
                        Some(next_epoch_state.epoch_start_timestamp_ms()),
                        Some(
                            store
                                .get_last_checkpoint_of_epoch(epoch)
                                .expect("last checkpoint of completed epoch should exist"),
                        ),
                    )
                } else {
                    // Next epoch doesn't exist, so this epoch is current or incomplete
                    (None, None)
                };

            Some(EpochInfo {
                epoch,
                protocol_version: system_state.protocol_version(),
                start_timestamp_ms: start_checkpoint.data().timestamp_ms,
                end_timestamp_ms,
                start_checkpoint: start_checkpoint_seq,
                end_checkpoint,
                reference_gas_price: system_state.reference_gas_price(),
                system_state,
            })
        }))
    }

    fn get_type_layout(&self, type_tag: &TypeTag) -> Result<Option<MoveTypeLayout>> {
        self.simulacrum
            .get_type_layout(type_tag)
            .map_err(Into::into)
    }

    fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<Arc<VerifiedTransaction>>> {
        Ok(self
            .simulacrum
            .with_store(|store| store.get_transaction(digest).cloned().map(Arc::new)))
    }

    fn get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionEffects>> {
        Ok(self
            .simulacrum
            .with_store(|store| store.get_transaction_effects(digest).cloned()))
    }

    fn get_transaction_events(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionEvents>> {
        Ok(self
            .simulacrum
            .with_store(|store| store.get_transaction_events(digest).cloned()))
    }

    fn get_transaction_checkpoint(&self, digest: &TransactionDigest) -> Result<Option<u64>> {
        Ok(self.simulacrum.with_store(|store| {
            let highest_seq = store
                .get_highest_checkpoint()
                .map(|cp| *cp.sequence_number())?;

            // Search backwards from the highest checkpoint to find the transaction
            for seq in (0..=highest_seq).rev() {
                if let Some(checkpoint) = store.get_checkpoint_by_sequence_number(seq) {
                    if let Some(contents) =
                        store.get_checkpoint_contents(&checkpoint.content_digest)
                    {
                        // Check if this checkpoint contains the transaction
                        if contents
                            .iter()
                            .any(|exec_digests| exec_digests.transaction == *digest)
                        {
                            return Some(*checkpoint.sequence_number());
                        }
                    }
                }
            }
            None
        }))
    }

    fn get_checkpoint_summary_and_contents(
        &self,
        seq: u64,
    ) -> Result<Option<(CertifiedCheckpointSummary, CheckpointContents)>> {
        Ok(self.simulacrum.with_store(|store| {
            let checkpoint = store.get_checkpoint_by_sequence_number(seq).cloned()?;
            let contents = store
                .get_checkpoint_contents(&checkpoint.content_digest)
                .cloned()?;
            Some((CertifiedCheckpointSummary::from(checkpoint), contents))
        }))
    }

    fn account_owned_objects_info_iter(
        &self,
        _owner: iota_types::base_types::IotaAddress,
        _cursor: Option<ObjectID>,
        _object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> Result<Box<dyn Iterator<Item = iota_grpc_server::OwnedObjectIterItem> + '_>> {
        // Simulacrum does not have index support for owned objects
        Ok(Box::new(std::iter::empty()))
    }

    fn account_owned_objects_info_iter_v2(
        &self,
        _owner: iota_types::base_types::IotaAddress,
        _cursor: Option<&iota_types::storage::OwnedObjectV2Cursor>,
        _object_type: Option<move_core_types::language_storage::StructTag>,
    ) -> Result<Box<dyn Iterator<Item = iota_grpc_server::OwnedObjectV2IterItem> + '_>> {
        // Simulacrum does not have index support for owned objects
        Ok(Box::new(std::iter::empty()))
    }

    fn dynamic_field_iter(
        &self,
        _parent: ObjectID,
        _cursor: Option<ObjectID>,
    ) -> Result<Box<dyn Iterator<Item = iota_grpc_server::DynamicFieldIterItem> + '_>> {
        // Simulacrum does not have index support for dynamic fields
        Ok(Box::new(std::iter::empty()))
    }

    fn get_coin_info(
        &self,
        _coin_type: &move_core_types::language_storage::StructTag,
    ) -> Result<Option<iota_types::storage::CoinInfo>> {
        // Simulacrum does not have index support for coin info
        Ok(None)
    }

    fn get_coin_v2_info(
        &self,
        _coin_type: &move_core_types::language_storage::StructTag,
    ) -> Result<Option<iota_types::storage::CoinInfoV2>> {
        // Simulacrum does not have index support for coin_v2 info
        Ok(None)
    }

    fn package_versions_iter(
        &self,
        _original_package_id: ObjectID,
        _cursor: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = iota_grpc_server::PackageVersionIterItem> + '_>> {
        // Simulacrum does not have index support for package versions
        Ok(Box::new(std::iter::empty()))
    }

    fn stream_checkpoint_transactions(
        &self,
        checkpoint_contents: CheckpointContents,
    ) -> std::pin::Pin<
        Box<dyn futures::Stream<Item = anyhow::Result<CheckpointTransaction>> + Send + '_>,
    > {
        self.simulacrum.with_store(|store| {
            let transactions: Vec<anyhow::Result<CheckpointTransaction>> = checkpoint_contents
                .iter()
                .map(|exec_digests| {
                    let verified_transaction = store
                        .get_transaction(&exec_digests.transaction)
                        .ok_or_else(|| {
                            anyhow::anyhow!("Transaction not found: {}", exec_digests.transaction)
                        })?;
                    let transaction = verified_transaction.clone().into();
                    let effects = store
                        .get_transaction_effects(&exec_digests.transaction)
                        .ok_or_else(|| {
                            anyhow::anyhow!("Effects not found: {}", exec_digests.transaction)
                        })?
                        .clone();

                    // Get events from effects if they exist
                    let events = if effects.events_digest().is_some() {
                        store
                            .get_transaction_events(effects.transaction_digest())
                            .cloned()
                    } else {
                        None
                    };

                    // Extract input and output objects with proper error propagation
                    let input_objects = get_transaction_input_objects(store, &effects)?;
                    let output_objects = get_transaction_output_objects(store, &effects)?;

                    Ok(CheckpointTransaction {
                        transaction,
                        effects,
                        events,
                        input_objects,
                        output_objects,
                    })
                })
                .collect();

            Box::pin(futures::stream::iter(transactions))
        })
    }
}
