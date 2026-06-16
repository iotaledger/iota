// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Public input / output types for the [`LocalVm`](super::LocalVm) surface:
//! the [`ChainContext`] / [`ExecuteOptions`] inputs and the
//! [`ExecutionResult`] / [`DecodedEvent`] / [`SignatureStatus`] outputs.

use iota_protocol_config::{Chain, ProtocolVersion};
use iota_sdk_types::{Event, ObjectId, gas::GasCostSummary};
use iota_types::{
    effects::{TransactionEffects, TransactionEvents},
    object::Object,
};
use move_core_types::annotated_value::MoveValue;

use crate::debug::{DebugArtifacts, DebugConfig};

/// The chain parameters a [`LocalVm`](super::LocalVm) needs.
///
/// Usually obtained from a node via
/// [`GrpcStore::fetch_chain_context`](crate::grpc::GrpcStore::fetch_chain_context)
/// or
/// [`GraphqlStore::fetch_chain_context`](crate::graphql::GraphqlStore::fetch_chain_context);
/// construct it manually only for offline runs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ChainContext {
    /// Protocol version the run should use; selects the protocol config.
    pub protocol_version: ProtocolVersion,
    /// Reference gas price for the epoch, in NANOS.
    pub reference_gas_price: u64,
    /// Epoch the transaction runs in.
    pub epoch_id: u64,
    /// Epoch start timestamp in milliseconds (the VM clock).
    pub epoch_timestamp_ms: u64,
    /// Which chain this is (`Mainnet` / `Testnet` / `Unknown`), used for
    /// chain-specific protocol behaviour.
    pub chain: Chain,
}

impl ChainContext {
    /// Build a [`ChainContext`] from its parts. The three `u64` arguments are,
    /// in order, `reference_gas_price`, `epoch_id`, and `epoch_timestamp_ms`.
    pub fn new(
        protocol_version: ProtocolVersion,
        reference_gas_price: u64,
        epoch_id: u64,
        epoch_timestamp_ms: u64,
        chain: Chain,
    ) -> Self {
        Self {
            protocol_version,
            reference_gas_price,
            epoch_id,
            epoch_timestamp_ms,
            chain,
        }
    }
}

/// How a transaction is run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ExecutionMode {
    /// Relaxed Move VM checks; the store is never modified.
    DevInspect,
    /// Full sign-time checks; the store is never modified. The default: full
    /// validation without committing.
    ///
    /// Object references in the transaction (gas payments and owned inputs)
    /// are resolved against whatever versions the store holds, so a stale
    /// version or digest that a node would reject at signing time still runs
    /// here.
    #[default]
    DryRun,
    /// Full sign-time checks; on success, effects are applied back to the
    /// store and [`ExecutionResult::committed`] is `true`.
    Execute,
}

/// The outcome of signature verification for a run.
#[derive(Debug)]
#[non_exhaustive]
pub enum SignatureStatus {
    /// No signature was supplied (unsigned
    /// [`LocalVm::execute`](super::LocalVm::execute)).
    NotChecked,
    /// Signatures verified successfully. For a
    /// [`MoveAuthenticator`](iota_types::move_authenticator::MoveAuthenticator)
    /// this means the authenticator function did not abort during execution.
    Verified,
    /// Signature verification failed.
    Failed(crate::error::SignatureError),
}

/// Options for a single run: the [`ExecutionMode`] plus a [`DebugConfig`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ExecuteOptions {
    pub mode: ExecutionMode,
    pub debug: DebugConfig,
}

impl ExecuteOptions {
    /// Dev-inspect mode (relaxed checks).
    pub fn dev_inspect() -> Self {
        Self {
            mode: ExecutionMode::DevInspect,
            debug: DebugConfig::default(),
        }
    }

    /// Dry-run mode (full checks, no commit).
    pub fn dry_run() -> Self {
        Self {
            mode: ExecutionMode::DryRun,
            debug: DebugConfig::default(),
        }
    }

    /// Execute mode (full checks, commit on success).
    pub fn execute() -> Self {
        Self {
            mode: ExecutionMode::Execute,
            debug: DebugConfig::default(),
        }
    }

    /// Attach a [`DebugConfig`] to capture prints / profile / trace.
    #[must_use]
    pub fn with_debug(mut self, cfg: DebugConfig) -> Self {
        self.debug = cfg;
        self
    }
}

/// The full result of a run: effects, events, per-command results, the input
/// and output object sets, gas accounting, signature status, whether the run
/// was committed to the store, and any captured debug artifacts.
#[derive(Debug)]
#[non_exhaustive]
pub struct ExecutionResult {
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    /// Per-PTB-command `(mutable_reference_outputs, return_values)`.
    ///
    /// Empty for `MoveAuthenticator`-signed runs: the authenticator engine
    /// entry point does not return per-command results.
    pub command_results: Vec<iota_types::execution::ExecutionResult>,
    pub input_objects: Vec<Object>,
    pub output_objects: Vec<Object>,
    pub gas_summary: GasCostSummary,
    pub mock_gas_id: Option<ObjectId>,
    pub status: iota_sdk_types::ExecutionStatus,
    pub signature_status: SignatureStatus,
    /// `true` if and only if [`ExecutionMode::Execute`] ran successfully and
    /// the effects were applied back to the store.
    pub committed: bool,
    pub debug: Option<DebugArtifacts>,
}

/// A Move event paired with its decoded payload.
#[derive(Debug)]
#[non_exhaustive]
pub struct DecodedEvent {
    /// The event as emitted: package, module, sender, type, and raw contents.
    pub event: Event,
    /// The event contents decoded against their Move type.
    pub value: MoveValue,
}
