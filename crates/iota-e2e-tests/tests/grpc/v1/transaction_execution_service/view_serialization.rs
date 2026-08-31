// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `ViewFunctionCalls` endpoint that exercise:
//!
//! - the value-serialization layer, in both directions, for a range of Move
//!   types — a value returned by a `#[view]` function is rendered to both BCS
//!   (`CommandOutput.bcs`) and JSON (`CommandOutput.json`);
//! - the argument-resolution layer — an input argument supplied either as a
//!   JSON value or as pre-encoded BCS is resolved against the callee's Move
//!   signature into the correct on-chain value;
//! - per-call independence — each call runs in its own transaction, so one call
//!   aborting or being rejected leaves the others' results intact, and a client
//!   can split many calls across several batched requests;
//! - read-mask handling — which output fields a valid mask selects, and that
//!   invalid mask paths are rejected;
//! - argument-resolution errors — too few or too many arguments, and a value
//!   that does not match a generic parameter's type.
//!
//! The `view_demo::echo` module returns each input unchanged, so a successful
//! call's return value equals its argument, letting each test assert a full
//! round trip.

use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

use iota_grpc_types::v1::{
    bcs::BcsData,
    command::{CommandOutput, InputArgument},
    transaction_execution_service::{
        ViewFunctionCallItem, ViewFunctionCallResult, ViewFunctionCallsRequest,
        ViewFunctionCallsResponse,
        transaction_execution_service_client::TransactionExecutionServiceClient,
        view_function_call_outputs::ExecutionResult,
        view_function_call_result::Result as CallResult,
    },
    types::TypeTag as ProtoTypeTag,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_sdk_types::{ObjectId, TypeTag};
use iota_test_transaction_builder::PublishData;
use iota_types::effects::TransactionEffectsAPI;
use prost_types::{FieldMask, Struct, Value, value::Kind};
use serde::Serialize;
use test_cluster::TestCluster;

use crate::utils::{first_sender, setup_grpc_test, wait_for_executed_transactions_checkpointed};

/// Rust mirror of `view_demo::echo::Pair`, in declaration order, so
/// `bcs::to_bytes` produces the same bytes as the Move value.
#[derive(Serialize)]
struct Pair {
    u: u128,
    b: Vec<u8>,
}

fn view_demo_package_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "grpc", "data", "view_demo"]);
    path
}

/// Publish the `view_demo` package and return its id. Waits for the publish to
/// land in a checkpoint, since the endpoint reads checkpoint-indexed state.
async fn publish_view_demo(
    test_cluster: &TestCluster,
    client: &iota_grpc_client::Client,
) -> ObjectId {
    let sender = first_sender(test_cluster);
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
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(signed_tx)
        .await
        .expect("publishing view_demo package should be submitted");
    assert!(
        effects.status().is_success(),
        "publishing view_demo package should succeed: {:?}",
        effects.status()
    );

    // A publish creates the package alongside its metadata objects, which are
    // immutable too, so the package is picked by reading the created objects
    // back from the store.
    let mut package_id = None;
    for object in &effects.created() {
        let object_id = object.reference.object_id;
        let stored = test_cluster
            .get_object_from_fullnode_store(&object_id)
            .await
            .expect("a created object should be in the store");
        if stored.data.is_package() {
            package_id = Some(object_id);
            break;
        }
    }
    let package_id = package_id.expect("publish should create the package object");

    wait_for_executed_transactions_checkpointed(test_cluster, client).await;
    package_id
}

fn string_value(s: impl Into<String>) -> Value {
    Value {
        kind: Some(Kind::StringValue(s.into())),
    }
}

fn number_value(n: f64) -> Value {
    Value {
        kind: Some(Kind::NumberValue(n)),
    }
}

fn struct_value(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let fields: BTreeMap<String, Value> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    Value {
        kind: Some(Kind::StructValue(Struct { fields })),
    }
}

fn json_arg(value: Value) -> InputArgument {
    InputArgument::default().with_json(value)
}

fn bcs_arg(bytes: Vec<u8>) -> InputArgument {
    InputArgument::default().with_bcs(BcsData::default().with_data(bytes))
}

/// Call one `#[view]` function (no type arguments) and return the response.
async fn call(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    fq_function_name: String,
    inputs: Vec<InputArgument>,
) -> std::result::Result<ViewFunctionCallsResponse, tonic::Status> {
    let item = ViewFunctionCallItem::default()
        .with_fq_function_name(fq_function_name)
        .with_type_args(vec![])
        .with_inputs(inputs);
    let request = ViewFunctionCallsRequest::default().with_view_function_calls(vec![item]);
    exec_client
        .view_function_calls(request)
        .await
        .map(tonic::Response::into_inner)
}

/// Extract the single `CommandOutput` from a call result, asserting the call
/// ran and returned exactly one value.
fn command_output_of(result: &ViewFunctionCallResult) -> &CommandOutput {
    let result = result.result.as_ref().expect("expected a call result");
    let CallResult::CallOutputs(outputs) = result else {
        panic!("expected the call to run, got {result:?}");
    };
    let execution_result = outputs
        .execution_result
        .as_ref()
        .expect("expected an execution result");
    let ExecutionResult::ReturnValues(return_values) = execution_result else {
        panic!("expected the call to return values, got {execution_result:?}");
    };
    return_values
        .outputs
        .first()
        .expect("expected one command output")
}

/// Extract the single `CommandOutput` from a one-call, one-return-value
/// response.
fn single_command_output(response: &ViewFunctionCallsResponse) -> &CommandOutput {
    command_output_of(
        response
            .call_results
            .first()
            .expect("expected one call result"),
    )
}

/// Assert a value round-trips through the endpoint: calling `echo_*` with the
/// value (as BCS, and as JSON when `json_input` is `Some`) returns a
/// `CommandOutput` whose `bcs` equals `value_bcs` and whose `json` equals
/// `expected_json`.
async fn assert_round_trip(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    package_id: ObjectId,
    function: &str,
    json_input: Option<Value>,
    value_bcs: Vec<u8>,
    expected_json: Value,
) {
    let fq = format!("{package_id}::echo::{function}");

    // BCS input: the pre-encoded bytes are used as the pure argument directly.
    let response = call(exec_client, fq.clone(), vec![bcs_arg(value_bcs.clone())])
        .await
        .unwrap_or_else(|e| panic!("{function} (bcs input) should succeed: {e}"));
    let output = single_command_output(&response);
    assert_eq!(
        output.bcs.as_ref().expect("bcs output").data.as_ref(),
        value_bcs.as_slice(),
        "{function} (bcs input): returned bcs should equal the input bcs"
    );
    assert_eq!(
        output.json.as_ref().expect("json output"),
        &expected_json,
        "{function} (bcs input): returned json"
    );

    // JSON input: resolved against the parameter's Move type into the same
    // on-chain value, so the returned bcs must match.
    if let Some(json_input) = json_input {
        let response = call(exec_client, fq, vec![json_arg(json_input)])
            .await
            .unwrap_or_else(|e| panic!("{function} (json input) should succeed: {e}"));
        let output = single_command_output(&response);
        assert_eq!(
            output.bcs.as_ref().expect("bcs output").data.as_ref(),
            value_bcs.as_slice(),
            "{function} (json input): resolved value's bcs should equal the expected bcs"
        );
        assert_eq!(
            output.json.as_ref().expect("json output"),
            &expected_json,
            "{function} (json input): returned json"
        );
    }
}

#[sim_test]
async fn values_round_trip_through_json_and_bcs() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let package_id = publish_view_demo(&test_cluster, &client).await;

    // Integer arguments are passed as JSON strings: a prost `Value` number is
    // an `f64`, which cannot represent a `u64`/`u128`/`u256` exactly, so the
    // resolver only accepts integers as strings. `u8`/`u32` render back as JSON
    // numbers; wider integers render as JSON strings.
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_u8",
        Some(string_value("255")),
        bcs::to_bytes(&255u8).unwrap(),
        number_value(255.0),
    )
    .await;
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_u32",
        Some(string_value("4294967295")),
        bcs::to_bytes(&u32::MAX).unwrap(),
        number_value(u32::MAX as f64),
    )
    .await;
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_u64",
        Some(string_value(u64::MAX.to_string())),
        bcs::to_bytes(&u64::MAX).unwrap(),
        string_value(u64::MAX.to_string()),
    )
    .await;
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_u128",
        Some(string_value(u128::MAX.to_string())),
        bcs::to_bytes(&u128::MAX).unwrap(),
        string_value(u128::MAX.to_string()),
    )
    .await;
    // u256 = 7, encoded as 32 little-endian bytes.
    let mut u256_bcs = vec![0u8; 32];
    u256_bcs[0] = 7;
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_u256",
        Some(string_value("7")),
        u256_bcs,
        string_value("7"),
    )
    .await;

    // vector<u8> [1, 2, 3]: bcs is a ULEB length prefix followed by the bytes;
    // json renders arbitrary bytes as base64 ("AQID" == [1, 2, 3]). As a JSON
    // input it is given as a 0x-prefixed hex string.
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_bytes",
        Some(string_value("0x010203")),
        bcs::to_bytes(&vec![1u8, 2, 3]).unwrap(),
        string_value("AQID"),
    )
    .await;

    // struct Pair { u: u128, b: vector<u8> }: JSON input is not supported for a
    // user struct (the resolver treats a non-primitive parameter as an object
    // reference), so it is supplied only as BCS. The return renders as a JSON
    // object keyed by field name.
    let pair = Pair {
        u: 42,
        b: vec![1, 2, 3],
    };
    assert_round_trip(
        &mut exec_client,
        package_id,
        "echo_pair",
        None,
        bcs::to_bytes(&pair).unwrap(),
        struct_value([("u", string_value("42")), ("b", string_value("AQID"))]),
    )
    .await;
}

#[sim_test]
async fn per_call_failures_are_independent() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let package_id = publish_view_demo(&test_cluster, &client).await;

    // One request, four calls with distinct outcomes: a success, an on-chain
    // abort, a non-`#[view]` rejection, and a missing-function rejection. Each
    // runs in its own transaction, so every result stands on its own.
    let items = vec![
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{package_id}::echo::echo_u64"))
            .with_type_args(vec![])
            .with_inputs(vec![json_arg(string_value("5"))]),
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{package_id}::echo::always_aborts"))
            .with_type_args(vec![])
            .with_inputs(vec![]),
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{package_id}::echo::not_view"))
            .with_type_args(vec![])
            .with_inputs(vec![]),
        ViewFunctionCallItem::default()
            .with_fq_function_name(format!("{package_id}::echo::does_not_exist"))
            .with_type_args(vec![])
            .with_inputs(vec![]),
    ];
    let request = ViewFunctionCallsRequest::default().with_view_function_calls(items);

    let response = exec_client
        .view_function_calls(request)
        .await
        .expect("the batch request itself should succeed")
        .into_inner();

    let results = &response.call_results;
    assert_eq!(results.len(), 4, "one result per requested call, in order");

    // 0: success — the call ran and returned 5.
    let success = command_output_of(&results[0]);
    assert_eq!(
        success.json.as_ref().expect("json output"),
        &string_value("5"),
        "first call should return 5"
    );

    // 1: on-chain abort — the call ran but aborted, reported as an execution
    // error within the outputs (not a request-level error).
    match results[1].result.as_ref().expect("a result") {
        CallResult::CallOutputs(outputs) => {
            match outputs.execution_result.as_ref().expect("result") {
                ExecutionResult::ExecutionError(_) => {}
                other => panic!("expected an execution error for the aborting call, got {other:?}"),
            }
        }
        other => panic!("expected the aborting call to run, got {other:?}"),
    }

    // 2: not a `#[view]` function — rejected before running, as a per-call error.
    match results[2].result.as_ref().expect("a result") {
        CallResult::Error(status) => {
            assert_eq!(status.code, tonic::Code::InvalidArgument as i32);
            assert!(
                status.message.contains("#[view]"),
                "expected a #[view] rejection, got: {}",
                status.message
            );
        }
        other => panic!("expected a per-call error for the non-view function, got {other:?}"),
    }

    // 3: missing function — a different rejection cause, independent of the rest.
    match results[3].result.as_ref().expect("a result") {
        CallResult::Error(status) => {
            assert_eq!(status.code, tonic::Code::InvalidArgument as i32);
        }
        other => panic!("expected a per-call error for the missing function, got {other:?}"),
    }
}

/// Build a proto `TypeTag` from a type-tag string such as `"u64"`.
fn type_tag(tag: &str) -> ProtoTypeTag {
    let tag = TypeTag::from_str(tag).unwrap_or_else(|e| panic!("invalid type tag `{tag}`: {e}"));
    (&tag).into()
}

/// Call one `#[view]` function with a single-path read mask.
async fn call_with_mask(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    fq_function_name: &str,
    inputs: Vec<InputArgument>,
    mask_path: &str,
) -> std::result::Result<ViewFunctionCallsResponse, tonic::Status> {
    let item = ViewFunctionCallItem::default()
        .with_fq_function_name(fq_function_name.to_string())
        .with_type_args(vec![])
        .with_inputs(inputs);
    let request = ViewFunctionCallsRequest::default()
        .with_view_function_calls(vec![item])
        .with_read_mask(FieldMask {
            paths: vec![mask_path.to_string()],
        });
    exec_client
        .view_function_calls(request)
        .await
        .map(tonic::Response::into_inner)
}

/// Assert the first (and only) call in a response is a per-call
/// `InvalidArgument` error whose message contains `message_contains`.
fn assert_first_call_rejected(response: &ViewFunctionCallsResponse, message_contains: &str) {
    match response
        .call_results
        .first()
        .expect("expected one result")
        .result
        .as_ref()
        .expect("expected a result")
    {
        CallResult::Error(status) => {
            assert_eq!(
                status.code,
                tonic::Code::InvalidArgument as i32,
                "expected InvalidArgument, got code {}: {}",
                status.code,
                status.message
            );
            assert!(
                status.message.contains(message_contains),
                "expected message to contain `{message_contains}`, got: {}",
                status.message
            );
        }
        other => panic!("expected a per-call error, got {other:?}"),
    }
}

#[sim_test]
async fn calls_are_batched_across_requests() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let package_id = publish_view_demo(&test_cluster, &client).await;

    /// Whether a batched call should return a value or be rejected.
    enum Expect {
        Returns(&'static str),
        Rejected,
    }

    // Five calls with mixed outcomes. The client sends them in batches of two —
    // three requests of sizes 2, 2, and 1 — and the server answers each batch
    // with one result per call, in order.
    let calls = [
        (
            format!("{package_id}::echo::echo_u64"),
            vec![json_arg(string_value("10"))],
            Expect::Returns("10"),
        ),
        (
            format!("{package_id}::echo::not_view"),
            vec![],
            Expect::Rejected,
        ),
        (
            format!("{package_id}::echo::echo_u64"),
            vec![json_arg(string_value("20"))],
            Expect::Returns("20"),
        ),
        (
            format!("{package_id}::echo::does_not_exist"),
            vec![],
            Expect::Rejected,
        ),
        (
            format!("{package_id}::echo::echo_u64"),
            vec![json_arg(string_value("30"))],
            Expect::Returns("30"),
        ),
    ];

    let mut batch_sizes = Vec::new();
    let mut results = Vec::new();
    for chunk in calls.chunks(2) {
        let items = chunk
            .iter()
            .map(|(fq, inputs, _)| {
                ViewFunctionCallItem::default()
                    .with_fq_function_name(fq.clone())
                    .with_type_args(vec![])
                    .with_inputs(inputs.clone())
            })
            .collect();
        let response = exec_client
            .view_function_calls(
                ViewFunctionCallsRequest::default().with_view_function_calls(items),
            )
            .await
            .expect("each batch request should succeed")
            .into_inner();
        batch_sizes.push(response.call_results.len());
        results.extend(response.call_results);
    }

    assert_eq!(batch_sizes, vec![2, 2, 1], "three batches of sizes 2, 2, 1");
    assert_eq!(results.len(), 5, "one result per call across all batches");

    for (result, (_, _, expect)) in results.iter().zip(&calls) {
        match expect {
            Expect::Returns(value) => assert_eq!(
                command_output_of(result)
                    .json
                    .as_ref()
                    .expect("json output"),
                &string_value(*value),
            ),
            Expect::Rejected => assert!(
                matches!(
                    result.result.as_ref().expect("a result"),
                    CallResult::Error(_)
                ),
                "expected a per-call error, got {:?}",
                result.result,
            ),
        }
    }
}

#[sim_test]
async fn read_mask_selects_output_fields() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let package_id = publish_view_demo(&test_cluster, &client).await;
    let fq = format!("{package_id}::echo::echo_u64");
    let input = || vec![json_arg(string_value("9"))];

    // Read mask paths are rooted at `ViewFunctionCallOutputs`. Selecting only
    // `bcs` yields the bcs output but not json or type_tag.
    let response = call_with_mask(
        &mut exec_client,
        &fq,
        input(),
        "execution_result.return_values.bcs",
    )
    .await
    .expect("a valid read mask should be accepted");
    let output = single_command_output(&response);
    assert!(output.bcs.is_some(), "bcs should be present");
    assert!(output.json.is_none(), "json should be excluded");
    assert!(output.type_tag.is_none(), "type_tag should be excluded");

    // Selecting only `json` yields json but not bcs.
    let response = call_with_mask(
        &mut exec_client,
        &fq,
        input(),
        "execution_result.return_values.json",
    )
    .await
    .expect("a valid read mask should be accepted");
    let output = single_command_output(&response);
    assert!(output.json.is_some(), "json should be present");
    assert!(output.bcs.is_none(), "bcs should be excluded");

    // Selecting the whole `return_values` yields every command-output field.
    let response = call_with_mask(
        &mut exec_client,
        &fq,
        input(),
        "execution_result.return_values",
    )
    .await
    .expect("a valid read mask should be accepted");
    let output = single_command_output(&response);
    assert!(
        output.bcs.is_some() && output.json.is_some() && output.type_tag.is_some(),
        "the whole return_values subtree should include every field: {output:?}"
    );

    // Paths that are not valid fields of `ViewFunctionCallOutputs` are rejected
    // for the whole request. `call_outputs` is a field of the outer
    // `ViewFunctionCallResult`, not of the outputs the mask applies to.
    for invalid in [
        "call_outputs",
        "outputs",
        "execution_result.return_values.nonexistent",
    ] {
        let error = call_with_mask(&mut exec_client, &fq, input(), invalid)
            .await
            .expect_err("an invalid read mask path should be rejected");
        assert_eq!(
            error.code(),
            tonic::Code::InvalidArgument,
            "invalid mask `{invalid}` should be rejected with InvalidArgument"
        );
        assert!(
            error.message().contains("read_mask"),
            "expected a read_mask error for `{invalid}`, got: {}",
            error.message()
        );
    }
}

#[sim_test]
async fn argument_resolution_errors_are_reported_per_call() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut exec_client = client.execution_service_client();
    let package_id = publish_view_demo(&test_cluster, &client).await;

    // Too few arguments: `echo_u64` takes one.
    let response = call(
        &mut exec_client,
        format!("{package_id}::echo::echo_u64"),
        vec![],
    )
    .await
    .expect("the request should succeed with a per-call error");
    assert_first_call_rejected(&response, "found 0");

    // Too many arguments.
    let response = call(
        &mut exec_client,
        format!("{package_id}::echo::echo_u64"),
        vec![json_arg(string_value("1")), json_arg(string_value("2"))],
    )
    .await
    .expect("the request should succeed with a per-call error");
    assert_first_call_rejected(&response, "found 2");

    // Generic type mismatch: `echo<u64>` given a value that is not a `u64`.
    let item = ViewFunctionCallItem::default()
        .with_fq_function_name(format!("{package_id}::generics::echo"))
        .with_type_args(vec![type_tag("u64")])
        .with_inputs(vec![json_arg(string_value("not_a_number"))]);
    let response = exec_client
        .view_function_calls(
            ViewFunctionCallsRequest::default().with_view_function_calls(vec![item]),
        )
        .await
        .expect("the request should succeed with a per-call error")
        .into_inner();
    match response
        .call_results
        .first()
        .expect("expected one result")
        .result
        .as_ref()
        .expect("expected a result")
    {
        CallResult::Error(status) => assert_eq!(
            status.code,
            tonic::Code::InvalidArgument as i32,
            "expected InvalidArgument for the type mismatch, got: {}",
            status.message
        ),
        other => panic!("expected a per-call error for the type mismatch, got {other:?}"),
    }
}
