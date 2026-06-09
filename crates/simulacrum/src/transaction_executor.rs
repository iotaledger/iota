// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transaction executor implementation for Simulacrum
//!
//! This module provides a TransactionExecutor implementation that allows
//! transaction execution and simulation via gRPC without requiring quorum
//! consensus.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use iota_types::{
    digests::TransactionDigest,
    effects::TransactionEffectsAPI,
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::{
        ExecuteTransactionRequestV1, ExecuteTransactionResponseV1, FinalizedEffects,
        QuorumDriverError,
    },
    storage,
    transaction::TransactionData,
    transaction_executor::{
        SimulateTransactionResult, TransactionExecutor as TransactionExecutorTrait, VmChecks,
    },
};

use crate::Simulacrum;

/// Transaction executor implementation for simulacrum
/// This allows transaction execution and simulation via gRPC without requiring
/// quorum consensus
pub struct TransactionExecutor {
    simulacrum: Arc<Simulacrum>,
}

impl TransactionExecutor {
    pub fn new(simulacrum: Arc<Simulacrum>) -> Self {
        Self { simulacrum }
    }
}

#[async_trait]
impl TransactionExecutorTrait for TransactionExecutor {
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV1,
        _client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let simulacrum = &*self.simulacrum;

        // Execute the transaction directly
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
                self.simulacrum.with_store(|store| {
                    store
                        .get_transaction_events(effects.transaction_digest())
                        .cloned()
                })
            } else {
                None
            },
            input_objects: if request.include_input_objects {
                self.simulacrum.with_store(|store| {
                    storage::get_transaction_input_objects(store, &effects).ok()
                })
            } else {
                None
            },
            output_objects: if request.include_output_objects {
                self.simulacrum.with_store(|store| {
                    storage::get_transaction_output_objects(store, &effects).ok()
                })
            } else {
                None
            },
            auxiliary_data: if request.include_auxiliary_data {
                // We don't have any aux data generated presently, also in the real network
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
        self.simulacrum.simulate_transaction(transaction, checks)
    }

    /// Wait for the given transactions to be included in a checkpoint.
    ///
    /// Returns a mapping from transaction digest to
    /// `(checkpoint_sequence_number, checkpoint_timestamp_ms)`.
    /// On timeout, returns partial results for any transactions that were
    /// already checkpointed.
    async fn wait_for_checkpoint_inclusion(
        &self,
        _digests: &[TransactionDigest],
        _timeout: Duration,
    ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError> {
        Err(IotaError::UnsupportedFeature {
            error: "wait_for_checkpoint_inclusion not supported by this executor".into(),
        })
    }
}
