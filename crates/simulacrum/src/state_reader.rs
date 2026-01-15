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
    digests::{ChainIdentifier, TransactionDigest, TransactionEventsDigest},
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::CheckpointData,
    iota_system_state::{IotaSystemState, IotaSystemStateTrait},
    messages_checkpoint::CertifiedCheckpointSummary,
    object::Object,
    storage::{EpochInfo, ReadStore, RestStateReader},
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
}

impl GrpcStateReader for SimulacrumGrpcReader {
    fn get_chain_identifier(&self) -> Result<ChainIdentifier> {
        Ok(self.chain_id)
    }

    fn get_latest_checkpoint_sequence_number(&self) -> Option<u64> {
        self.simulacrum.with_store(|store| {
            store
                .get_highest_checkpoint()
                .map(|checkpoint| *checkpoint.sequence_number())
        })
    }

    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        self.simulacrum.with_store(|store| {
            store
                .get_checkpoint_by_sequence_number(seq)
                .cloned()
                .map(CertifiedCheckpointSummary::from)
        })
    }

    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        self.simulacrum
            .with_store(|store| match store.get_checkpoint_by_sequence_number(seq) {
                None => None,
                Some(checkpoint) => {
                    let contents = store
                        .get_checkpoint_contents(&checkpoint.content_digest)
                        .cloned()?;
                    store
                        .try_get_checkpoint_data(checkpoint.clone(), contents)
                        .ok()
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

    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.simulacrum
            .with_store(|store| store.get_object(object_id).cloned())
    }

    fn get_object_by_key(&self, object_id: &ObjectID, version: VersionNumber) -> Option<Object> {
        self.simulacrum
            .with_store(|store| store.get_object_at_version(object_id, version).cloned())
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

    fn get_epoch_info(&self, epoch: u64) -> Option<EpochInfo> {
        self.simulacrum.with_store(|store| {
            // Get the start checkpoint of the epoch
            let start_checkpoint_seq = store
                .get_last_checkpoint_of_epoch(epoch - 1)
                .map(|seq| seq + 1)
                .unwrap_or(0);

            let start_checkpoint = store
                .get_checkpoint_by_sequence_number(start_checkpoint_seq)
                .cloned()?;

            let system_state = store.get_system_state();

            Some(EpochInfo {
                epoch,
                protocol_version: system_state.protocol_version(),
                start_timestamp_ms: start_checkpoint.data().timestamp_ms,
                end_timestamp_ms: None,
                start_checkpoint: start_checkpoint_seq,
                end_checkpoint: None,
                reference_gas_price: system_state.reference_gas_price(),
                system_state,
            })
        })
    }

    fn get_type_layout(&self, type_tag: &TypeTag) -> Result<Option<MoveTypeLayout>> {
        self.simulacrum
            .get_type_layout(type_tag)
            .map_err(Into::into)
    }

    fn get_transaction(&self, digest: &TransactionDigest) -> Option<Arc<VerifiedTransaction>> {
        self.simulacrum
            .with_store(|store| store.get_transaction(digest).cloned().map(Arc::new))
    }

    fn get_transaction_effects(&self, digest: &TransactionDigest) -> Option<TransactionEffects> {
        self.simulacrum
            .with_store(|store| store.get_transaction_effects(digest).cloned())
    }

    fn get_transaction_events(
        &self,
        digest: &TransactionEventsDigest,
    ) -> Option<TransactionEvents> {
        self.simulacrum
            .with_store(|store| store.get_transaction_events(digest).cloned())
    }

    fn get_transaction_checkpoint(&self, digest: &TransactionDigest) -> Option<u64> {
        self.simulacrum.with_store(|store| {
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
        })
    }
}
