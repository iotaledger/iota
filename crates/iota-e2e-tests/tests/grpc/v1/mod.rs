// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod header;
mod ledger_service;
mod move_package_service;
mod state_service;
mod transaction_execution_service;

use iota_grpc_types::v1::{
    command::{
        Argument, CommandOutput, CommandOutputs, CommandResult, CommandResults,
        argument::{Input, Result},
    },
    ledger_service::GetServiceInfoResponse,
    object::Object,
    transaction::{ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents},
    transaction_execution_service::{ExecutionError, SimulatedTransaction},
    types::ObjectReference,
};

use crate::impl_field_presence_checker;

impl_field_presence_checker!(ObjectReference {
    object_id,
    version,
    digest,
});
impl_field_presence_checker!(Object {
    reference: ObjectReference,
    bcs,
});

impl_field_presence_checker!(GetServiceInfoResponse {
    chain_id,
    chain,
    epoch,
    executed_checkpoint_height,
    executed_checkpoint_timestamp,
    lowest_available_checkpoint,
    lowest_available_checkpoint_objects,
    server,
});

impl_field_presence_checker!(Transaction { digest, bcs });
impl_field_presence_checker!(TransactionEffects { digest, bcs });
impl_field_presence_checker!(TransactionEvents { digest, events });
impl_field_presence_checker!(ExecutedTransaction {
    transaction: Transaction,
    signatures,
    effects: TransactionEffects,
    events: TransactionEvents,
    checkpoint,
    timestamp,
    input_objects,
    output_objects,
});
impl_field_presence_checker!(Input { index });
impl_field_presence_checker!(Result {
    index,
    nested_result_index,
});
impl_field_presence_checker!(Argument { kind });
impl_field_presence_checker!(CommandOutput {
    argument: Argument,
    type_tag,
    bcs,
    json,
});
impl_field_presence_checker!(CommandOutputs, transparent_repeated(outputs) {
    argument,
    type_tag,
    bcs,
    json,
});
impl_field_presence_checker!(CommandResult {
    mutated_by_ref: CommandOutputs,
    return_values: CommandOutputs,
});
impl_field_presence_checker!(CommandResults, transparent_repeated(results) {
    mutated_by_ref,
    return_values,
});
impl_field_presence_checker!(ExecutionError {
    bcs_kind,
    source,
    command_index,
});
impl_field_presence_checker!(SimulatedTransaction {
    executed_transaction: ExecutedTransaction,
    suggested_gas_price,
    execution_result,
});
