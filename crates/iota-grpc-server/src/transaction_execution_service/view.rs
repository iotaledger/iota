// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Handler for the `ViewFunctionCalls` gRPC endpoint: call `#[view]` Move
//! functions and return their values, without producing on-chain effects.

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    proto::prost_to_json,
    read_masks::VIEW_FUNCTION_CALLS_READ_MASK,
    v1::{
        bcs::BcsData,
        command::{CommandOutputs, InputArgument, input_argument},
        transaction_execution_service::{
            ExecutionError, ViewFunctionCallItem, ViewFunctionCallOutputs, ViewFunctionCallResult,
            ViewFunctionCallsRequest, ViewFunctionCallsResponse,
        },
        types::TypeTag as ProtoTypeTag,
    },
};
use iota_json::{
    IotaJsonValue, IotaMoveCallInputValue, ResolvedCallArg, resolve_move_function_args,
};
use iota_node_transaction_builder::NodeTransactionBuilderResolveClient;
use iota_sdk_transaction_builder::TransactionBuilder;
use iota_sdk_types::{
    Address, GasPayment, Identifier, ObjectId, TransactionExpiration, TypeTag,
    transaction::TransactionV1,
};
use iota_types::{
    error::IotaError,
    move_package::MovePackageExt,
    parse_iota_fq_name,
    transaction_executor::{TransactionExecutor, VmChecks},
};
use move_binary_format::binary_config::BinaryConfig;
use tonic::Code;

use super::CommandOutputsReadSource;
use crate::{error::RpcError, merge::Merge, types::GrpcReader, validation::validate_read_mask};

/// Call each requested `#[view]` Move function and return its results, in
/// request order.
///
/// Each call is resolved against the callee's on-chain signature and run in its
/// own dev-inspect transaction, so one call aborting or being rejected leaves
/// the others unaffected. A call's outcome is one of:
///
/// - `outputs.return_values` — the call ran and returned;
/// - `outputs.execution_error` — the call ran but aborted;
/// - `error` — the server rejected the call before running it (not a `#[view]`
///   function, unknown package/module/function, or an argument that could not
///   be resolved).
///
/// The request's `read_mask` selects which fields of each result to populate,
/// defaulting to [`VIEW_FUNCTION_CALLS_READ_MASK`].
#[tracing::instrument(skip_all, fields(batch_size = request.view_function_calls.len()))]
pub async fn view_function_calls(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    build_client: &NodeTransactionBuilderResolveClient,
    config: &iota_config::node::GrpcApiConfig,
    request: ViewFunctionCallsRequest,
) -> Result<ViewFunctionCallsResponse, RpcError> {
    super::validate_batch_size(
        request.view_function_calls.len(),
        config.max_view_function_call_batch_size as usize,
    )?;
    let read_mask = validate_read_mask::<ViewFunctionCallOutputs>(
        request.read_mask,
        VIEW_FUNCTION_CALLS_READ_MASK,
    )?;

    let mut call_results = Vec::with_capacity(request.view_function_calls.len());
    for item in &request.view_function_calls {
        let result = match view_function_call(
            reader,
            executor,
            build_client,
            config,
            item,
            &read_mask,
        )
        .await
        {
            Ok(r) => r,
            Err(error) => ViewFunctionCallResult::default().with_error(error.into_status_proto()),
        };
        call_results.push(result);
    }

    Ok(ViewFunctionCallsResponse::default().with_call_results(call_results))
}

/// Run a single view function call in its own dev-inspect transaction and build
/// its [`ViewFunctionCallResult`].
///
/// Returns `Err` only for calls the server rejects before execution (bad
/// arguments, missing or non-`#[view]` function); the caller records that as
/// the result's `error`. A call that runs — whether it returns or aborts — is
/// reported through the `Ok` result's `outputs`.
async fn view_function_call(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    build_client: &NodeTransactionBuilderResolveClient,
    config: &iota_config::node::GrpcApiConfig,
    item: &ViewFunctionCallItem,
    read_mask: &FieldMaskTree,
) -> Result<ViewFunctionCallResult, RpcError> {
    let mut builder = TransactionBuilder::new(Address::ZERO).with_client(build_client.clone());
    add_view_function_call(reader, &mut builder, item)?;

    let kind = builder.finish_kind().await.map_err(|err| {
        RpcError::new(
            Code::InvalidArgument,
            format!("Failed to build transaction kind from arguments: {err}"),
        )
    })?;
    // A view call needs no gas: the sender is the zero address with no gas
    // coins, and a zero gas price and budget signal the executor to mock a gas
    // coin for the dev-inspect run.
    let transaction = TransactionV1 {
        kind,
        sender: Address::ZERO,
        gas_payment: GasPayment {
            objects: vec![],
            owner: Address::ZERO,
            price: 0,
            budget: 0,
        },
        expiration: TransactionExpiration::None,
    };

    // Dev-inspect the transaction
    let simulation = executor
        .simulate_transaction(transaction.into(), VmChecks::Disabled)
        .map_err(|e| {
            RpcError::new(
                if matches!(e, IotaError::UserInput { .. }) {
                    Code::InvalidArgument
                } else {
                    Code::Internal
                },
                format!("transaction simulation failed: {e}"),
            )
        })?;

    // Build the response
    let mut response = ViewFunctionCallResult::default();

    // Only include the result if requested
    if let Some(outputs_mask) = read_mask.subtree(ViewFunctionCallOutputs::EXECUTION_RESULT_ONEOF) {
        // The call ran either way; report its return values, or the reason it
        // aborted, as the `execution_result` of the outputs.
        let outputs = match simulation.execution_result {
            Ok(command_results) => {
                // The transaction holds exactly one `MoveCall` command, so the
                // executor returns exactly one set of command results.
                let [(mutable_ref_outputs, return_values)] = command_results.as_slice() else {
                    return Err(RpcError::new(
                        Code::Internal,
                        format!(
                            "expected exactly one command result, got {}",
                            command_results.len()
                        ),
                    ));
                };
                if !mutable_ref_outputs.is_empty() {
                    return Err(RpcError::new(
                        Code::Internal,
                        format!(
                            "expected no mutable ref outputs in command result, got {}",
                            mutable_ref_outputs.len()
                        ),
                    ));
                }
                build_return_values(reader, config, &outputs_mask, return_values)?
            }
            Err(execution_error) => build_execution_error(&outputs_mask, &execution_error)?,
        };
        response.result = Some(super::view_function_call_result::Result::CallOutputs(
            outputs,
        ));
    }

    Ok(response)
}

/// Build the `return_values` of a call that ran to completion, honoring the
/// read mask (a subtree of the `outputs` field).
fn build_return_values(
    reader: &Arc<GrpcReader>,
    config: &iota_config::node::GrpcApiConfig,
    outputs_mask: &FieldMaskTree,
    return_values: &[(Vec<u8>, TypeTag)],
) -> Result<ViewFunctionCallOutputs, RpcError> {
    let Some(return_values_mask) =
        outputs_mask.subtree(ViewFunctionCallOutputs::RETURN_VALUES_FIELD.name)
    else {
        return Ok(ViewFunctionCallOutputs::default());
    };
    let source = CommandOutputsReadSource {
        reader,
        config,
        outputs: return_values
            .iter()
            .map(|(bcs, ty)| (None, bcs.as_slice(), ty))
            .collect(),
    };
    Ok(ViewFunctionCallOutputs::default()
        .with_return_values(CommandOutputs::merge_from(&source, &return_values_mask)?))
}

/// Build the `execution_error` of a call that aborted, honoring the read mask
/// (a subtree of the `outputs` field). Mirrors the error handling in
/// `simulate_transactions`.
fn build_execution_error(
    outputs_mask: &FieldMaskTree,
    execution_error: &iota_types::error::ExecutionError,
) -> Result<ViewFunctionCallOutputs, RpcError> {
    let Some(error_mask) =
        outputs_mask.subtree(ViewFunctionCallOutputs::EXECUTION_ERROR_FIELD.name)
    else {
        return Ok(ViewFunctionCallOutputs::default());
    };
    let mut exec_error = ExecutionError::default();
    if error_mask.contains(ExecutionError::BCS_KIND_FIELD.name) {
        exec_error.bcs_kind = Some(BcsData::serialize(execution_error.kind()).map_err(|e| {
            RpcError::new(
                Code::Internal,
                format!("failed to serialize execution error kind: {e}"),
            )
        })?);
    }
    if error_mask.contains(ExecutionError::SOURCE_FIELD.name) {
        exec_error.source = execution_error
            .source()
            .as_ref()
            .map(|source| source.to_string());
    }
    if error_mask.contains(ExecutionError::COMMAND_INDEX_FIELD.name) {
        if let Some(command_idx) = execution_error.command() {
            exec_error.command_index = Some(command_idx);
        }
    }
    Ok(ViewFunctionCallOutputs::default().with_execution_error(exec_error))
}

/// Resolve one call's arguments against its `#[view]` signature and append a
/// `MoveCall` command to `builder`.
fn add_view_function_call(
    reader: &Arc<GrpcReader>,
    builder: &mut TransactionBuilder<NodeTransactionBuilderResolveClient>,
    item: &ViewFunctionCallItem,
) -> Result<(), RpcError> {
    let (package_id, module, function) = parse_fq_function_name(&item.fq_function_name)?;
    let type_args = convert_type_args(&item.type_args)?;
    let args = item
        .inputs
        .iter()
        .map(proto_arg_to_call_arg)
        .collect::<Result<Vec<_>, _>>()?;

    let object = reader
        .get_object(&package_id)
        .map_err(|e| RpcError::new(Code::Internal, format!("failed to read package: {e}")))?
        .ok_or_else(|| RpcError::new(Code::NotFound, format!("package {package_id} not found")))?;
    let package = object.as_package_opt().ok_or_else(|| {
        RpcError::new(
            Code::InvalidArgument,
            format!("{package_id} is not a package"),
        )
    })?;

    let compiled_module = package
        .deserialize_module(&module, &BinaryConfig::standard())
        .map_err(|e| {
            RpcError::new(
                Code::InvalidArgument,
                format!("failed to deserialize module {module}: {e}"),
            )
        })?;
    let resolved = resolve_move_function_args(&compiled_module, &function, &type_args, args, true)
        .map_err(|e| RpcError::new(Code::InvalidArgument, format!("{e}")))?;

    let mut args = Vec::with_capacity(resolved.len());
    for (resolved_arg, _) in resolved {
        let arg = match resolved_arg {
            ResolvedCallArg::Pure(bytes) => {
                builder.apply_argument(iota_sdk_types::Input::Pure(bytes))
            }
            ResolvedCallArg::Object(object_id) => builder.apply_argument(object_id),
            ResolvedCallArg::ObjVec(_) => {
                return Err(RpcError::new(
                    Code::InvalidArgument,
                    "vector of objects argument to view functions not supported".to_string(),
                ));
            }
        };
        args.push(arg);
    }

    builder
        .move_call(package_id, module.as_str(), function.as_str())
        .type_tags(type_args)
        .arguments(args);

    Ok(())
}

/// Split `<package>::<module>::<function>` into its parts.
fn parse_fq_function_name(name: &str) -> Result<(ObjectId, Identifier, Identifier), RpcError> {
    let (module_id, function) = parse_iota_fq_name(name).map_err(|e| {
        RpcError::new(
            Code::InvalidArgument,
            format!("invalid function name `{name}`: {e}"),
        )
    })?;
    let package = ObjectId::new((*module_id.address()).into_bytes());
    let module = Identifier::new(module_id.name().as_str())
        .map_err(|e| RpcError::new(Code::InvalidArgument, format!("invalid module name: {e}")))?;
    let function = Identifier::new(function)
        .map_err(|e| RpcError::new(Code::InvalidArgument, format!("invalid function name: {e}")))?;
    Ok((package, module, function))
}

/// Convert the request's protobuf type arguments into internal `TypeTag`s.
fn convert_type_args(type_args: &[ProtoTypeTag]) -> Result<Vec<TypeTag>, RpcError> {
    type_args
        .iter()
        .map(|tt| {
            TypeTag::try_from(tt).map_err(|e| {
                RpcError::new(Code::InvalidArgument, format!("invalid type argument: {e}"))
            })
        })
        .collect()
}

/// Convert one request argument into a [`IotaMoveCallInputValue`]. Exactly one
/// of `bcs` / `json` must be set.
fn proto_arg_to_call_arg(arg: &InputArgument) -> Result<IotaMoveCallInputValue, RpcError> {
    match &arg.input {
        Some(input_argument::Input::Bcs(bcs)) => Ok(IotaMoveCallInputValue::Bcs(bcs.data.to_vec())),
        Some(input_argument::Input::Json(value)) => IotaJsonValue::new(prost_to_json(value))
            .map(IotaMoveCallInputValue::Json)
            .map_err(|e| {
                RpcError::new(Code::InvalidArgument, format!("invalid json argument: {e}"))
            }),
        Some(_) => Err(RpcError::new(
            Code::InvalidArgument,
            "argument is neither bcs nor json",
        )),
        None => Err(RpcError::new(Code::InvalidArgument, "argument is not set")),
    }
}

#[cfg(test)]
mod tests {
    use iota_grpc_types::v1::{bcs::BcsData, command::InputArgument};
    use iota_sdk_types::ObjectId;
    use prost_types::{Value, value::Kind};

    use super::*;

    #[test]
    fn json_arg_becomes_json_view_arg() {
        let arg = InputArgument::default().with_json(Value {
            kind: Some(Kind::StringValue("42".into())),
        });
        assert!(matches!(
            proto_arg_to_call_arg(&arg).unwrap(),
            IotaMoveCallInputValue::Json(_)
        ));
    }

    #[test]
    fn bcs_arg_becomes_bcs_view_arg() {
        let arg = InputArgument::default().with_bcs(BcsData::default().with_data(vec![1, 2, 3]));
        match proto_arg_to_call_arg(&arg).unwrap() {
            IotaMoveCallInputValue::Bcs(bytes) => assert_eq!(bytes, vec![1, 2, 3]),
            _ => panic!("expected bcs"),
        }
    }

    #[test]
    fn arg_with_neither_field_is_rejected() {
        let arg = InputArgument::default();
        assert!(proto_arg_to_call_arg(&arg).is_err());
    }

    #[test]
    fn parse_fq_name_splits_package_module_function() {
        let (pkg, module, function) = parse_fq_function_name("0x2::coin::value").unwrap();
        assert_eq!(pkg, ObjectId::from_short_hex("0x2").unwrap());
        assert_eq!(module.as_str(), "coin");
        assert_eq!(function.as_str(), "value");
    }

    #[test]
    fn parse_fq_name_rejects_missing_function() {
        assert!(parse_fq_function_name("0x2::coin").is_err());
    }
}
