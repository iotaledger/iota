// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simulate;
mod transaction;

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    read_masks::EXECUTE_TRANSACTION_READ_MASK,
    v0::{
        error_reason::ErrorReason,
        transaction::ExecutedTransaction,
        transaction_execution_service::{
            self as grpc_tx_service, ExecuteTransactionRequest, ExecuteTransactionResponse,
            SimulateTransactionRequest, SimulateTransactionResponse,
        },
    },
};
use iota_types::{
    quorum_driver_types::{ExecuteTransactionRequestV1, ExecuteTransactionResponseV1},
    transaction_executor::TransactionExecutor,
};
use tonic::{Request, Response};
pub use transaction::{CommandResultsReadSource, TransactionReadSource};

use crate::{error::RpcError, merge::Merge, types::GrpcReader};

pub struct TransactionExecutionGrpcService {
    pub config: iota_config::node::GrpcApiConfig,
    pub reader: Arc<GrpcReader>,
    pub executor: Arc<dyn TransactionExecutor>,
}

impl TransactionExecutionGrpcService {
    pub fn new(
        config: iota_config::node::GrpcApiConfig,
        reader: Arc<GrpcReader>,
        executor: Arc<dyn TransactionExecutor>,
    ) -> Self {
        Self {
            config,
            reader,
            executor,
        }
    }
}

#[tonic::async_trait]
impl grpc_tx_service::transaction_execution_service_server::TransactionExecutionService
    for TransactionExecutionGrpcService
{
    async fn execute_transaction(
        &self,
        request: Request<ExecuteTransactionRequest>,
    ) -> Result<Response<ExecuteTransactionResponse>, tonic::Status> {
        let response = execute_transaction(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn simulate_transaction(
        &self,
        request: Request<SimulateTransactionRequest>,
    ) -> Result<Response<SimulateTransactionResponse>, tonic::Status> {
        let response = simulate::simulate_transaction(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }
}

/// Available Read Mask Fields
///
/// The `execute_transaction` function supports the following `read_mask` fields
/// to control which data is included in the response:
///
/// ## Transaction Fields
/// - `transaction` - includes all transaction fields
///   - `transaction.digest` - the transaction digest
///   - `transaction.bcs` - the full BCS-encoded transaction
/// - `signatures` - includes all signature fields
///   - `signatures.bcs` - the full BCS-encoded signature
/// - `effects` - includes all effects fields
///   - `effects.digest` - the effects digest
///   - `effects.bcs` - the full BCS-encoded effects
/// - `checkpoint` - the checkpoint that included the transaction (not available
///   for just-executed transactions)
/// - `timestamp` - the timestamp of the checkpoint (not available for
///   just-executed transactions)
///
/// ## Event Fields
/// - `events` - includes all event fields (all events of the transaction)
///   - `events.digest` - the events digest
///   - `events.events.bcs` - the full BCS-encoded event
///   - `events.events.package_id` - the ID of the package that emitted the
///     event
///   - `events.events.module` - the module that emitted the event
///   - `events.events.sender` - the sender that triggered the event
///   - `events.events.event_type` - the type of the event
///   - `events.events.bcs_contents` - the full BCS-encoded contents of the
///     event
///   - `events.events.json_contents` - the JSON-encoded contents of the event
///
/// ## Object Fields
/// - `input_objects` - includes all input object fields
///   - `input_objects.reference` - includes all reference fields
///     - `input_objects.reference.object_id` - the ID of the input object
///     - `input_objects.reference.version` - the version of the input object
///     - `input_objects.reference.digest` - the digest of the input object
///       contents
///   - `input_objects.bcs` - the full BCS-encoded object
/// - `output_objects` - includes all output object fields
///   - `output_objects.reference` - includes all reference fields
///     - `output_objects.reference.object_id` - the ID of the output object
///     - `output_objects.reference.version` - the version of the output object
///     - `output_objects.reference.digest` - the digest of the output object
///       contents
///   - `output_objects.bcs` - the full BCS-encoded object
#[tracing::instrument(skip(reader, executor))]
pub async fn execute_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    request: ExecuteTransactionRequest,
) -> Result<ExecuteTransactionResponse, RpcError> {
    // Parse read mask
    let read_mask = request
        .read_mask
        .map(|mask| FieldMaskTree::from_field_mask(&mask))
        .unwrap_or_else(|| {
            EXECUTE_TRANSACTION_READ_MASK
                .parse::<FieldMaskTree>()
                .unwrap()
        });

    // Extract and validate transaction
    let transaction_proto = request
        .transaction
        .as_ref()
        .ok_or_else(|| FieldViolation::new("transaction").with_reason(ErrorReason::FieldMissing))?;

    let transaction_bcs = transaction_proto.bcs.as_ref().ok_or_else(|| {
        FieldViolation::new("transaction.bcs")
            .with_description("transaction BCS is required")
            .with_reason(ErrorReason::FieldMissing)
    })?;

    let sdk_transaction: iota_sdk_types::transaction::Transaction =
        bcs::from_bytes(&transaction_bcs.data).map_err(|e| {
            FieldViolation::new("transaction.bcs")
                .with_description(format!("invalid transaction BCS: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
        })?;

    // Validate the digest if provided
    if let Some(provided_digest) = &transaction_proto.digest {
        let computed_digest = sdk_transaction.digest();
        let provided_digest_bytes: [u8; 32] =
            provided_digest.digest.as_ref().try_into().map_err(|_| {
                FieldViolation::new("transaction.digest")
                    .with_description("digest must be exactly 32 bytes")
                    .with_reason(ErrorReason::FieldInvalid)
            })?;

        if computed_digest.inner() != &provided_digest_bytes {
            let provided_digest_typed = iota_sdk_types::Digest::new(provided_digest_bytes);
            return Err(FieldViolation::new("transaction.digest")
                .with_description(format!(
                    "provided digest does not match computed digest: provided={provided_digest_typed}, computed={computed_digest}"
                ))
                .with_reason(ErrorReason::FieldInvalid)
                .into());
        }
    }

    // Extract and validate signatures
    let signatures_proto = request
        .signatures
        .as_ref()
        .ok_or_else(|| FieldViolation::new("signatures").with_reason(ErrorReason::FieldMissing))?;

    let sdk_signatures = signatures_proto
        .signatures
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            let bcs_data = sig.bcs.as_ref().ok_or_else(|| {
                FieldViolation::new_at("signatures", i)
                    .with_description("signature BCS is required")
                    .with_reason(ErrorReason::FieldMissing)
            })?;

            bcs::from_bytes::<iota_sdk_types::UserSignature>(&bcs_data.data).map_err(|e| {
                FieldViolation::new_at("signatures", i)
                    .with_description(format!("invalid signature: {e}"))
                    .with_reason(ErrorReason::FieldInvalid)
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Create signed transaction
    let sdk_signed_transaction = iota_sdk_types::SignedTransaction {
        transaction: sdk_transaction,
        signatures: sdk_signatures,
    };

    let transaction = iota_types::transaction::Transaction::try_from(sdk_signed_transaction)
        .map_err(|e| {
            RpcError::new(
                tonic::Code::InvalidArgument,
                format!("failed to convert transaction to internal type: {e}"),
            )
        })?;

    // Determine what to include in the request based on read mask.
    // ExecuteTransactionResponse is transparent, so the read_mask paths apply
    // directly to ExecutedTransaction fields.
    let include_events = read_mask.contains(ExecutedTransaction::EVENTS_FIELD.name);
    let include_input_objects = read_mask.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name);
    let include_output_objects = read_mask.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name);

    // Create execution request
    let exec_request = ExecuteTransactionRequestV1 {
        transaction: transaction.clone(),
        include_events,
        include_input_objects,
        include_output_objects,
        include_auxiliary_data: false,
    };

    // Execute the transaction
    let ExecuteTransactionResponseV1 {
        effects,
        events,
        input_objects,
        output_objects,
        auxiliary_data: _,
    } = executor
        .execute_transaction(exec_request, None)
        .await
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("transaction execution failed: {e}"),
            )
        })?;

    // Build the response.
    // ExecuteTransactionResponse is transparent, so we use read_mask directly.
    let sdk_transaction: iota_sdk_types::Transaction =
        transaction.transaction_data().clone().try_into()?;
    let signatures: Vec<iota_sdk_types::UserSignature> = transaction
        .tx_signatures()
        .to_owned()
        .into_iter()
        .map(|sig| sig.try_into())
        .collect::<Result<_, _>>()?;

    // Create a source for the merge
    let source = TransactionReadSource {
        reader: reader.clone(),
        config,
        transaction: Some(sdk_transaction),
        signatures: Some(signatures),
        effects: Some(effects.effects),
        events,
        // For execute_transaction, checkpoint and timestamp are not available
        // immediately as the transaction is just being executed and not yet
        // included in a checkpoint
        checkpoint: None,
        timestamp_ms: None,
        input_objects,
        output_objects,
    };

    Ok(
        ExecuteTransactionResponse::default().with_executed_transaction(
            ExecutedTransaction::merge_from(&source, &read_mask)
                .map_err(|e| e.with_context("failed to merge executed transaction"))?,
        ),
    )
}
