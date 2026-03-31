// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, time::Duration};

use crate::{
    base_types::ObjectID,
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEvents},
    error::{ExecutionError, IotaError},
    execution::ExecutionResult,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    quorum_driver_types::{
        ExecuteTransactionRequestV1, ExecuteTransactionResponseV1, QuorumDriverError,
    },
    transaction::TransactionData,
};

/// Trait to define the interface for how the REST service interacts with a
/// QuorumDriver or a simulated transaction executor.
#[async_trait::async_trait]
pub trait TransactionExecutor: Send + Sync {
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError>;

    fn simulate_transaction(
        &self,
        transaction: TransactionData,
        checks: VmChecks,
    ) -> Result<SimulateTransactionResult, IotaError>;

    /// Wait for the given transactions to be included in a checkpoint.
    ///
    /// Returns a mapping from transaction digest to
    /// `(checkpoint_sequence_number, checkpoint_timestamp_ms)`.
    /// On timeout, returns partial results for any transactions that were
    /// already checkpointed.
    async fn wait_for_checkpoint_inclusion(
        &self,
        digests: &[TransactionDigest],
        timeout: Duration,
    ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError>;
}

pub struct SimulateTransactionResult {
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub input_objects: BTreeMap<ObjectID, Object>,
    pub output_objects: BTreeMap<ObjectID, Object>,
    pub execution_result: Result<Vec<ExecutionResult>, ExecutionError>,
    pub mock_gas_id: Option<ObjectID>,
    pub suggested_gas_price: Option<u64>,
}

#[derive(Default, Debug, Copy, Clone)]
pub enum VmChecks {
    #[default]
    Enabled,
    Disabled,
}

impl VmChecks {
    pub fn disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}
