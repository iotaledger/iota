// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simulate;
mod transaction;

use std::{str::FromStr, sync::Arc};

use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    merge::Merge,
    v0::{
        error_reason::ErrorReason,
        event::Event,
        signatures::{UserSignature, UserSignatures},
        transaction::{
            ExecutedTransaction, TransactionEvents as ProtoTransactionEvents, TransactionReadSource,
        },
        transaction_execution_service::{
            self as grpc_tx_service, ExecuteTransactionRequest, ExecuteTransactionResponse,
            SimulateTransactionRequest, SimulateTransactionResponse,
        },
    },
};
use iota_package_resolver::{PackageStoreWithLruCache, Resolver};
use iota_types::{
    crypto::ToFromBytes, quorum_driver_types::ExecuteTransactionRequestV1,
    transaction_executor::TransactionExecutor,
};
use move_core_types::{annotated_value::MoveDatatypeLayout, language_storage::StructTag};
use tonic::{Request, Response};
pub use transaction::TransactionReadSource;

use crate::{error::RpcError, types::GrpcReader};

pub struct TransactionExecutionGrpcService {
    pub reader: Arc<GrpcReader>,
    pub executor: Arc<dyn TransactionExecutor>,
    pub config: iota_config::node::GrpcApiConfig,
}

impl TransactionExecutionGrpcService {
    pub fn new(
        reader: Arc<GrpcReader>,
        executor: Arc<dyn TransactionExecutor>,
        config: iota_config::node::GrpcApiConfig,
    ) -> Self {
        Self {
            reader,
            executor,
            config,
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
        execute_transaction(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(Into::into)
    }

    async fn simulate_transaction(
        &self,
        request: Request<SimulateTransactionRequest>,
    ) -> Result<Response<SimulateTransactionResponse>, tonic::Status> {
        simulate::simulate_transaction(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(Into::into)
    }
}

pub const EXECUTE_TRANSACTION_READ_MASK_DEFAULT: &str = "transaction.effects";

#[tracing::instrument(skip(reader, executor))]
pub async fn execute_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    request: ExecuteTransactionRequest,
) -> Result<ExecuteTransactionResponse, RpcError> {
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

    let transaction_data: iota_types::transaction::TransactionData =
        bcs::from_bytes(&transaction_bcs.data).map_err(|e| {
            FieldViolation::new("transaction.bcs")
                .with_description(format!("invalid transaction BCS: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
        })?;

    // Validate the digest if provided
    // Note that we don't force validation of the digest
    if let Some(provided_digest) = &transaction_proto.digest {
        let computed_digest = transaction_data.digest();
        let provided_digest_bytes: [u8; 32] =
            provided_digest.digest.as_ref().try_into().map_err(|_| {
                FieldViolation::new("transaction.digest")
                    .with_description("digest must be exactly 32 bytes")
                    .with_reason(ErrorReason::FieldInvalid)
            })?;

        if computed_digest.inner() != &provided_digest_bytes {
            let provided_digest_typed =
                iota_types::digests::TransactionDigest::new(provided_digest_bytes);
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

    let signatures = signatures_proto
        .signatures
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            let bcs_data = sig.bcs.as_ref().ok_or_else(|| {
                FieldViolation::new_at("signatures", i)
                    .with_description("signature BCS is required")
                    .with_reason(ErrorReason::FieldMissing)
            })?;

            iota_types::crypto::Signature::from_bytes(&bcs_data.data).map_err(|e| {
                FieldViolation::new_at("signatures", i)
                    .with_description(format!("invalid signature: {e}"))
                    .with_reason(ErrorReason::FieldInvalid)
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Create signed transaction
    // Clone signatures before moving them so we can use them in the response
    let signatures_for_response = signatures.clone();
    let signed_transaction =
        iota_types::transaction::Transaction::from_data(transaction_data, signatures);

    // Parse read mask
    let read_mask = request
        .read_mask
        .map(|mask| FieldMaskTree::from_field_mask(&mask))
        .unwrap_or_else(|| {
            EXECUTE_TRANSACTION_READ_MASK_DEFAULT
                .parse::<FieldMaskTree>()
                .unwrap()
        });

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

    // Create execution request
    let exec_request = ExecuteTransactionRequestV1 {
        transaction: signed_transaction.clone(),
        include_events,
        include_input_objects,
        include_output_objects,
        include_auxiliary_data: false,
    };

    // Execute the transaction
    let exec_response = executor
        .execute_transaction(exec_request, None)
        .await
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("transaction execution failed: {e}"),
            )
        })?;

    let effects = exec_response.effects.effects;
    let events = exec_response.events;
    let input_objects = exec_response.input_objects;
    let output_objects = exec_response.output_objects;

    // Get transaction digest
    let digest = *signed_transaction.digest();

    // For execute_transaction, checkpoint and timestamp are not available
    // immediately as the transaction is just being executed and not yet
    // included in a checkpoint
    let checkpoint = None;
    let timestamp_ms = None;

    // Convert iota_types to iota_sdk_types types for external compatibility
    // TODO: Remove this conversion when we migrate iota-types to iota_sdk_types
    // types
    let sdk_transaction: iota_sdk_types::SignedTransaction =
        signed_transaction.clone().try_into().map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert transaction to SDK type: {e}"),
            )
        })?;

    let sdk_effects: iota_sdk_types::TransactionEffects =
        effects.clone().try_into().map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert effects to SDK type: {e}"),
            )
        })?;

    let sdk_events: Option<iota_sdk_types::TransactionEvents> = events
        .as_ref()
        .map(|e| e.clone().try_into())
        .transpose()
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert events to SDK type: {e}"),
            )
        })?;

    let sdk_input_objects: Option<Vec<iota_sdk_types::object::Object>> = input_objects
        .map(|objects| {
            objects
                .into_iter()
                .map(|obj| obj.try_into())
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert input objects to SDK type: {e}"),
            )
        })?;

    let sdk_output_objects: Option<Vec<iota_sdk_types::object::Object>> = output_objects
        .map(|objects| {
            objects
                .into_iter()
                .map(|obj| obj.try_into())
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to convert output objects to SDK type: {e}"),
            )
        })?;

    let sdk_digest: iota_sdk_types::Digest = digest.into();

    // Build the response using merge
    let mut executed_transaction = ExecutedTransaction::default();

    let source = TransactionReadSource {
        digest: sdk_digest,
        transaction: &sdk_transaction,
        effects: &sdk_effects,
        events: sdk_events.as_ref(),
        checkpoint,
        timestamp_ms,
    };

    // Build the response
    let mut response = ExecuteTransactionResponse::default();

    // Only include transaction in response if requested by the mask
    if let Some(tx_mask) = read_mask.subtree(ExecuteTransactionResponse::TRANSACTION_FIELD.name) {
        executed_transaction.merge(&source, &tx_mask);

        // Handle events separately since they need special rendering
        if let Some(events_mask) = tx_mask.subtree(ExecutedTransaction::EVENTS_FIELD.name) {
            let mut proto_events = ProtoTransactionEvents::default();
            if let Some(sdk_events) = &sdk_events {
                proto_events.merge(sdk_events, &events_mask);

                // Populate json_contents for events if requested in the mask
                if events_mask
                    .subtree(ProtoTransactionEvents::EVENTS_FIELD.name)
                    .is_some_and(|mask| mask.contains(Event::JSON_CONTENTS_FIELD.name))
                {
                    // Create a package resolver
                    let package_store = PackageStoreWithLruCache::new(reader.as_ref().clone());
                    let resolver = Resolver::new(package_store);

                    // proto_events.events is Option<Events>, and Events.events is Vec<Event>
                    if let Some(ref mut events) = proto_events.events {
                        for (proto_event, sdk_event) in events.events.iter_mut().zip(&sdk_events.0)
                        {
                            // Convert sdk2 StructTag to move_core_types StructTag via string
                            // representation
                            let type_str = sdk_event.type_.to_string();
                            if let Ok(struct_tag) = StructTag::from_str(&type_str) {
                                // Get the type layout for this event's type
                                if let Ok(layout) = resolver.type_layout(struct_tag.into()).await {
                                    // Extract the datatype layout from the type layout
                                    let datatype_layout = match layout {
                                        move_core_types::annotated_value::MoveTypeLayout::Struct(s) => {
                                            Some(MoveDatatypeLayout::Struct(s))
                                        },
                                        move_core_types::annotated_value::MoveTypeLayout::Enum(e) => {
                                            Some(MoveDatatypeLayout::Enum(e))
                                        },
                                        _ => None, // Primitives are not datatypes
                                    };

                                    // Populate json_contents if we have a valid datatype layout
                                    if let Some(dt_layout) = datatype_layout {
                                        proto_event.populate_json_contents_with_layout(
                                            sdk_event, &dt_layout,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Always set events if requested in mask, even if empty
            executed_transaction.events = Some(proto_events);
        }

        // Handle signatures if requested
        if tx_mask.contains(ExecutedTransaction::SIGNATURES_FIELD.name) {
            // Convert signatures to proto format
            let proto_signatures: Vec<UserSignature> = signatures_for_response
                .iter()
                .map(|sig| UserSignature {
                    bcs: Some(iota_grpc_types::v0::bcs::BcsData {
                        data: sig.as_ref().to_vec().into(),
                    }),
                })
                .collect();

            executed_transaction.signatures = Some(UserSignatures {
                signatures: proto_signatures,
            });
        }

        // Handle input_objects if requested
        if let Some(input_objects_mask) =
            tx_mask.subtree(ExecutedTransaction::INPUT_OBJECTS_FIELD.name)
        {
            let mut proto_objects = iota_grpc_types::v0::object::Objects::default();
            if let Some(sdk_input_objects) = &sdk_input_objects {
                proto_objects.merge(sdk_input_objects.as_slice(), &input_objects_mask);
            }
            executed_transaction.input_objects = Some(proto_objects);
        }

        // Handle output_objects if requested
        if let Some(output_objects_mask) =
            tx_mask.subtree(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name)
        {
            let mut proto_objects = iota_grpc_types::v0::object::Objects::default();
            if let Some(sdk_output_objects) = &sdk_output_objects {
                proto_objects.merge(sdk_output_objects.as_slice(), &output_objects_mask);
            }
            executed_transaction.output_objects = Some(proto_objects);
        }

        response.transaction = Some(executed_transaction);
    }

    Ok(response)
}
