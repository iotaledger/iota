// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC streaming server for Simulacrum

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_server::{GrpcReader, GrpcServerHandle, start_grpc_server};
use iota_types::{
    digests::{ChainIdentifier, CheckpointDigest},
    effects::TransactionEffectsAPI,
    quorum_driver_types::{
        ExecuteTransactionRequestV1, ExecuteTransactionResponseV1, FinalizedEffects,
        QuorumDriverError,
    },
    transaction::TransactionData,
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};
use simulacrum::{Simulacrum, state_reader::SimulacrumGrpcReader};

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

/// Transaction executor implementation for simulacrum
/// This allows transaction execution and simulation via gRPC without requiring
/// quorum consensus
pub struct SimulacrumTransactionExecutor {
    simulacrum: Arc<Simulacrum>,
}

impl SimulacrumTransactionExecutor {
    pub fn new(simulacrum: Arc<Simulacrum>) -> Self {
        Self { simulacrum }
    }
}

#[async_trait::async_trait]
impl TransactionExecutor for SimulacrumTransactionExecutor {
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV1,
        _client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let simulacrum = &*self.simulacrum;

        // Execute the transaction directly (it's already a Transaction type)
        let (effects, _execution_error) = simulacrum
            .execute_transaction(request.transaction.clone())
            .map_err(|e| {
                QuorumDriverError::QuorumDriverInternal(iota_types::error::IotaError::Unknown(
                    e.to_string(),
                ))
            })?;

        // Create a checkpoint to finalize the transaction
        let checkpoint = simulacrum.create_checkpoint();

        tracing::debug!(
            tx_digest = ?effects.transaction_digest(),
            checkpoint = checkpoint.sequence_number(),
            "Transaction executed and finalized in simulacrum"
        );

        // For simulacrum, we create a dummy certified effects since there's no real
        // validator consensus. We use
        // CertifiedTransactionEffects::new_from_data_and_sig with empty
        // signatures.
        let (test_committee, _) = iota_types::committee::Committee::new_simple_test_committee();
        let effects_cert = iota_types::effects::CertifiedTransactionEffects::new_from_data_and_sig(
            effects.clone(),
            iota_types::crypto::AuthorityQuorumSignInfo::new_from_auth_sign_infos(
                vec![],
                &test_committee,
            )
            .unwrap(),
        );
        let verified_effects =
            iota_types::effects::VerifiedCertifiedTransactionEffects::new_unchecked(effects_cert);

        // Build response
        let response = ExecuteTransactionResponseV1 {
            effects: FinalizedEffects::new_from_effects_cert(verified_effects.into()),
            events: if request.include_events {
                // TODO: get events from simulacrum store
                None
            } else {
                None
            },
            input_objects: if request.include_input_objects {
                // TODO: get input objects from simulacrum store
                None
            } else {
                None
            },
            output_objects: if request.include_output_objects {
                // TODO: get output objects from simulacrum store
                None
            } else {
                None
            },
            auxiliary_data: if request.include_auxiliary_data {
                // TODO: get auxiliary data
                None
            } else {
                None
            },
        };

        Ok(response)
    }

    fn simulate_transaction(
        &self,
        transaction: TransactionData,
        checks: VmChecks,
    ) -> Result<SimulateTransactionResult, iota_types::error::IotaError> {
        // Simulacrum is already thread-safe, no locking needed
        self.simulacrum.simulate_transaction(transaction, checks)
    }
}

/// Start a gRPC server for the given simulacrum instance
pub async fn start_simulacrum_grpc_server(
    simulacrum: Arc<Simulacrum>,
    config: iota_config::node::GrpcApiConfig,
    shutdown_token: tokio_util::sync::CancellationToken,
) -> Result<GrpcServerHandle> {
    let chain_id = ChainIdentifier::from(CheckpointDigest::default());

    // Create a transaction executor for simulacrum to enable transaction execution
    // and simulation via gRPC
    let executor = Some(
        Arc::new(SimulacrumTransactionExecutor::new(simulacrum.clone()))
            as Arc<dyn TransactionExecutor>,
    );

    let simulacrum_reader = Arc::new(SimulacrumGrpcReader::new(simulacrum.clone(), chain_id));
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

        let simulacrum = Arc::new(simulacrum);

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
