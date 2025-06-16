// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use iota_protocol_config::ProtocolConfig;
use iota_types::{
    base_types::{IotaAddress, ObjectRef, TxContext},
    committee::EpochId,
    digests::TransactionDigest,
    effects::TransactionEffects,
    error::ExecutionError,
    execution::{ExecutionResult, TypeLayoutStore},
    gas::IotaGasStatus,
    inner_temporary_store::InnerTemporaryStore,
    layout_resolver::LayoutResolver,
    metrics::LimitsMetrics,
    storage::BackingStore,
    transaction::{CheckedInputObjects, ProgrammableTransaction, TransactionKind},
};
use move_trace_format::format::MoveTraceBuilder;

/// Abstracts over access to the VM across versions of the execution layer.
pub trait Executor {
    fn execute_transaction_to_effects(
        &self,
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
        // Epoch
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        // Transaction Inputs
        input_objects: CheckedInputObjects,
        // Gas related
        gas_coins: Vec<ObjectRef>,
        gas_status: IotaGasStatus,
        // Transaction
        transaction_kind: TransactionKind,
        transaction_signer: IotaAddress,
        transaction_digest: TransactionDigest,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<(), ExecutionError>,
    );

    fn dev_inspect_transaction(
        &self,
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
        // Epoch
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        // Transaction Inputs
        input_objects: CheckedInputObjects,
        // Gas related
        gas_coins: Vec<ObjectRef>,
        gas_status: IotaGasStatus,
        // Transaction
        transaction_kind: TransactionKind,
        transaction_signer: IotaAddress,
        transaction_digest: TransactionDigest,
        skip_all_checks: bool,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<Vec<ExecutionResult>, ExecutionError>,
    );

    fn update_genesis_state(
        &self,
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        // Genesis State
        tx_context: &mut TxContext,
        // Transaction
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore, ExecutionError>;

    fn type_layout_resolver<'r, 'vm: 'r, 'store: 'r>(
        &'vm self,
        store: Box<dyn TypeLayoutStore + 'store>,
    ) -> Box<dyn LayoutResolver + 'r>;
}
