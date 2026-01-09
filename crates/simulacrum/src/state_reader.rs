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
    iota_system_state::{
        IotaSystemState, epoch_start_iota_system_state::EpochStartSystemStateTrait,
    },
    messages_checkpoint::CertifiedCheckpointSummary,
    object::Object,
    storage::EpochInfo,
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
        let checkpoint = self
            .simulacrum
            .with_store(|store| store.get_checkpoint_by_sequence_number(seq).cloned())?;

        let contents = self.simulacrum.with_store(|store| {
            store
                .get_checkpoint_contents(&checkpoint.content_digest)
                .cloned()
        })?;

        Some(CheckpointData {
            checkpoint_summary: CertifiedCheckpointSummary::from(checkpoint),
            checkpoint_contents: contents,
            // TODO: we should return the transactions as well
            transactions: vec![],
        })
    }

    fn get_epoch_last_checkpoint(&self, epoch: u64) -> Result<Option<CertifiedCheckpointSummary>> {
        // Simple implementation for simulacrum - find the last checkpoint of the given
        // epoch
        // TODO: optimize that by storing epoch -> last checkpoint mapping
        let latest_seq = self
            .simulacrum
            .with_store(|store| {
                store
                    .get_highest_checkpoint()
                    .map(|checkpoint| *checkpoint.sequence_number())
            })
            .unwrap_or(0);

        for seq in (0..=latest_seq).rev() {
            if let Some(checkpoint) = self
                .simulacrum
                .with_store(|store| store.get_checkpoint_by_sequence_number(seq).cloned())
            {
                if checkpoint.epoch() == epoch {
                    return Ok(Some(CertifiedCheckpointSummary::from(checkpoint)));
                }
            }
        }
        Ok(None)
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
        let current_epoch = self.simulacrum.with_store(|store| {
            store
                .get_highest_checkpoint()
                .map(|cp| cp.epoch())
                .unwrap_or(0)
        });

        if epoch == current_epoch {
            let epoch_start_state = self.simulacrum.epoch_start_state();
            Ok(Some(Arc::new(epoch_start_state.get_iota_committee())))
        } else {
            // TODO: implement
            Ok(None)
        }
    }

    fn get_system_state(&self) -> Result<IotaSystemState> {
        Ok(self.simulacrum.with_store(|store| store.get_system_state()))
    }

    fn get_epoch_info(&self, _epoch: u64) -> Option<EpochInfo> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }

    fn get_type_layout(&self, _type_tag: &TypeTag) -> Result<Option<MoveTypeLayout>> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        Ok(None)
    }

    fn get_transaction(&self, _digest: &TransactionDigest) -> Option<Arc<VerifiedTransaction>> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }

    fn get_transaction_effects(&self, _digest: &TransactionDigest) -> Option<TransactionEffects> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }

    fn get_transaction_events(
        &self,
        _digest: &TransactionEventsDigest,
    ) -> Option<TransactionEvents> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }

    fn get_transaction_checkpoint(&self, _digest: &TransactionDigest) -> Option<u64> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }
}
