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

/// Which input checks a simulation drops relative to a transaction bound for
/// execution.
///
/// Every field defaults to `false`, so a check added to the shared path applies
/// to a simulation too until someone names it here and says why.
#[derive(Default, Debug, Copy, Clone)]
pub struct InputCheckRules {
    /// Skip the bounds on the gas budget itself, so a caller whose gas is not
    /// settled runs out of gas rather than being rejected. The gas coins are
    /// still required to be address-owned and to cover whatever budget is set.
    pub unbounded_gas_budget: bool,
    /// Skip the requirement that an address-owned input object be owned by the
    /// sender, so a caller can ask what a transaction would do over objects it
    /// does not own.
    ///
    /// This relaxes only whose object it is. A child object or a shared object
    /// named as an owned input is still rejected: those are not questions of
    /// permission but of what may be an owned input at all, and the engine
    /// treats the input checks as having settled them — a child object reaching
    /// it trips an invariant that names this checker.
    ///
    /// It also drops the check that the gas payment is owned by the
    /// transaction's gas owner, since gas coins go through the same arm. The
    /// weaker requirement that they be address-owned survives, in the gas
    /// balance check.
    pub any_object_owner: bool,
    /// Skip the match between an input object's declared digest and the loaded
    /// object. The digest is an optimistic-concurrency token for submission,
    /// which a simulation does not do, and nothing in execution reads it: the
    /// object is loaded by id and version, and every value is built from what
    /// was loaded. An object that does not exist still fails in the loader.
    pub any_object_digest: bool,
    /// Skip the match between a receiving reference's declared version and the
    /// version of the object it names, so a simulation can run over a reference
    /// the caller has not refreshed.
    ///
    /// A receiving reference's version is not only an optimistic-concurrency
    /// token, unlike the two above: execution resolves the receive at exactly
    /// the declared version, so a stale one still fails at runtime with
    /// `E_UNABLE_TO_RECEIVE_OBJECT`. This drops the up-front rejection, not the
    /// outcome.
    ///
    /// Neither this nor [`Self::any_receiving_object_digest`] relaxes what the
    /// object is, or the rejection of a reference that duplicates another or
    /// collides with an input object — that last one is the only rejection
    /// standing between a duplicated receiving ticket and an invariant
    /// violation in the object runtime.
    pub any_receiving_object_version: bool,
    /// Skip the match between a receiving reference's declared digest and the
    /// object it names, for the same reason as [`Self::any_object_digest`]: the
    /// digest is an optimistic-concurrency token for submission, which a
    /// simulation does not do.
    ///
    /// See [`Self::any_receiving_object_version`] for what stays checked.
    pub any_receiving_object_digest: bool,
}

impl InputCheckRules {
    /// No relaxations: exactly what a validator applies.
    pub const EXECUTION: Self = Self {
        unbounded_gas_budget: false,
        any_object_owner: false,
        any_object_digest: false,
        any_receiving_object_version: false,
        any_receiving_object_digest: false,
    };

    /// What a simulation with [`VmChecks::Disabled`] drops.
    pub const SIMULATION: Self = Self {
        unbounded_gas_budget: true,
        any_object_owner: true,
        any_object_digest: true,
        any_receiving_object_version: true,
        any_receiving_object_digest: true,
    };
}
