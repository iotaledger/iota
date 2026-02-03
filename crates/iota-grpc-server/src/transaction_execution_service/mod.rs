// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simulate;
mod transaction;

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    merge::Merge,
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

use crate::{error::RpcError, types::GrpcReader};

pub const EXECUTE_TRANSACTION_READ_MASK_DEFAULT: &str = "transaction.effects";

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
            EXECUTE_TRANSACTION_READ_MASK_DEFAULT
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

    // Determine what to include in the request based on read mask
    // The mask is at the response level, so we need to check the "transaction"
    // subtree
    let tx_mask = read_mask.subtree(ExecuteTransactionResponse::TRANSACTION_FIELD.name);
    let include_events = tx_mask
        .as_ref()
        .map(|m| m.contains(ExecutedTransaction::EVENTS_FIELD.name))
        .unwrap_or(false);
    let include_input_objects = tx_mask
        .as_ref()
        .map(|m| m.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name))
        .unwrap_or(false);
    let include_output_objects = tx_mask
        .as_ref()
        .map(|m| m.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name))
        .unwrap_or(false);

    let transaction_data = transaction.transaction_data().clone();
    let signatures = transaction.tx_signatures().to_owned();

    // Create execution request
    let exec_request = ExecuteTransactionRequestV1 {
        transaction,
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

    // Build the response
    let mut response = ExecuteTransactionResponse::default();

    // Only include transaction if requested
    if let Some(tx_mask) = read_mask.subtree(ExecuteTransactionResponse::TRANSACTION_FIELD.name) {
        // Create a source for the merge
        let source = TransactionReadSource {
            reader: reader.clone(),
            config,
            transaction_data,
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

        response.transaction = Some(ExecutedTransaction::merge_from(&source, &tx_mask).map_err(
            |e| {
                RpcError::new(
                    tonic::Code::Internal,
                    format!("failed to build executed transaction in execution response: {e}"),
                )
            },
        )?);
    }

    Ok(response)
}
