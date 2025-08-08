// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Built-in gRPC streaming server for Simulacrum

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_api::{Config, GrpcReader, GrpcServerHandle, GrpcStateReader, start_grpc_server};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};

use crate::{Simulacrum, store::SimulatorStore};

/// GrpcStateReader implementation for Simulacrum
pub struct SimulacrumGrpcReader<Store: SimulatorStore> {
    simulacrum: Arc<Simulacrum<rand::rngs::OsRng, Store>>,
}

impl<Store: SimulatorStore> SimulacrumGrpcReader<Store> {
    pub fn new(simulacrum: Arc<Simulacrum<rand::rngs::OsRng, Store>>) -> Self {
        Self { simulacrum }
    }
}

impl<Store: SimulatorStore + Send + Sync + 'static> GrpcStateReader
    for SimulacrumGrpcReader<Store>
{
    fn get_latest_checkpoint_sequence(&self) -> Option<u64> {
        self.simulacrum
            .store()
            .get_highest_checkpoint()
            .map(|checkpoint| *checkpoint.sequence_number())
    }

    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        self.simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)
            .map(CertifiedCheckpointSummary::from)
    }

    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let checkpoint = self
            .simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)?;
        let contents = self
            .simulacrum
            .store()
            .get_checkpoint_contents(&checkpoint.content_digest)?;

        Some(CheckpointData {
            checkpoint_summary: CertifiedCheckpointSummary::from(checkpoint),
            checkpoint_contents: contents,
            transactions: vec![],
        })
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        // Simple implementation for simulacrum - find the last checkpoint of the given
        // epoch
        let latest_seq = self.get_latest_checkpoint_sequence().unwrap_or(0);

        for seq in (0..=latest_seq).rev() {
            if let Some(summary) = self.get_checkpoint_summary(seq) {
                if summary.epoch() == epoch {
                    return Ok(Some(summary));
                }
            }
        }
        Ok(None)
    }
}

impl<Store: SimulatorStore + Send + Sync + 'static> Simulacrum<rand::rngs::OsRng, Store> {
    /// Start a gRPC server for this simulacrum instance
    pub async fn start_grpc_server(self: Arc<Self>, config: Config) -> Result<GrpcServerHandle> {
        // Create the simulacrum gRPC reader
        let simulacrum_reader = Arc::new(SimulacrumGrpcReader::new(self));
        let grpc_reader = Arc::new(GrpcReader::new(simulacrum_reader));

        // Use the shared gRPC server utility
        start_grpc_server(grpc_reader, config).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iota_config::local_ip_utils;
    use iota_types::base_types::IotaAddress;

    use super::*;
    use crate::Simulacrum;

    #[tokio::test]
    async fn test_grpc_server_startup() {
        let mut simulacrum = Simulacrum::new();

        // Create some checkpoints
        simulacrum.advance_clock(Duration::from_secs(1));
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(simulacrum);

        // Start gRPC server with test configuration using test utilities
        let address = local_ip_utils::new_local_tcp_socket_for_testing();
        let config = Config {
            address,
            ..Config::default()
        };

        let server_handle = simulacrum.start_grpc_server(config).await.unwrap();

        // Verify server handle was created with proper address resolution
        assert!(server_handle.address().ip().is_loopback());
        assert!(server_handle.address().port() > 0);

        // Shutdown
        server_handle.shutdown();
    }

    #[tokio::test]
    async fn test_simulacrum_grpc_reader() {
        let mut simulacrum = Simulacrum::new();

        // Create some activity
        let recipient = IotaAddress::random_for_testing_only();
        let (tx, _) = simulacrum.transfer_txn(recipient);
        simulacrum.execute_transaction(tx).unwrap();
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(simulacrum);
        let reader = SimulacrumGrpcReader::new(simulacrum);

        // Test basic functionality
        assert!(reader.get_latest_checkpoint_sequence().is_some());
        assert!(reader.get_checkpoint_summary(0).is_some());
        assert!(reader.get_checkpoint_data(0).is_some());
        assert!(reader.get_epoch_last_checkpoint(0).unwrap().is_some());
    }
}
