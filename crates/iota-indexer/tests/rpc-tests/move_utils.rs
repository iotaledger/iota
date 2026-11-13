// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_json_rpc_api::MoveUtilsClient;
use iota_json_rpc_types::{
    IotaMoveAbility, IotaMoveNormalizedStructType, IotaMoveNormalizedType, IotaMoveVisibility,
    MoveFunctionArgType, ObjectValueKind,
};
use iota_sdk_types::{Address, ObjectId};

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
            "should not have any function args"
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

        assert!(matches!(result, Err(err) if err.to_string().contains("Package not found: 0x0000000000000000000000000000000000000000000000000000000001823746")));

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

        assert!(matches!(result, Err(err) if err.to_string().contains("Package not found: 0x0000000000000000000000000000000000000000000000000000000001823746")));
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

#[test]
fn get_normalized_move_modules_by_package_of_framework() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let move_modules = client
            .get_normalized_move_modules_by_package(ObjectId::FRAMEWORK)
            .await
            .unwrap();

        assert_eq!(
            move_modules.keys().cloned().collect::<HashSet<String>>(),
            [
                "account",
                "address",
                "authenticator_function",
                "authenticator_state",
                "auth_context",
                "bag",
                "balance",
                "bcs",
                "bls12381",
                "borrow",
                "clock",
                "coin",
                "coin_manager",
                "config",
                "deny_list",
                "derived_object",
                "display",
                "dynamic_field",
                "dynamic_object_field",
                "ecdsa_k1",
                "ecdsa_r1",
                "ecvrf",
                "ed25519",
                "event",
                "groth16",
                "group_ops",
                "hash",
                "hex",
                "hmac",
                "intent",
                "iota",
                "kiosk",
                "kiosk_extension",
                "labeler",
                "linked_table",
                "object",
                "object_bag",
                "object_table",
                "module_metadata",
                "package",
                "pay",
                "poseidon",
                "protocol_config",
                "priority_queue",
                "package_metadata",
                "ptb",
                "ptb_call_arg",
                "ptb_command",
                "prover",
                "random",
                "system_admin_cap",
                "table",
                "table_vec",
                "timelock",
                "token",
                "transaction_deny_rules",
                "transfer",
                "transfer_policy",
                "tx_context",
                "types",
                "url",
                "vdf",
                "vec_map",
                "vec_set",
                "versioned",
                "zklogin_verified_id",
                "zklogin_verified_issuer",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<String>>(),
        );
    });
}

#[test]
fn get_normalized_move_module_of_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;
        let module_name = "coin";

        let move_module = client
            .get_normalized_move_module(ObjectId::FRAMEWORK, module_name.into())
            .await
            .unwrap();

        assert_eq!(move_module.file_format_version, 6);
        assert_eq!(move_module.address, Address::FRAMEWORK.to_short_hex());
        assert_eq!(move_module.name, module_name);
        assert_eq!(move_module.friends.len(), 0);
        assert_eq!(
            move_module
                .structs
                .keys()
                .cloned()
                .collect::<HashSet<String>>(),
            [
                "Coin",
                "CoinMetadata",
                "DenyCapV1",
                "RegulatedCoinMetadata",
                "TreasuryCap",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<String>>(),
        );
        assert_eq!(
            move_module
                .exposed_functions
                .keys()
                .cloned()
                .collect::<HashSet<String>>(),
            [
                "balance",
                "balance_mut",
                "burn",
                "create_currency",
                "create_regulated_currency_v1",
                "deny_list_v1_add",
                "deny_list_v1_contains_current_epoch",
                "deny_list_v1_contains_next_epoch",
                "deny_list_v1_disable_global_pause",
                "deny_list_v1_enable_global_pause",
                "deny_list_v1_is_global_pause_enabled_current_epoch",
                "deny_list_v1_is_global_pause_enabled_next_epoch",
                "deny_list_v1_remove",
                "destroy_zero",
                "divide_into_n",
                "from_balance",
                "get_decimals",
                "get_description",
                "get_icon_url",
                "get_name",
                "get_symbol",
                "into_balance",
                "join",
                "mint",
                "mint_and_transfer",
                "mint_balance",
                "put",
                "split",
                "supply_immut",
                "supply_mut",
                "take",
                "total_supply",
                "treasury_into_supply",
                "update_description",
                "update_icon_url",
                "update_name",
                "update_symbol",
                "value",
                "zero",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<String>>(),
        );
    });
}

#[test]
fn get_normalized_move_struct_of_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;
        let module_name = "coin";

        let move_struct = client
            .get_normalized_move_struct(ObjectId::FRAMEWORK, module_name.into(), "Coin".into())
            .await
            .unwrap();

        assert_eq!(move_struct.abilities.abilities.len(), 2);
        assert!(matches!(
            move_struct.abilities.abilities[..],
            [IotaMoveAbility::Store, IotaMoveAbility::Key]
        ));

        assert_eq!(move_struct.type_parameters.len(), 1);
        let type_parameter = &move_struct.type_parameters[0];
        assert_eq!(type_parameter.constraints.abilities.len(), 0);
        assert!(type_parameter.is_phantom);

        assert_eq!(move_struct.fields.len(), 2);
        let id_field = &move_struct.fields[0];
        let balance_field = &move_struct.fields[1];

        assert_eq!(id_field.name, "id");
        assert!(matches!(
            id_field.type_,
            IotaMoveNormalizedType::Struct { .. }
        ));
        if let IotaMoveNormalizedType::Struct { inner } = &id_field.type_ {
            let IotaMoveNormalizedStructType {
                address,
                module,
                name,
                type_arguments,
            } = &**inner;

            assert_eq!(*address, Address::FRAMEWORK.to_short_hex());
            assert_eq!(module, "object");
            assert_eq!(name, "UID");
            assert_eq!(type_arguments.len(), 0);
        };

        assert_eq!(balance_field.name, "balance");
        assert!(matches!(
            balance_field.type_,
            IotaMoveNormalizedType::Struct { .. }
        ));
        if let IotaMoveNormalizedType::Struct { inner } = &balance_field.type_ {
            let IotaMoveNormalizedStructType {
                address,
                module,
                name,
                type_arguments,
            } = &**inner;
            assert_eq!(*address, Address::FRAMEWORK.to_short_hex());
            assert_eq!(module, "balance");
            assert_eq!(name, "Balance");
            assert_eq!(type_arguments.len(), 1);
            assert!(matches!(
                type_arguments[0],
                IotaMoveNormalizedType::TypeParameter(0)
            ));
        };
    });
}

#[test]
fn get_normalized_move_function_of_coin_split() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;
        let module_name = "coin";

        let move_function = client
            .get_normalized_move_function(ObjectId::FRAMEWORK, module_name.into(), "split".into())
            .await
            .unwrap();

        assert!(matches!(
            move_function.visibility,
            IotaMoveVisibility::Public
        ));
        assert!(!move_function.is_entry);

        let type_parameter = &move_function.type_parameters[0];
        assert_eq!(type_parameter.abilities.len(), 0);

        assert_eq!(move_function.parameters.len(), 3);
        let coin_parameter = &move_function.parameters[0];
        let amount_parameter = &move_function.parameters[1];
        let context_parameter = &move_function.parameters[2];

        assert!(matches!(
            coin_parameter,
            IotaMoveNormalizedType::MutableReference(..)
        ));
        if let IotaMoveNormalizedType::MutableReference(boxed_type) = &coin_parameter {
            let unboxed_type = boxed_type.as_ref();
            assert!(matches!(
                unboxed_type,
                IotaMoveNormalizedType::Struct { .. }
            ));
            if let IotaMoveNormalizedType::Struct { inner } = unboxed_type {
                let IotaMoveNormalizedStructType {
                    address,
                    module,
                    name,
                    type_arguments,
                } = &**inner;
                assert_eq!(*address, Address::FRAMEWORK.to_short_hex());
                assert_eq!(module, "coin");
                assert_eq!(name, "Coin");
                assert_eq!(type_arguments.len(), 1);
                assert!(matches!(
                    type_arguments[0],
                    IotaMoveNormalizedType::TypeParameter(0)
                ));
            };
        };

        assert!(matches!(amount_parameter, IotaMoveNormalizedType::U64));

        assert!(matches!(
            context_parameter,
            IotaMoveNormalizedType::MutableReference(..)
        ));
        if let IotaMoveNormalizedType::MutableReference(boxed_type) = &context_parameter {
            let unboxed_type = boxed_type.as_ref();
            assert!(matches!(
                unboxed_type,
                IotaMoveNormalizedType::Struct { .. }
            ));
            if let IotaMoveNormalizedType::Struct { inner } = unboxed_type {
                let IotaMoveNormalizedStructType {
                    address,
                    module,
                    name,
                    type_arguments,
                } = &**inner;
                assert_eq!(*address, Address::FRAMEWORK.to_short_hex());
                assert_eq!(module, "tx_context");
                assert_eq!(name, "TxContext");
                assert_eq!(type_arguments.len(), 0);
            };
        };

        let return_types = move_function.return_;
        assert_eq!(return_types.len(), 1);
        let return_type = &return_types[0];
        assert!(matches!(return_type, IotaMoveNormalizedType::Struct { .. }));
        if let IotaMoveNormalizedType::Struct { inner } = return_type {
            let IotaMoveNormalizedStructType {
                address,
                module,
                name,
                type_arguments,
            } = &**inner;
            assert_eq!(*address, Address::FRAMEWORK.to_short_hex());
            assert_eq!(module, "coin");
            assert_eq!(name, "Coin");
            assert_eq!(type_arguments.len(), 1);
            assert!(matches!(
                type_arguments[0],
                IotaMoveNormalizedType::TypeParameter(0)
            ));
        };
    });
}

#[test]
fn get_move_function_arg_types_of_coin_split() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;
        let module_name = "coin";

        let arg_types = client
            .get_move_function_arg_types(ObjectId::FRAMEWORK, module_name.into(), "split".into())
            .await
            .unwrap();

        assert_eq!(arg_types.len(), 3);
        let coin_parameter_type = &arg_types[0];
        let amount_parameter_type = &arg_types[1];
        let context_parameter_type = &arg_types[2];

        assert!(matches!(
            coin_parameter_type,
            MoveFunctionArgType::Object(ObjectValueKind::ByMutableReference)
        ));
        assert!(matches!(amount_parameter_type, MoveFunctionArgType::Pure));
        assert!(matches!(
            context_parameter_type,
            MoveFunctionArgType::Object(ObjectValueKind::ByMutableReference)
        ));
    });
}
