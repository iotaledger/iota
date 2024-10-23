// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::MoveUtilsClient;
use iota_json_rpc_types::{
    IotaMoveAbility, IotaMoveAbilitySet, IotaMoveNormalizedField, IotaMoveNormalizedFunction,
    IotaMoveNormalizedStruct, IotaMoveNormalizedType, IotaMoveStructTypeParameter,
    IotaMoveVisibility, MoveFunctionArgType, ObjectValueKind,
};

use crate::common::{
    ApiTestSetup, CustomEq, indexer_wait_for_checkpoint, rpc_call_error_msg_matches,
};

impl<T: CustomEq> CustomEq for &[T] {
    fn eq(&self, other: &Self) -> bool {
        self.iter()
            .zip(other.iter())
            .all(|(fullnode, indexer)| fullnode.eq(indexer))
    }
}

impl CustomEq for IotaMoveNormalizedStruct {
    fn eq(&self, other: &Self) -> bool {
        let IotaMoveNormalizedStruct {
            abilities,
            fields,
            type_parameters,
        } = self;

        let IotaMoveNormalizedStruct {
            abilities: other_abilities,
            fields: other_fields,
            type_parameters: other_type_parameters,
        } = other;

        let abilities = abilities.eq(other_abilities);
        let fields = fields.as_slice().eq(&other_fields.as_slice());
        let type_parameters = type_parameters
            .as_slice()
            .eq(&other_type_parameters.as_slice());

        abilities && fields && type_parameters
    }
}

impl CustomEq for IotaMoveNormalizedFunction {
    fn eq(&self, other: &Self) -> bool {
        let IotaMoveNormalizedFunction {
            is_entry,
            parameters,
            return_,
            type_parameters,
            visibility,
        } = self;

        let IotaMoveNormalizedFunction {
            is_entry: other_is_entry,
            parameters: other_parameters,
            return_: other_return_,
            type_parameters: other_type_parameters,
            visibility: other_visibility,
        } = other;

        let entry = is_entry == other_is_entry;
        let parameters = parameters.as_slice().eq(&other_parameters.as_slice());
        let returned_types = return_.as_slice().eq(&other_return_.as_slice());
        let type_parameters = type_parameters
            .as_slice()
            .eq(&other_type_parameters.as_slice());

        entry && parameters && returned_types && type_parameters && visibility.eq(other_visibility)
    }
}

impl CustomEq for IotaMoveVisibility {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (IotaMoveVisibility::Friend, IotaMoveVisibility::Friend)
                | (IotaMoveVisibility::Private, IotaMoveVisibility::Private)
                | (IotaMoveVisibility::Public, IotaMoveVisibility::Public)
        )
    }
}

impl CustomEq for IotaMoveStructTypeParameter {
    fn eq(&self, other: &Self) -> bool {
        let IotaMoveStructTypeParameter {
            constraints,
            is_phantom,
        } = self;
        let IotaMoveStructTypeParameter {
            constraints: other_constraints,
            is_phantom: other_is_phantom,
        } = other;

        is_phantom == other_is_phantom && constraints.eq(other_constraints)
    }
}

impl CustomEq for IotaMoveAbilitySet {
    fn eq(&self, other: &Self) -> bool {
        // we do this unpacking only to ensure that if a new field will be added then
        // the impl should be changed accordingly as an error will be displayed
        let IotaMoveAbilitySet { abilities } = self;
        let IotaMoveAbilitySet {
            abilities: other_abilities,
        } = other;

        abilities.as_slice().eq(&other_abilities.as_slice())
    }
}

impl CustomEq for IotaMoveAbility {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (IotaMoveAbility::Copy, IotaMoveAbility::Copy)
                | (IotaMoveAbility::Drop, IotaMoveAbility::Drop)
                | (IotaMoveAbility::Store, IotaMoveAbility::Store)
                | (IotaMoveAbility::Key, IotaMoveAbility::Key)
        )
    }
}

impl CustomEq for IotaMoveNormalizedField {
    fn eq(&self, other: &Self) -> bool {
        let IotaMoveNormalizedField { name, type_ } = self;
        let IotaMoveNormalizedField {
            name: other_name,
            type_: other_type,
        } = other;

        name == other_name && type_.eq(other_type)
    }
}

impl CustomEq for IotaMoveNormalizedType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (IotaMoveNormalizedType::Bool, IotaMoveNormalizedType::Bool)
            | (IotaMoveNormalizedType::U8, IotaMoveNormalizedType::U8)
            | (IotaMoveNormalizedType::U16, IotaMoveNormalizedType::U16)
            | (IotaMoveNormalizedType::U32, IotaMoveNormalizedType::U32)
            | (IotaMoveNormalizedType::U64, IotaMoveNormalizedType::U64)
            | (IotaMoveNormalizedType::U128, IotaMoveNormalizedType::U128)
            | (IotaMoveNormalizedType::U256, IotaMoveNormalizedType::U256)
            | (IotaMoveNormalizedType::Address, IotaMoveNormalizedType::Address)
            | (IotaMoveNormalizedType::Signer, IotaMoveNormalizedType::Signer) => true,

            (IotaMoveNormalizedType::Vector(fullnode), IotaMoveNormalizedType::Vector(indexer)) => {
                fullnode.eq(indexer)
            }
            (
                IotaMoveNormalizedType::TypeParameter(fullnode),
                IotaMoveNormalizedType::TypeParameter(indexer),
            ) => fullnode == indexer,
            (
                IotaMoveNormalizedType::Reference(fullnode),
                IotaMoveNormalizedType::Reference(indexer),
            ) => fullnode.eq(indexer),
            (
                IotaMoveNormalizedType::MutableReference(fullnode),
                IotaMoveNormalizedType::MutableReference(indexer),
            ) => fullnode.eq(indexer),
            (
                IotaMoveNormalizedType::Struct {
                    address,
                    module,
                    name,
                    type_arguments,
                },
                IotaMoveNormalizedType::Struct {
                    address: indexer_address,
                    module: indexer_module,
                    name: indexer_name,
                    type_arguments: indexer_type_arguments,
                },
            ) => {
                address == indexer_address
                    && module == indexer_module
                    && name == indexer_name
                    && type_arguments
                        .as_slice()
                        .eq(&indexer_type_arguments.as_slice())
            }
            _ => false,
        }
    }
}

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

        assert!(matches!(indexer_function_args_type[..], [
            MoveFunctionArgType::Object(ObjectValueKind::ByMutableReference),
            MoveFunctionArgType::Pure
        ]));
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

		assert!(rpc_call_error_msg_matches(result,  r#"{"code":-32602,"message":"Package object does not exist with ID 0x0000000000000000000000000000000000000000000000000000000001823746"}"#));

        let result = client
            .get_move_function_arg_types(
                "0x1".parse().unwrap(),
                "wrong_module".to_owned(),
                "push_back".to_owned(),
            )
            .await;

		assert!(rpc_call_error_msg_matches(result,  r#"{"code":-32602,"message":"No module was found with name wrong_module"}"#));

        let result = client
            .get_move_function_arg_types(
                "0x1".parse().unwrap(),
                "vector".to_owned(),
                "wrong_function".to_owned(),
            )
            .await;

		assert!(rpc_call_error_msg_matches(result, r#"{"code":-32602,"message":"No function was found with function name wrong_function"}"#));
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

			assert!(rpc_call_error_msg_matches(result,  r#"{"code":-32602,"message":"Package object does not exist with ID 0x0000000000000000000000000000000000000000000000000000000001823746"}"#));
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

        assert!(fullnode_response.eq(&indexer_response))
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

        assert!(fullnode_response.eq(&indexer_response))
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
