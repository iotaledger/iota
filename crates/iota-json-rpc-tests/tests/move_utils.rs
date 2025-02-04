// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_json_rpc_api::MoveUtilsClient;
use iota_json_rpc_types::{
    IotaMoveAbility, IotaMoveNormalizedType, IotaMoveVisibility, MoveFunctionArgType,
    ObjectValueKind,
};
use iota_macros::sim_test;
use iota_types::{IOTA_FRAMEWORK_ADDRESS, base_types::ObjectID};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn get_normalized_move_modules_by_package() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let move_modules = http_client
        .get_normalized_move_modules_by_package(ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()))
        .await?;

    assert_eq!(
        move_modules.keys().cloned().collect::<HashSet<String>>(),
        [
            "address",
            "authenticator_state",
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
            "iota",
            "kiosk",
            "kiosk_extension",
            "labeler",
            "linked_table",
            "object",
            "object_bag",
            "object_table",
            "package",
            "pay",
            "poseidon",
            "priority_queue",
            "prover",
            "random",
            "system_admin_cap",
            "table",
            "table_vec",
            "timelock",
            "token",
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

    Ok(())
}

#[sim_test]
async fn get_normalized_move_modules_by_package_wrong_package() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let wrong_package_address = ObjectID::ZERO;

    let response = http_client
        .get_normalized_move_modules_by_package(wrong_package_address)
        .await;

    assert!(response.is_err_and(|e| {
        e.to_string()
            .contains("Package object does not exist with ID")
    }));

    Ok(())
}

#[sim_test]
async fn get_normalized_move_module() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";

    let move_module = http_client
        .get_normalized_move_module(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
        )
        .await?;

    assert_eq!(move_module.file_format_version, 6);
    assert_eq!(move_module.address, IOTA_FRAMEWORK_ADDRESS.to_hex_literal());
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

    Ok(())
}

#[sim_test]
async fn get_normalized_move_module_wrong_module() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let wrong_module_name = "foobar";

    let response = http_client
        .get_normalized_move_module(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            wrong_module_name.into(),
        )
        .await;

    assert!(response.is_err_and(|e| e.to_string().contains("No module found with module name")));

    Ok(())
}

#[sim_test]
async fn get_normalized_move_struct() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";

    let move_struct = http_client
        .get_normalized_move_struct(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
            "Coin".into(),
        )
        .await?;

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
    if let IotaMoveNormalizedType::Struct {
        address,
        module,
        name,
        type_arguments,
    } = &id_field.type_
    {
        assert_eq!(*address, IOTA_FRAMEWORK_ADDRESS.to_hex_literal());
        assert_eq!(module, "object");
        assert_eq!(name, "UID");
        assert_eq!(type_arguments.len(), 0);
    };

    assert_eq!(balance_field.name, "balance");
    assert!(matches!(
        balance_field.type_,
        IotaMoveNormalizedType::Struct { .. }
    ));
    if let IotaMoveNormalizedType::Struct {
        address,
        module,
        name,
        type_arguments,
    } = &balance_field.type_
    {
        assert_eq!(*address, IOTA_FRAMEWORK_ADDRESS.to_hex_literal());
        assert_eq!(module, "balance");
        assert_eq!(name, "Balance");
        assert_eq!(type_arguments.len(), 1);
        assert!(matches!(
            type_arguments[0],
            IotaMoveNormalizedType::TypeParameter(0)
        ));
    };

    Ok(())
}

#[sim_test]
async fn get_normalized_move_struct_wrong_struct_name() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";
    let wrong_struct_name = "foobar";

    let response = http_client
        .get_normalized_move_struct(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
            wrong_struct_name.into(),
        )
        .await;

    assert!(response.is_err_and(|e| {
        e.to_string()
            .contains("No struct was found with struct name")
    }));

    Ok(())
}

#[sim_test]
async fn get_normalized_move_function() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";

    let move_function = http_client
        .get_normalized_move_function(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
            "split".into(),
        )
        .await?;

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
        if let IotaMoveNormalizedType::Struct {
            address,
            module,
            name,
            type_arguments,
        } = unboxed_type
        {
            assert_eq!(*address, IOTA_FRAMEWORK_ADDRESS.to_hex_literal());
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
        if let IotaMoveNormalizedType::Struct {
            address,
            module,
            name,
            type_arguments,
        } = unboxed_type
        {
            assert_eq!(*address, IOTA_FRAMEWORK_ADDRESS.to_hex_literal());
            assert_eq!(module, "tx_context");
            assert_eq!(name, "TxContext");
            assert_eq!(type_arguments.len(), 0);
        };
    };

    let return_types = move_function.return_;
    assert_eq!(return_types.len(), 1);
    let return_type = &return_types[0];
    assert!(matches!(return_type, IotaMoveNormalizedType::Struct { .. }));
    if let IotaMoveNormalizedType::Struct {
        address,
        module,
        name,
        type_arguments,
    } = return_type
    {
        assert_eq!(*address, IOTA_FRAMEWORK_ADDRESS.to_hex_literal());
        assert_eq!(module, "coin");
        assert_eq!(name, "Coin");
        assert_eq!(type_arguments.len(), 1);
        assert!(matches!(
            type_arguments[0],
            IotaMoveNormalizedType::TypeParameter(0)
        ));
    };

    Ok(())
}

#[sim_test]
async fn get_normalized_move_function_wrong_function_name() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";
    let wrong_function_name = "foobar";

    let response = http_client
        .get_normalized_move_function(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
            wrong_function_name.into(),
        )
        .await;

    assert!(response.is_err_and(|e| {
        e.to_string()
            .contains("No function was found with function name")
    }));

    Ok(())
}

#[sim_test]
async fn get_move_function_arg_types() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";

    let arg_types = http_client
        .get_move_function_arg_types(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
            "split".into(),
        )
        .await?;

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

    Ok(())
}

#[sim_test]
async fn get_move_function_arg_types_wrong_function_name() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let module_name = "coin";
    let wrong_function_name = "foobar";

    let response = http_client
        .get_move_function_arg_types(
            ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
            module_name.into(),
            wrong_function_name.into(),
        )
        .await;

    assert!(response.is_err_and(|e| e.to_string().contains("No parameters found for function")));

    Ok(())
}
