// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Public input / output types for the [`LocalVm`](super::LocalVm) surface:
//! the [`ChainContext`] / [`ExecuteOptions`] inputs and the
//! [`ExecutionResult`] / [`DecodedEvent`] / [`SignatureStatus`] outputs.

use iota_config::transaction_deny_config::TransactionDenyConfig;
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
    /// Start from the protocol version and chain. The epoch fields default to
    /// `0`; set them with the `with_*` methods.
    pub fn new(protocol_version: ProtocolVersion, chain: Chain) -> Self {
        Self {
            protocol_version,
            reference_gas_price: 0,
            epoch_id: 0,
            epoch_timestamp_ms: 0,
            chain,
        }
    }

    /// Set the reference gas price for the epoch, in NANOS.
    #[must_use]
    pub fn with_reference_gas_price(mut self, reference_gas_price: u64) -> Self {
        self.reference_gas_price = reference_gas_price;
        self
    }

    /// Set the epoch the transaction runs in.
    #[must_use]
    pub fn with_epoch_id(mut self, epoch_id: u64) -> Self {
        self.epoch_id = epoch_id;
        self
    }

    /// Set the epoch start timestamp in milliseconds (the VM clock).
    #[must_use]
    pub fn with_epoch_timestamp_ms(mut self, epoch_timestamp_ms: u64) -> Self {
        self.epoch_timestamp_ms = epoch_timestamp_ms;
        self
    }
}

/// How a transaction is run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ExecutionMode {
    /// Relaxed Move VM checks; the store is never modified.
    DevInspect,
    /// Full sign-time checks; the store is never modified. The default.
    ///
    /// Object references in the transaction (gas payments and owned inputs)
    /// are resolved against whatever versions the store holds, so a stale
    /// version or digest that a node would reject at signing time still runs
    /// here.
    #[default]
    DryRun,
    /// Full sign-time checks; on success, effects are applied back to the
    /// store and [`ExecutionResult::committed`] is `true`.
    ///
    /// Requires a real gas payment — a gasless transaction is rejected rather
    /// than funded with the mock simulation coin, since its effects would be
    /// committed.
    ///
    /// A transaction that aborts commits nothing — not even the gas charge — so
    /// across multiple runs the store does not reflect a node's post-abort
    /// state.
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

/// Options for a single run: the [`ExecutionMode`], a [`DebugConfig`], and the
/// deny-list configuration.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ExecuteOptions {
    pub mode: ExecutionMode,
    pub debug: DebugConfig,
    /// Deny-list configuration applied during preparation. Defaults to an
    /// empty deny-list; set it to the target network's configuration to match
    /// a live validator's deny checks.
    pub deny_config: TransactionDenyConfig,
    /// Whether to run the regulated-coin deny-list check (a validator's
    /// `check_coin_deny_list_v1`) during preparation. Off by default: it does a
    /// store lookup per coin type in the transaction, so it is opt-in and only
    /// meaningful when the store holds the on-chain `DenyList`. Distinct from
    /// [`deny_config`](Self::deny_config), which is the operator's local
    /// policy.
    pub check_coin_deny_list: bool,
}

impl ExecuteOptions {
    /// Dev-inspect mode (relaxed checks).
    pub fn dev_inspect() -> Self {
        Self {
            mode: ExecutionMode::DevInspect,
            ..Self::default()
        }
    }

    /// Dry-run mode (full checks, no commit).
    pub fn dry_run() -> Self {
        Self {
            mode: ExecutionMode::DryRun,
            ..Self::default()
        }
    }

    /// Execute mode (full checks, commit on success).
    pub fn execute() -> Self {
        Self {
            mode: ExecutionMode::Execute,
            ..Self::default()
        }
    }

    /// Set the [`ExecutionMode`].
    #[must_use]
    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Attach a [`DebugConfig`] to capture a gas profile and/or an execution
    /// trace.
    #[must_use]
    pub fn with_debug(mut self, cfg: DebugConfig) -> Self {
        self.debug = cfg;
        self
    }

    /// Set the deny-list configuration applied during preparation.
    #[must_use]
    pub fn with_deny_config(mut self, deny_config: TransactionDenyConfig) -> Self {
        self.deny_config = deny_config;
        self
    }

    /// Enable the regulated-coin deny-list check during preparation. Requires
    /// the on-chain `DenyList` object in the store to have any effect; see
    /// [`check_coin_deny_list`](Self::check_coin_deny_list).
    #[must_use]
    pub fn with_coin_deny_list_check(mut self) -> Self {
        self.check_coin_deny_list = true;
        self
    }
}

/// The engine's per-PTB-command result `(mutable_reference_outputs,
/// return_values)`.
pub type CommandResult = iota_types::execution::ExecutionResult;

/// The full result of a run.
#[derive(Debug)]
#[non_exhaustive]
pub struct ExecutionResult {
    /// The transaction effects (object changes, gas, status digest).
    pub effects: TransactionEffects,
    /// Emitted events, if the run produced any.
    pub events: Option<TransactionEvents>,
    /// Per-PTB-command `(mutable_reference_outputs, return_values)`.
    ///
    /// Empty for `MoveAuthenticator`-signed runs (the authenticator engine
    /// entry point does not return per-command results) and for failed runs
    /// (the engine reports them only for a successful execution).
    pub command_results: Vec<CommandResult>,
    /// Objects read as inputs to the run.
    pub input_objects: Vec<Object>,
    /// Objects written by the run (created or mutated).
    pub output_objects: Vec<Object>,
    /// Gas ledger for the run (computation / storage / rebate).
    pub gas_summary: GasCostSummary,
    /// Id of the mock gas coin minted for a gas-less transaction, if any.
    pub mock_gas_id: Option<ObjectId>,
    /// The Move-level execution status (success or abort).
    pub status: iota_sdk_types::ExecutionStatus,
    /// The outcome of signature verification for the run.
    pub signature_status: SignatureStatus,
    /// `true` if and only if [`ExecutionMode::Execute`] ran successfully and
    /// the effects were applied back to the store.
    pub committed: bool,
    /// Captured debug artifacts (profile / trace), if requested.
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
