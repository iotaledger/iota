// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, time::Duration};

use iota_sdk_types::{
    GasPayment, ObjectId, Transaction, TransactionDigest, TransactionEffects, TransactionEvents,
};

use crate::{
    error::{ExecutionError, IotaError},
    execution::ExecutionResult,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    quorum_driver_types::{
        ExecuteTransactionRequestV1, ExecuteTransactionResponseV1, QuorumDriverError,
    },
};

/// Trait to define the interface for how the REST service interacts with a
/// QuorumDriver or a simulated transaction executor.
#[async_trait::async_trait]
pub trait TransactionExecutor: Send + Sync {
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV1,
        skip_certification: bool,
        client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError>;

    fn simulate_transaction(
        &self,
        transaction: Transaction,
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

    /// Read authoritative effects, events, and input/output objects for a
    /// locally-executed transaction from the cache. Used by callers that
    /// have already waited for checkpoint inclusion and want to discard any
    /// uncertified single-validator copies.
    ///
    /// Returns `Ok(None)` if the tx is not in the cache, or if the executor
    /// does not maintain a local cache (e.g. simulacrum).
    fn read_transaction_from_cache(
        &self,
        digest: &TransactionDigest,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
    ) -> Result<Option<CachedTransactionData>, IotaError> {
        // Default: no cache — safe fallback for executors like simulacrum.
        let _ = (
            digest,
            include_events,
            include_input_objects,
            include_output_objects,
        );
        Ok(None)
    }
}

/// Authoritative per-transaction data read from a local cache.
pub struct CachedTransactionData {
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub input_objects: Option<Vec<Object>>,
    pub output_objects: Option<Vec<Object>>,
}

pub struct SimulateTransactionResult {
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    /// Every object the transaction ran with as input — including immutable
    /// and read-only shared inputs, the packages it calls, and the gas coins
    /// (the mock one included) — plus the runtime-loaded objects (e.g. dynamic
    /// fields) it modified, at their pre-state versions, keyed by id.
    pub input_objects: BTreeMap<ObjectId, Object>,
    pub output_objects: BTreeMap<ObjectId, Object>,
    /// The return values and mutable-reference outputs of every command, under
    /// either [`VmChecks`] — both run through the executor's dev-inspect entry
    /// point, which collects them regardless of which checks are in force.
    pub execution_result: Result<Vec<ExecutionResult>, ExecutionError>,
    pub mock_gas_id: Option<ObjectId>,
    pub suggested_gas_price: Option<u64>,
    /// The gas the simulation ran with, once whatever the transaction left
    /// unset was filled in. Callers reporting the transaction back should
    /// use this rather than re-deriving it, which would read a possibly
    /// different epoch.
    pub gas_data: GasPayment,
}

/// Which Move VM checks a simulation runs with.
///
/// This is the only thing that separates the two ways a transaction can be
/// simulated, so it is what callers pick between: a dry run wants
/// [`VmChecks::Enabled`], a dev inspect wants [`VmChecks::Disabled`].
#[derive(Default, Debug, Copy, Clone)]
pub enum VmChecks {
    /// Run the transaction as it would run on chain: the same input and gas
    /// checks a validator applies, and metering against the transaction's own
    /// budget.
    #[default]
    Enabled,
    /// Relax the rules around entry functions and argument values, so that any
    /// Move function can be called and any value built from its bytes. Input
    /// checks are reduced to the ones execution cannot proceed without.
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
