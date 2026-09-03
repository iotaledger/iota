// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use iota_grpc_types::v1::{
    command::InputArgument,
    transaction_execution_service::{
        ViewFunctionCallItem, ViewFunctionCallsRequest, ViewFunctionCallsResponse,
        transaction_execution_service_client::TransactionExecutionServiceClient,
        view_function_call_outputs, view_function_call_result,
    },
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_sdk_types::{ObjectId, Owner, SharedObjectReference};
use iota_test_transaction_builder::PublishData;
use iota_types::{effects::TransactionEffectsAPI, transaction::CallArg};
use prost_types::{Value, value::Kind};
use test_cluster::TestCluster;

use crate::utils::{first_sender, setup_grpc_test, wait_for_executed_transactions_checkpointed};

/// Path to the `view_functions` test package (`Move.toml` + `sources/`),
/// copied from `iota-json-rpc-tests`.
fn view_functions_package_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "grpc", "data", "view_functions"]);
    path
}

/// Publish the `view_functions` test package and return `(package_id,
/// counter)`, where `counter` is the shared `Counter` object seeded to
/// `value = 42` in the package's `init`.
///
/// Waits for the publish transaction to land in a checkpoint before
/// returning: the `view_function_calls` endpoint reads through the same
/// checkpoint-indexed state as the rest of the gRPC service, which lags
/// behind local (`WaitForLocalExecution`) visibility.
async fn publish_view_functions_package(
    test_cluster: &TestCluster,
    client: &iota_grpc_client::Client,
) -> (ObjectId, SharedObjectReference) {
    let sender = first_sender(test_cluster);

    // `with_allow_view_function` is required for the `#[view]` attribute on
    // `counter::value` to be compiled into on-chain module metadata.
    let compiled_package = BuildConfig::new_for_testing()
        .with_allow_view_function()
        .build(&view_functions_package_path())
        .expect("view_functions package should build");

    let tx_data = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .publish_with_data(PublishData::CompiledPackage(compiled_package))
        .build();
    let signed_tx = test_cluster.sign_transaction(&tx_data);
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(signed_tx)
        .await
        .expect("publishing view_functions package should be submitted");
    assert!(
        effects.status().is_success(),
        "publishing view_functions package should succeed: {:?}",
        effects.status()
    );

    // A publish creates the package alongside its metadata objects, and the
    // effects say nothing about what each created object is, so every one of
    // them is read back from the store to be told apart.
    let mut package_id = None;
    let mut counter = None;
    for object in &effects.created() {
        let object_id = object.reference.object_id;
        let stored = test_cluster
            .get_object_from_fullnode_store(&object_id)
            .await
            .expect("a created object should be in the store");
        if stored.data.is_package() {
            package_id = Some(object_id);
            continue;
        }
        let Owner::Shared(initial_shared_version) = object.owner else {
            continue;
        };
        if stored
            .data
            .opt_struct_tag()
            .is_some_and(|struct_tag| struct_tag.name().as_str() == "Counter")
        {
            counter = Some(SharedObjectReference::new(
                object_id,
                initial_shared_version,
                true,
            ));
        }
    }
    let package_id = package_id.expect("publish should create the package object");
    let counter = counter.expect("init should create the shared Counter object");

    wait_for_executed_transactions_checkpointed(test_cluster, client).await;

    (package_id, counter)
}

/// Build a `InputArgument` carrying `value` as a JSON string (used
/// both for object-id arguments and for pure values, matching how the
/// endpoint resolves `json` arguments against the callee's signature).
fn json_arg(value: impl Into<String>) -> InputArgument {
    InputArgument::default().with_json(Value {
        kind: Some(Kind::StringValue(value.into())),
    })
}

/// Build a single-call `ViewFunctionCallsRequest` with no type arguments.
fn single_call_request(
    fq_function_name: String,
    arguments: Vec<InputArgument>,
) -> ViewFunctionCallsRequest {
    let item = ViewFunctionCallItem::default()
        .with_fq_function_name(fq_function_name)
        .with_type_args(vec![])
        .with_inputs(arguments);
    ViewFunctionCallsRequest::default().with_view_function_calls(vec![item])
}

/// Call `{package_id}::counter::value(counter)` and return the response.
async fn call_counter_value(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    package_id: ObjectId,
    counter_id: ObjectId,
) -> Result<ViewFunctionCallsResponse, tonic::Status> {
    let request = single_call_request(
        format!("{package_id}::counter::value"),
        vec![json_arg(counter_id.to_string())],
    );
    exec_client
        .view_function_calls(request)
        .await
        .map(tonic::Response::into_inner)
}

/// Extract the single `CommandOutput.json` value from a one-call,
/// one-return-value `ViewFunctionCallsResponse`.
fn single_json_output(response: &ViewFunctionCallsResponse) -> &Value {
    let result = response
        .call_results
        .first()
        .expect("expected one function call result")
        .result
        .as_ref()
        .expect("expected view_function_call_result to be present");
    match result {
        view_function_call_result::Result::CallOutputs(outputs) => {
            let result = outputs
                .execution_result
                .as_ref()
                .expect("expected execution_result to be present");
            match result {
                view_function_call_outputs::ExecutionResult::ReturnValues(outputs) => outputs
                    .outputs
                    .first()
                    .expect("expected one command output")
                    .json
                    .as_ref()
                    .expect("expected json output to be present"),
                view_function_call_outputs::ExecutionResult::ExecutionError(_) => {
                    panic!("view function call failed")
                }
                _ => panic!("expected result to be successful"),
            }
        }
        _ => panic!("expected view_function_call_result to be successful"),
    }
}

#[sim_test]
async fn view_returns_object_field() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();

    let (package_id, counter) = publish_view_functions_package(&test_cluster, &client).await;

    let response = call_counter_value(&mut exec_client, package_id, counter.object_id)
        .await
        .expect("view call to counter::value should succeed");

    assert_eq!(
        single_json_output(&response),
        &Value {
            kind: Some(Kind::StringValue("42".to_string())),
        },
        "counter::value should return the seeded value 42 as a JSON string"
    );
}

#[sim_test]
async fn non_view_function_is_rejected() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();

    let (package_id, counter) = publish_view_functions_package(&test_cluster, &client).await;

    let request = single_call_request(
        format!("{package_id}::counter::value_not_view"),
        vec![json_arg(counter.object_id.to_string())],
    );

    // The server rejects the call before running it, reported as a per-call
    // error rather than a request-level error.
    let response = exec_client
        .view_function_calls(request)
        .await
        .expect("the request should succeed with a per-call error")
        .into_inner();

    let result = response
        .call_results
        .first()
        .expect("expected one result")
        .result
        .as_ref()
        .expect("expected a result");
    match result {
        view_function_call_result::Result::Error(status) => {
            assert_eq!(status.code, tonic::Code::InvalidArgument as i32);
            assert!(
                status.message.contains("#[view]"),
                "expected error message to mention #[view], got: {}",
                status.message
            );
        }
        other => panic!("expected a per-call error for the non-view function, got {other:?}"),
    }
}

#[sim_test]
async fn no_metadata_function_is_rejected() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();

    let (package_id, _counter) = publish_view_functions_package(&test_cluster, &client).await;

    let request = single_call_request(format!("{package_id}::plain::forty"), vec![]);

    // A module with no view metadata records no view functions, so the call is
    // rejected — as a per-call error, not a request-level error.
    let response = exec_client
        .view_function_calls(request)
        .await
        .expect("the request should succeed with a per-call error")
        .into_inner();

    let result = response
        .call_results
        .first()
        .expect("expected one result")
        .result
        .as_ref()
        .expect("expected a result");
    match result {
        view_function_call_result::Result::Error(status) => {
            assert_eq!(status.code, tonic::Code::InvalidArgument as i32);
        }
        other => panic!("expected a per-call error for the no-metadata function, got {other:?}"),
    }
}

#[sim_test]
async fn view_reflects_state_change() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();

    let (package_id, counter) = publish_view_functions_package(&test_cluster, &client).await;

    let response = call_counter_value(&mut exec_client, package_id, counter.object_id)
        .await
        .expect("view call to counter::value should succeed");
    assert_eq!(
        single_json_output(&response),
        &Value {
            kind: Some(Kind::StringValue("42".to_string())),
        },
        "counter::value should return 42 before bump"
    );

    // Mutate on-chain state with a normal transaction, then re-run the view
    // call and confirm it reflects the new value.
    let sender = first_sender(&test_cluster);
    let tx_data = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .move_call(
            package_id,
            "counter",
            "bump",
            vec![CallArg::Shared(counter)],
        )
        .build();
    let signed_tx = test_cluster.sign_transaction(&tx_data);
    let (bump_effects, _) = test_cluster
        .execute_transaction_return_raw_effects(signed_tx)
        .await
        .expect("bump transaction should be submitted");
    assert!(
        bump_effects.status().is_success(),
        "bump transaction should succeed: {:?}",
        bump_effects.status()
    );
    wait_for_executed_transactions_checkpointed(&test_cluster, &client).await;

    let response = call_counter_value(&mut exec_client, package_id, counter.object_id)
        .await
        .expect("view call to counter::value should succeed");
    assert_eq!(
        single_json_output(&response),
        &Value {
            kind: Some(Kind::StringValue("43".to_string())),
        },
        "counter::value should return 43 after bump"
    );
}
