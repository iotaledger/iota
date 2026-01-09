// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC streaming server for Simulacrum

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_server::{GrpcReader, GrpcServerHandle, GrpcStateReader, start_grpc_server};
use iota_types::{
    TypeTag,
    base_types::{ObjectID, VersionNumber},
    committee::Committee,
    digests::{ChainIdentifier, CheckpointDigest, TransactionDigest, TransactionEventsDigest},
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::CheckpointData,
    iota_system_state::IotaSystemState,
    messages_checkpoint::CertifiedCheckpointSummary,
    object::Object,
    storage::EpochInfo,
    transaction::VerifiedTransaction,
};
use move_core_types::annotated_value::MoveTypeLayout;
use simulacrum::Simulacrum;

// Dummy event subscriber for simulacrum (events not supported)
// TODO: add support for events in simulacrum?
struct DummyEventSubscriber;

impl iota_grpc_server::types::EventSubscriber for DummyEventSubscriber {
    fn subscribe_events(
        &self,
        _filter: iota_json_rpc_types::EventFilter,
    ) -> Box<dyn futures::Stream<Item = iota_json_rpc_types::IotaEvent> + Send + Unpin> {
        // Return an empty stream
        Box::new(Box::pin(futures::stream::empty()))
    }
}

/// GrpcStateReader implementation that works with a mutex-protected Simulacrum
/// This allows sharing the same simulacrum instance between REST and gRPC APIs
pub struct SimulacrumGrpcReader {
    simulacrum: Arc<tokio::sync::Mutex<Simulacrum>>,
    chain_id: ChainIdentifier,
}

impl SimulacrumGrpcReader {
    pub fn new(simulacrum: Arc<tokio::sync::Mutex<Simulacrum>>, chain_id: ChainIdentifier) -> Self {
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
        let rt = tokio::runtime::Handle::current();
        let simulacrum = rt.block_on(self.simulacrum.lock());

        simulacrum
            .store()
            .get_highest_checkpoint()
            .map(|checkpoint| *checkpoint.sequence_number())
    }

    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        let rt = tokio::runtime::Handle::current();
        let simulacrum = rt.block_on(self.simulacrum.lock());

        simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)
            .map(CertifiedCheckpointSummary::from)
    }

    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let rt = tokio::runtime::Handle::current();
        let simulacrum = rt.block_on(self.simulacrum.lock());

        let checkpoint = simulacrum.store().get_checkpoint_by_sequence_number(seq)?;
        let contents = simulacrum
            .store()
            .get_checkpoint_contents(&checkpoint.content_digest)?;

        Some(CheckpointData {
            checkpoint_summary: CertifiedCheckpointSummary::from(checkpoint),
            checkpoint_contents: contents,
            // TODO: we should return the transactions as well
            transactions: vec![],
        })
    }

    fn get_epoch_last_checkpoint(&self, epoch: u64) -> Result<Option<CertifiedCheckpointSummary>> {
        let rt = tokio::runtime::Handle::current();
        let simulacrum = rt.block_on(self.simulacrum.lock());

        // Simple implementation for simulacrum - find the last checkpoint of the given
        // epoch
        let latest_seq = simulacrum
            .store()
            .get_highest_checkpoint()
            .map(|checkpoint| *checkpoint.sequence_number())
            .unwrap_or(0);

        // TODO: optimize that by storing epoch -> last checkpoint mapping
        for seq in (0..=latest_seq).rev() {
            if let Some(checkpoint) = simulacrum.store().get_checkpoint_by_sequence_number(seq) {
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

    fn get_object(&self, _object_id: &ObjectID) -> Option<Object> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }

    fn get_object_by_key(&self, _object_id: &ObjectID, _version: VersionNumber) -> Option<Object> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        None
    }

    fn get_committee(&self, _epoch: u64) -> Result<Option<Arc<Committee>>> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        Ok(None)
    }

    fn get_system_state(&self) -> Result<IotaSystemState> {
        // Not implemented for simulacrum gRPC reader
        // TODO: implement
        Err(anyhow::anyhow!("System state not available in simulacrum"))
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

/// Start a gRPC server for the given simulacrum instance
pub async fn start_simulacrum_grpc_server(
    simulacrum: Arc<tokio::sync::Mutex<Simulacrum>>,
    config: iota_config::node::GrpcApiConfig,
    shutdown_token: tokio_util::sync::CancellationToken,
) -> Result<GrpcServerHandle> {
    let chain_id = ChainIdentifier::from(CheckpointDigest::default());

    // TODO: find a way to convert from Executor to TransactionExecutor (I guess we
    // need some simple implementation of the TransactionOrchestrator without
    // consensus)
    // let rt = tokio::runtime::Handle::current();
    // let simulacrum_tmp = rt.block_on(simulacrum.lock());
    // let executor = simulacrum_tmp.executor();
    // drop(simulacrum_tmp);
    let executor = None;

    let simulacrum_reader = Arc::new(SimulacrumGrpcReader::new(simulacrum, chain_id));
    let grpc_reader = Arc::new(GrpcReader::new(simulacrum_reader, None));

    // TODO: add if needed
    let event_subscriber: Option<Arc<dyn iota_grpc_server::types::EventSubscriber>> = None;

    start_grpc_server(
        grpc_reader,
        event_subscriber.unwrap_or_else(|| Arc::new(DummyEventSubscriber)),
        executor,
        config,
        shutdown_token,
        chain_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iota_config::local_ip_utils;
    use simulacrum::Simulacrum;

    use super::*;

    #[tokio::test]
    async fn test_grpc_server_startup_with_mutex() {
        let mut simulacrum = Simulacrum::new();

        // Create some checkpoints
        simulacrum.advance_clock(Duration::from_secs(1));
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(tokio::sync::Mutex::new(simulacrum));

        // Start gRPC server with test configuration using test utilities
        let address = local_ip_utils::new_local_tcp_socket_for_testing();
        let config = iota_config::node::GrpcApiConfig {
            address,
            ..Default::default()
        };
        let shutdown_token = tokio_util::sync::CancellationToken::new();

        let server_handle =
            start_simulacrum_grpc_server(simulacrum, config, shutdown_token.clone())
                .await
                .unwrap();

        // Verify server handle was created with proper address resolution
        assert!(server_handle.address().ip().is_loopback());
        assert!(server_handle.address().port() > 0);

        // Shutdown
        shutdown_token.cancel();
        server_handle.shutdown().await.unwrap();
    }
}
