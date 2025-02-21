// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::MoveUtilsClient;
use iota_json_rpc_types::{MoveFunctionArgType, ObjectValueKind};

use crate::common::{ApiTestSetup, indexer_wait_for_checkpoint, rpc_call_error_msg_matches};

#[test]
fn get_move_function_arg_types_empty() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let indexer_function_args_type = client
            .get_move_function_arg_types(
                "0x1".parse().unwrap(),
                "address".to_owned(),
                "length".to_owned(),
            )
            .await
            .unwrap();

        assert!(
            indexer_function_args_type.is_empty(),
            "Should not have any function args"
        )
    });
}

#[test]
fn get_move_function_arg_types() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let indexer_function_args_type = client
            .get_move_function_arg_types(
                "0x1".parse().unwrap(),
                "vector".to_owned(),
                "push_back".to_owned(),
            )
            .await
            .unwrap();

        assert!(matches!(
            indexer_function_args_type.as_slice(),
            [
                MoveFunctionArgType::Object(ObjectValueKind::ByMutableReference),
                MoveFunctionArgType::Pure
            ]
        ));
    });
}

#[test]
fn get_move_function_arg_types_not_found() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let result = client
            .get_move_function_arg_types(
                "0x1823746".parse().unwrap(),
                "vector".to_owned(),
                "push_back".to_owned(),
            )
            .await;

        assert!(matches!(result, Err(err) if err.to_string().contains("Package not found in DB: 0000000000000000000000000000000000000000000000000000000001823746")));

        let result = client
            .get_move_function_arg_types(
                "0x1".parse().unwrap(),
                "wrong_module".to_owned(),
                "push_back".to_owned(),
            )
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32602,"message":"No module was found with name wrong_module"}"#
        ));

        let result = client
            .get_move_function_arg_types(
                "0x1".parse().unwrap(),
                "vector".to_owned(),
                "wrong_function".to_owned(),
            )
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32602,"message":"No function was found with function name wrong_function"}"#
        ));
    });
}

#[test]
fn get_normalized_move_modules_by_package() {
    let ApiTestSetup {
        cluster,
        runtime,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let package_id = "0x1".parse().unwrap();

        let fullnode_response = cluster
            .rpc_client()
            .get_normalized_move_modules_by_package(package_id)
            .await
            .unwrap();

        let indexer_response = client
            .get_normalized_move_modules_by_package(package_id)
            .await
            .unwrap();

        assert_eq!(fullnode_response, indexer_response);
    });
}

#[test]
fn get_normalized_move_modules_by_package_not_found() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let result = client
            .get_normalized_move_modules_by_package("0x1823746".parse().unwrap())
            .await;

        assert!(matches!(result, Err(err) if err.to_string().contains("Package not found in DB: 0000000000000000000000000000000000000000000000000000000001823746")));
    });
}

#[test]
fn get_normalized_move_module() {
    let ApiTestSetup {
        cluster,
        runtime,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let package_id = "0x1".parse().unwrap();
        let module = "vector".to_owned();

        let fullnode_response = cluster
            .rpc_client()
            .get_normalized_move_module(package_id, module.clone())
            .await
            .unwrap();

        let indexer_response = client
            .get_normalized_move_module(package_id, module)
            .await
            .unwrap();

        assert_eq!(fullnode_response, indexer_response);
    });
}

#[test]
fn get_normalized_move_module_not_found() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let result = client
            .get_normalized_move_module("0x1".parse().unwrap(), "wrong_module".to_owned())
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32602,"message":"No module was found with name wrong_module"}"#
        ));
    });
}

#[test]
fn get_normalized_move_struct() {
    let ApiTestSetup {
        cluster,
        runtime,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let package_id = "0x2".parse().unwrap();
        let module = "vec_set".to_owned();
        let struct_name = "VecSet".to_owned();

        let fullnode_response = cluster
            .rpc_client()
            .get_normalized_move_struct(package_id, module.clone(), struct_name.clone())
            .await
            .unwrap();

        let indexer_response = client
            .get_normalized_move_struct(package_id, module, struct_name)
            .await
            .unwrap();

        assert_eq!(fullnode_response, indexer_response)
    });
}

#[test]
fn get_normalized_move_struct_not_found() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let result = client
            .get_normalized_move_struct(
                "0x2".parse().unwrap(),
                "vec_set".to_owned(),
                "WrongStruct".to_owned(),
            )
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32602,"message":"No struct was found with struct name WrongStruct"}"#
        ));
    });
}

#[test]
fn get_normalized_move_function() {
    let ApiTestSetup {
        cluster,
        runtime,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let package_id = "0x2".parse().unwrap();
        let module = "vec_set".to_owned();
        let function_name = "insert".to_owned();

        let fullnode_response = cluster
            .rpc_client()
            .get_normalized_move_function(package_id, module.clone(), function_name.clone())
            .await
            .unwrap();

        let indexer_response = client
            .get_normalized_move_function(package_id, module, function_name)
            .await
            .unwrap();

        assert_eq!(fullnode_response, indexer_response)
    });
}

#[test]
fn get_normalized_move_function_not_found() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let result = client
            .get_normalized_move_function(
                "0x2".parse().unwrap(),
                "vec_set".to_owned(),
                "wrong_function".to_owned(),
            )
            .await;

        assert!(rpc_call_error_msg_matches(
            result,
            r#"{"code":-32602,"message":"No function was found with function name wrong_function"}"#
        ));
    });
}
