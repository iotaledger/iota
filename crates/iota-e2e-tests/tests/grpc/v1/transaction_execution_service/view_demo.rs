// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr};

use iota_grpc_types::v1::{
    command::{CommandOutput, InputArgument},
    transaction_execution_service::{
        ViewFunctionCallItem, ViewFunctionCallsRequest, ViewFunctionCallsResponse,
        transaction_execution_service_client::TransactionExecutionServiceClient,
        view_function_call_outputs, view_function_call_result,
    },
    types::TypeTag as ProtoTypeTag,
};
use iota_json_rpc_types::ObjectChange;
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_sdk_types::{Address, ObjectId, Owner, SharedObjectReference, TypeTag};
use iota_test_transaction_builder::PublishData;
use prost_types::{ListValue, Struct, Value, value::Kind};
use test_cluster::TestCluster;

use crate::utils::{first_sender, setup_grpc_test, wait_for_executed_transactions_checkpointed};

/// Path to the `view_demo` test package (`Move.toml` + `sources/`).
fn view_demo_package_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "grpc", "data", "view_demo"]);
    path
}

/// The objects created by publishing `view_demo`.
struct ViewDemoPackage {
    package_id: ObjectId,
    shop: SharedObjectReference,
    box_u64: SharedObjectReference,
    owner: Address,
}

/// Publish the `view_demo` test package and return the package id, the
/// shared `Shop` and `Box<u64>` objects seeded by `init`, and the publishing
/// sender (the `Shop.owner`).
///
/// Waits for the publish transaction to land in a checkpoint before
/// returning: the `view_function_calls` endpoint reads through the same
/// checkpoint-indexed state as the rest of the gRPC service, which lags
/// behind local (`WaitForLocalExecution`) visibility.
async fn publish_view_demo_package(
    test_cluster: &TestCluster,
    client: &iota_grpc_client::Client,
) -> ViewDemoPackage {
    let sender = first_sender(test_cluster);

    // `with_allow_view_function` is required for the `#[view]` attribute to be
    // compiled into on-chain module metadata.
    let compiled_package = BuildConfig::new_for_testing()
        .with_allow_view_function()
        .build(&view_demo_package_path())
        .expect("view_demo package should build");

    let tx_data = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .publish_with_data(PublishData::CompiledPackage(compiled_package))
        .build();
    let signed_tx = test_cluster.sign_transaction(&tx_data);
    let response = test_cluster.execute_transaction(signed_tx).await;
    assert_eq!(
        response.status_ok(),
        Some(true),
        "publishing view_demo package should succeed"
    );

    let object_changes = response
        .object_changes
        .expect("publish response should include object_changes");
    let package_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .expect("publish should create the package object");

    let shared_object_named = |name: &str| {
        object_changes
            .iter()
            .find_map(|change| match change {
                ObjectChange::Created {
                    object_id,
                    owner: Owner::Shared(initial_shared_version),
                    object_type,
                    ..
                } if object_type.name().as_str() == name => Some(SharedObjectReference::new(
                    *object_id,
                    *initial_shared_version,
                    true,
                )),
                _ => None,
            })
            .unwrap_or_else(|| panic!("init should create the shared {name} object"))
    };
    let shop = shared_object_named("Shop");
    let box_u64 = shared_object_named("Box");

    wait_for_executed_transactions_checkpointed(test_cluster, client).await;

    ViewDemoPackage {
        package_id,
        shop,
        box_u64,
        owner: sender,
    }
}

/// Build a `InputArgument` carrying `value` as a JSON string (used
/// both for object-id arguments and for pure values, matching how the
/// endpoint resolves `json` arguments against the callee's signature).
fn json_arg(value: impl Into<String>) -> InputArgument {
    InputArgument::default().with_json(Value {
        kind: Some(Kind::StringValue(value.into())),
    })
}

/// Build a `InputArgument` carrying `value` as a JSON bool.
fn bool_arg(value: bool) -> InputArgument {
    InputArgument::default().with_json(Value {
        kind: Some(Kind::BoolValue(value)),
    })
}

/// Build a proto `TypeTag` for a type-tag string such as `"0x2::iota::IOTA"`,
/// `"u64"`, or `"bool"`.
fn type_tag(tag: &str) -> ProtoTypeTag {
    let tag = TypeTag::from_str(tag).unwrap_or_else(|e| panic!("invalid type tag `{tag}`: {e}"));
    (&tag).into()
}

/// Build a single-call `ViewFunctionCallsRequest`.
fn single_call_request(
    fq_function_name: String,
    type_args: Vec<ProtoTypeTag>,
    arguments: Vec<InputArgument>,
) -> ViewFunctionCallsRequest {
    let item = ViewFunctionCallItem::default()
        .with_fq_function_name(fq_function_name)
        .with_type_args(type_args)
        .with_inputs(arguments);
    ViewFunctionCallsRequest::default().with_view_function_calls(vec![item])
}

/// Call the `view_demo` endpoint for one `fq_function_name`/`type_args`/
/// `arguments` and return the response.
async fn call_view(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    fq_function_name: String,
    type_args: Vec<ProtoTypeTag>,
    arguments: Vec<InputArgument>,
) -> Result<ViewFunctionCallsResponse, tonic::Status> {
    let request = single_call_request(fq_function_name, type_args, arguments);
    exec_client
        .view_function_calls(request)
        .await
        .map(tonic::Response::into_inner)
}

/// Return the `outputs` of the `call_idx`-th function call result.
fn outputs(response: &ViewFunctionCallsResponse, call_idx: usize) -> &[CommandOutput] {
    let result = response
        .call_results
        .get(call_idx)
        .unwrap_or_else(|| panic!("expected a function call result at index {call_idx}"))
        .result
        .as_ref()
        .expect("expected outputs to be present");
    match result {
        view_function_call_result::Result::CallOutputs(outputs) => {
            let result = outputs
                .execution_result
                .as_ref()
                .expect("expected outputs must be present");
            match result {
                view_function_call_outputs::ExecutionResult::ReturnValues(outputs) => {
                    &outputs.outputs
                }
                view_function_call_outputs::ExecutionResult::ExecutionError(_) => {
                    panic!("view function call failed")
                }
                _ => panic!("expected result to be successful"),
            }
        }
        _ => panic!("expected outputs to be successful"),
    }
}

/// Extract the `output_idx`-th `CommandOutput.json` value of the
/// `call_idx`-th function call result.
fn json_output(response: &ViewFunctionCallsResponse, call_idx: usize, output_idx: usize) -> &Value {
    outputs(response, call_idx)
        .get(output_idx)
        .unwrap_or_else(|| panic!("expected a command output at index {output_idx}"))
        .json
        .as_ref()
        .expect("expected json output to be present")
}

/// Extract the single `CommandOutput.json` value from a one-call,
/// one-return-value `ViewFunctionCallsResponse`.
fn single_json_output(response: &ViewFunctionCallsResponse) -> &Value {
    json_output(response, 0, 0)
}

fn string_value(s: impl Into<String>) -> Value {
    Value {
        kind: Some(Kind::StringValue(s.into())),
    }
}

fn bool_value(b: bool) -> Value {
    Value {
        kind: Some(Kind::BoolValue(b)),
    }
}

fn null_value() -> Value {
    Value {
        kind: Some(Kind::NullValue(0)),
    }
}

fn list_value(values: Vec<Value>) -> Value {
    Value {
        kind: Some(Kind::ListValue(ListValue { values })),
    }
}

/// Build a `Value` rendering a Move struct: a `StructValue` whose fields are
/// keyed by the Move field names, matching how the endpoint renders struct
/// return values.
fn struct_value(fields: Vec<(&str, Value)>) -> Value {
    Value {
        kind: Some(Kind::StructValue(Struct {
            fields: fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        })),
    }
}

#[sim_test]
async fn discounted_price_pure_args() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::discounted_price", pkg.package_id),
        vec![],
        vec![json_arg("100"), json_arg("25")],
    )
    .await
    .expect("discounted_price view call should succeed");

    assert_eq!(single_json_output(&response), &string_value("75"));
}

#[sim_test]
async fn total_revenue_sums_sales() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::total_revenue", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string())],
    )
    .await
    .expect("total_revenue view call should succeed");

    assert_eq!(single_json_output(&response), &string_value("4000"));
}

#[sim_test]
async fn name_reference_return() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::name", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string())],
    )
    .await
    .expect("name view call should succeed");

    assert_eq!(
        single_json_output(&response),
        &string_value("IOTA Merch Store")
    );
}

#[sim_test]
async fn sales_vector_return() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::sales", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string())],
    )
    .await
    .expect("sales view call should succeed");

    assert_eq!(
        single_json_output(&response),
        &list_value(vec![
            string_value("1000"),
            string_value("2500"),
            string_value("500"),
        ])
    );
}

#[sim_test]
async fn sale_at_option_some_and_none() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let some_response = call_view(
        &mut exec_client,
        format!("{}::shop::sale_at", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string()), json_arg("1")],
    )
    .await
    .expect("sale_at(1) view call should succeed");
    assert_eq!(single_json_output(&some_response), &string_value("2500"));

    let none_response = call_view(
        &mut exec_client,
        format!("{}::shop::sale_at", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string()), json_arg("99")],
    )
    .await
    .expect("sale_at(99) view call should succeed");
    assert_eq!(single_json_output(&none_response), &null_value());
}

#[sim_test]
async fn stats_multiple_return_values() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::stats", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string())],
    )
    .await
    .expect("stats view call should succeed");

    let call_outputs = outputs(&response, 0);
    assert_eq!(
        call_outputs.len(),
        3,
        "stats should produce three command outputs"
    );
    assert_eq!(json_output(&response, 0, 0), &string_value("4000"));
    assert_eq!(json_output(&response, 0, 1), &string_value("3"));
    assert_eq!(
        json_output(&response, 0, 2),
        &string_value(pkg.owner.to_string())
    );
}

#[sim_test]
async fn summary_struct_return() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::summary", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string())],
    )
    .await
    .expect("summary view call should succeed");

    assert_eq!(
        single_json_output(&response),
        &struct_value(vec![
            ("total_revenue", string_value("4000")),
            ("sale_count", string_value("3")),
            ("owner", string_value(pkg.owner.to_string())),
        ])
    );
}

#[sim_test]
async fn is_owner_address_argument() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::is_owner", pkg.package_id),
        vec![],
        vec![
            json_arg(pkg.shop.object_id.to_string()),
            json_arg(pkg.owner.to_string()),
        ],
    )
    .await
    .expect("is_owner view call should succeed");

    assert_eq!(single_json_output(&response), &bool_value(true));
}

#[sim_test]
async fn type_name_of_generic_type_arg() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::generics::type_name_of", pkg.package_id),
        vec![type_tag("0x2::iota::IOTA")],
        vec![],
    )
    .await
    .expect("type_name_of view call should succeed");

    let value = single_json_output(&response);
    let Kind::StringValue(name) = value.kind.as_ref().expect("expected a string value") else {
        panic!("expected a string value, got {value:?}");
    };
    assert!(
        name.ends_with("::iota::IOTA"),
        "expected type name to end with `::iota::IOTA`, got {name}"
    );
}

#[sim_test]
async fn type_name_of_vector_u8() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::generics::type_name_of", pkg.package_id),
        vec![type_tag("vector<u8>")],
        vec![],
    )
    .await
    .expect("type_name_of<vector<u8>> view call should succeed");

    assert_eq!(single_json_output(&response), &string_value("vector<u8>"));
}

#[sim_test]
async fn echo_generic_value() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let u64_response = call_view(
        &mut exec_client,
        format!("{}::generics::echo", pkg.package_id),
        vec![type_tag("u64")],
        vec![json_arg("42")],
    )
    .await
    .expect("echo<u64> view call should succeed");
    assert_eq!(single_json_output(&u64_response), &string_value("42"));

    let bool_response = call_view(
        &mut exec_client,
        format!("{}::generics::echo", pkg.package_id),
        vec![type_tag("bool")],
        vec![bool_arg(true)],
    )
    .await
    .expect("echo<bool> view call should succeed");
    assert_eq!(single_json_output(&bool_response), &bool_value(true));
}

#[sim_test]
async fn boxed_item_generic_object() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::generics::boxed_item", pkg.package_id),
        vec![type_tag("u64")],
        vec![json_arg(pkg.box_u64.object_id.to_string())],
    )
    .await
    .expect("boxed_item view call should succeed");

    assert_eq!(single_json_output(&response), &string_value("7"));
}

#[sim_test]
async fn record_sale_non_view_is_rejected() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let request = single_call_request(
        format!("{}::shop::record_sale", pkg.package_id),
        vec![],
        vec![json_arg(pkg.shop.object_id.to_string()), json_arg("1")],
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

/// One request with three items: each call runs in its own dev-inspect
/// transaction and the response carries one `call_results` entry
/// per item, in the same order.
#[sim_test]
async fn batch_request_multiple_calls() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let items = vec![
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{}::shop::discounted_price", pkg.package_id))
            .with_type_args(vec![])
            .with_inputs(vec![json_arg("100"), json_arg("25")]),
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{}::shop::total_revenue", pkg.package_id))
            .with_type_args(vec![])
            .with_inputs(vec![json_arg(pkg.shop.object_id.to_string())]),
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{}::shop::name", pkg.package_id))
            .with_type_args(vec![])
            .with_inputs(vec![json_arg(pkg.shop.object_id.to_string())]),
    ];
    let request = ViewFunctionCallsRequest::default().with_view_function_calls(items);

    let response = exec_client
        .view_function_calls(request)
        .await
        .expect("batch view call should succeed")
        .into_inner();

    assert_eq!(
        response.call_results.len(),
        3,
        "expected one call_results entry per requested item"
    );
    assert_eq!(json_output(&response, 0, 0), &string_value("75"));
    assert_eq!(json_output(&response, 1, 0), &string_value("4000"));
    assert_eq!(
        json_output(&response, 2, 0),
        &string_value("IOTA Merch Store")
    );
}

#[sim_test]
async fn open_for_ms_reads_clock() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let pkg = publish_view_demo_package(&test_cluster, &client).await;

    let response = call_view(
        &mut exec_client,
        format!("{}::shop::open_for_ms", pkg.package_id),
        vec![],
        vec![
            json_arg(pkg.shop.object_id.to_string()),
            json_arg(ObjectId::from_str("0x6").unwrap().to_string()),
        ],
    )
    .await
    .expect("open_for_ms view call should succeed");

    // The clock timestamp is non-deterministic; just check it parses as a u64.
    let value = single_json_output(&response);
    let Kind::StringValue(millis) = value.kind.as_ref().expect("expected a string value") else {
        panic!("expected a string value, got {value:?}");
    };
    millis
        .parse::<u64>()
        .unwrap_or_else(|e| panic!("expected open_for_ms to return a u64, got `{millis}`: {e}"));
}
