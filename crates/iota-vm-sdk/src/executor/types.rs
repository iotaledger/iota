// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Public input / output types for the [`LocalVm`](super::LocalVm) surface:
//! the [`ChainContext`] / [`ExecuteOptions`] inputs and the
//! [`ExecutionResult`] / [`GasEstimate`] / [`DecodedEvent`] /
//! [`SignatureStatus`] outputs.

use iota_protocol_config::{Chain, ProtocolVersion};
use iota_sdk_types::{
    Address as IotaAddress, Identifier, ObjectId, StructTag, gas::GasCostSummary,
};
use iota_types::{
    effects::{TransactionEffects, TransactionEvents},
    object::Object,
};
use move_core_types::annotated_value::MoveValue;

use crate::debug::{DebugArtifacts, DebugConfig};

/// The chain parameters a [`LocalVm`](super::LocalVm) needs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ChainContext {
    pub protocol_version: ProtocolVersion,
    pub reference_gas_price: u64,
    pub epoch_id: u64,
    pub epoch_timestamp_ms: u64,
    pub chain: Chain,
}

impl ChainContext {
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
    pub gas_estimate: GasEstimate,
    pub mock_gas_id: Option<ObjectId>,
    pub status: iota_sdk_types::ExecutionStatus,
    pub signature_status: SignatureStatus,
    /// `true` if and only if [`ExecutionMode::Execute`] ran successfully and
    /// the effects were applied back to the store.
    pub committed: bool,
    pub debug: Option<DebugArtifacts>,
}

/// Convenience summary of a run's gas ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GasEstimate {
    pub computation_cost: u64,
    pub storage_cost: u64,
    pub storage_rebate: u64,
    pub non_refundable_storage_fee: u64,
    /// `computation_cost + storage_cost - storage_rebate`. The figure a gas
    /// budget needs to cover.
    pub net_gas_usage: i64,
}

impl GasEstimate {
    pub(super) fn from_summary(s: &GasCostSummary) -> Self {
        Self {
            computation_cost: s.computation_cost,
            storage_cost: s.storage_cost,
            storage_rebate: s.storage_rebate,
            non_refundable_storage_fee: s.non_refundable_storage_fee,
            net_gas_usage: s.net_gas_usage(),
        }
    }

    /// Suggest a gas budget for a future execution of the same transaction:
    /// `net_gas_usage * headroom_factor`, rounded up. Returns `0` for a net
    /// rebate.
    pub fn suggested_budget_with_headroom(&self, headroom_factor: f64) -> u64 {
        if self.net_gas_usage <= 0 {
            return 0;
        }
        ((self.net_gas_usage as f64) * headroom_factor).ceil() as u64
    }
}

/// One decoded Move event with every field named and typed.
#[derive(Debug)]
#[non_exhaustive]
pub struct DecodedEvent {
    /// Package that emitted the event.
    pub package_id: ObjectId,
    /// Module inside that package that emitted the event.
    pub module: Identifier,
    /// The event struct's name.
    pub name: Identifier,
    /// Address of the transaction sender.
    pub sender: IotaAddress,
    /// The event struct's type, e.g. `0x2::coin::CoinEvent`.
    pub type_tag: StructTag,
    /// Decoded event payload.
    pub value: MoveValue,
}
