// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk_types::{
    Command, ExecutionError, ExecutionStatus, ObjectId, TypeArgumentError, TypeTag,
};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::TEST_ONLY_GAS_UNIT_FOR_GENERIC,
};

use crate::authority::{
    authority_tests::{
        TestCallArg, call_move, execute_programmable_transaction, init_state_with_ids,
    },
    move_integration_tests::build_and_publish_test_package,
};

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_same_module_type_param() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params",
        // with_unpublished_deps
        true,
    )
    .await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m1",
        "create_and_transfer",
        vec![],
        vec![
            TestCallArg::Pure(bcs::to_bytes(&(16_u64)).unwrap()),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    let created_object_id = effects.created()[0].reference().object_id;
    let type_param =
        TypeTag::from_str(format!("{}::m1::Object", package.object_id).as_str()).unwrap();

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m1",
        "transfer_object",
        vec![type_param],
        vec![
            TestCallArg::Object(created_object_id),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_different_module_type_param() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params",
        // with_unpublished_deps
        true,
    )
    .await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m2",
        "create_and_transfer",
        vec![],
        vec![
            TestCallArg::Pure(bcs::to_bytes(&(16_u64)).unwrap()),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    let created_object_id = effects.created()[0].reference().object_id;
    let type_param =
        TypeTag::from_str(format!("{}::m2::AnotherObject", package.object_id).as_str()).unwrap();

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        // a different module than the one where the type was defined
        "m1",
        "transfer_object",
        vec![type_param],
        vec![
            TestCallArg::Object(created_object_id),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_nested_type_param() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params",
        // with_unpublished_deps
        true,
    )
    .await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m1",
        "create_and_transfer_gen",
        vec![],
        vec![
            TestCallArg::Pure(bcs::to_bytes(&(16_u64)).unwrap()),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    let created_object_id = effects.created()[0].reference().object_id;
    let type_param = TypeTag::from_str(
        format!(
            "{}::m1::GenObject<{}::m2::AnotherObject>",
            package.object_id, package.object_id
        )
        .as_str(),
    )
    .unwrap();

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m1",
        "transfer_object",
        // outer type comes from the same module but nested one from a different module
        vec![type_param],
        vec![
            TestCallArg::Object(created_object_id),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_nested_type_param_different_module() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params",
        // with_unpublished_deps
        true,
    )
    .await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m1",
        "create_and_transfer_gen",
        vec![],
        vec![
            TestCallArg::Pure(bcs::to_bytes(&(16_u64)).unwrap()),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    let created_object_id = effects.created()[0].reference().object_id;
    let type_param = TypeTag::from_str(
        format!(
            "{}::m1::GenObject<{}::m2::AnotherObject>",
            package.object_id, package.object_id
        )
        .as_str(),
    )
    .unwrap();

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        // a different module than those where types where defined
        "m3",
        "transfer_object",
        vec![type_param],
        vec![
            TestCallArg::Object(created_object_id),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_different_package_type_param() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params",
        // with_unpublished_deps
        true,
    )
    .await;

    let package_extra = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params_extra",
        // with_unpublished_deps
        true,
    )
    .await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m2",
        "create_and_transfer",
        vec![],
        vec![
            TestCallArg::Pure(bcs::to_bytes(&(16_u64)).unwrap()),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    let created_object_id = effects.created()[0].reference().object_id;
    let type_param =
        TypeTag::from_str(format!("{}::m2::AnotherObject", package.object_id).as_str()).unwrap();

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        // a different package than the one where the type was defined
        &package_extra.object_id,
        "m1",
        "transfer_object",
        vec![type_param],
        vec![
            TestCallArg::Object(created_object_id),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_nested_type_param_different_package() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params",
        // with_unpublished_deps
        true,
    )
    .await;

    let package_extra = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "type_params_extra",
        // with_unpublished_deps
        true,
    )
    .await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "m1",
        "create_and_transfer_gen",
        vec![],
        vec![
            TestCallArg::Pure(bcs::to_bytes(&(16_u64)).unwrap()),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    let created_object_id = effects.created()[0].reference().object_id;
    let type_param = TypeTag::from_str(
        format!(
            "{}::m1::GenObject<{}::m2::AnotherObject>",
            package.object_id, package.object_id
        )
        .as_str(),
    )
    .unwrap();

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        // a different package than those where types where defined
        &package_extra.object_id,
        "m1",
        "transfer_object",
        vec![type_param],
        vec![
            TestCallArg::Object(created_object_id),
            TestCallArg::Pure(bcs::to_bytes(&sender).unwrap()),
        ],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_self_type_name_in_move_call_type_argument() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &ObjectId::FRAMEWORK,
        "coin",
        "zero",
        vec![self_type_tag()],
        vec![],
    )
    .await
    .unwrap();

    assert_eq!(effects.status(), &type_not_found_failure());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_self_type_name_in_make_move_vec_type() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let mut builder = ProgrammableTransactionBuilder::new();
    builder.command(Command::new_make_move_vector(Some(self_type_tag()), vec![]));

    let effects = execute_programmable_transaction(
        &authority,
        &gas,
        &sender,
        &sender_key,
        builder.finish(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC,
    )
    .await
    .unwrap();

    assert_eq!(effects.status(), &type_not_found_failure());
}

/// `0x2::iota::<SELF>`: the SDK accepts `<SELF>` as an identifier, move-core
/// rejects it.
fn self_type_tag() -> TypeTag {
    TypeTag::from_str("0x2::iota::<SELF>").unwrap()
}

/// The status of a transaction whose first command fails to load its first
/// type argument.
fn type_not_found_failure() -> ExecutionStatus {
    ExecutionStatus::Failure {
        error: ExecutionError::TypeArgumentError {
            type_argument: 0,
            kind: TypeArgumentError::TypeNotFound,
        },
        command: Some(0),
    }
}
