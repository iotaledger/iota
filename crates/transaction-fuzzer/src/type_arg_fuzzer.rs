// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_core::test_utils::send_and_confirm_transaction;
use iota_sdk_types::{Identifier, ObjectId, StructTag, TransactionKind, TypeTag};
use iota_types::{
    base_types::IotaAddress,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaError,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ProgrammableTransaction, TransactionData, TransactionDataAPI},
    utils::to_sender_signed_transaction,
};
use proptest::{arbitrary::*, prelude::*};

use crate::{
    account_universe::AccountCurrent,
    executor::{Executor, assert_is_acceptable_result},
};

const GAS_PRICE: u64 = 1000;
const GAS: u64 = 1_000_000 * GAS_PRICE;

pub fn gen_type_tag() -> impl Strategy<Value = TypeTag> {
    prop_oneof![
        2 => any::<TypeTag>(),
        1 => gen_nested_type_tag()
    ]
}

// Generate deep nested type tags
pub fn gen_nested_type_tag() -> impl Strategy<Value = TypeTag> {
    let leaf = prop_oneof![
        Just(TypeTag::Bool),
        Just(TypeTag::U8),
        Just(TypeTag::U16),
        Just(TypeTag::U32),
        Just(TypeTag::U64),
        Just(TypeTag::U128),
        Just(TypeTag::U256),
        Just(TypeTag::Address),
        Just(TypeTag::Signer),
    ];
    leaf.prop_recursive(8, 6, 10, |inner| {
        prop_oneof![
            inner.prop_map(|x| TypeTag::Vector(Box::new(x))),
            gen_struct_tag().prop_map(|x| TypeTag::Struct(Box::new(x))),
        ]
    })
}

pub fn gen_struct_tag() -> impl Strategy<Value = StructTag> {
    (
        any::<IotaAddress>(),
        any::<Identifier>(),
        any::<Identifier>(),
        any::<Vec<TypeTag>>(),
    )
        .prop_map(|(address, module, name, type_params)| {
            StructTag::new(address, module, name, type_params)
        })
}

pub fn generate_valid_type_factory_tags(
    type_factory_addr: ObjectId,
) -> impl Strategy<Value = TypeTag> {
    let leaf = prop_oneof![
        base_type_factory_tag_gen(type_factory_addr),
        nested_type_factory_tag_gen(type_factory_addr),
    ];

    leaf.prop_recursive(8, 6, 10, move |inner| {
        prop_oneof![inner.prop_map(|x| TypeTag::Vector(Box::new(x))),]
    })
}

pub fn generate_valid_and_invalid_type_factory_tags(
    type_factory_addr: ObjectId,
) -> impl Strategy<Value = TypeTag> {
    let leaf = prop_oneof![
        any::<TypeTag>(),
        base_type_factory_tag_gen(type_factory_addr),
        nested_type_factory_tag_gen(type_factory_addr),
    ];

    leaf.prop_recursive(8, 6, 10, move |inner| {
        prop_oneof![inner.prop_map(|x| TypeTag::Vector(Box::new(x))),]
    })
}

pub fn base_type_factory_tag_gen(addr: ObjectId) -> impl Strategy<Value = TypeTag> {
    "[A-Z]".prop_map(move |name| {
        TypeTag::Struct(Box::new(StructTag::new(
            addr,
            Identifier::from_static("type_factory"),
            Identifier::new(name).unwrap(),
            vec![],
        )))
    })
}

pub fn nested_type_factory_tag_gen(addr: ObjectId) -> impl Strategy<Value = TypeTag> {
    base_type_factory_tag_gen(addr).prop_recursive(20, 256, 10, move |inner| {
        (inner, "[A-Z]").prop_map(move |(instantiation, name)| {
            TypeTag::Struct(Box::new(StructTag::new(
                addr,
                Identifier::from_static("type_factory"),
                Identifier::new(name.to_string() + &name).unwrap(),
                vec![instantiation],
            )))
        })
    })
}

pub fn type_factory_pt_for_tags(
    package_id: ObjectId,
    type_tags: Vec<TypeTag>,
    len: usize,
) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    builder
        .move_call(
            package_id,
            Identifier::from_static("type_factory"),
            Identifier::new(format!("type_tags{len}")).unwrap(),
            type_tags,
            vec![],
        )
        .unwrap();
    builder.finish()
}

pub fn pt_for_tags(type_tags: Vec<TypeTag>) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    builder
        .move_call(
            ObjectId::FRAMEWORK,
            Identifier::from_static("random_type_tag_fuzzing"),
            Identifier::from_static("random_type_tag_fuzzing_fn"),
            type_tags,
            vec![],
        )
        .unwrap();
    builder.finish()
}

pub fn run_pt(account: &mut AccountCurrent, exec: &mut Executor, pt: ProgrammableTransaction) {
    let result = run_pt_effects(account, exec, pt);
    let status = result.map(|effects| effects.status().clone());
    assert_is_acceptable_result(&status);
}

pub fn run_pt_effects(
    account: &mut AccountCurrent,
    exec: &mut Executor,
    pt: ProgrammableTransaction,
) -> Result<TransactionEffects, IotaError> {
    let gas_object = account.new_gas_object(exec);
    let gas_object_ref = gas_object.object_ref();
    let kind = TransactionKind::Programmable(pt);
    let tx_data = TransactionData::new(
        kind,
        account.initial_data.account.address,
        gas_object_ref,
        GAS,
        GAS_PRICE,
    );
    let signed_txn = to_sender_signed_transaction(tx_data, &account.initial_data.account.key);
    exec.rt
        .block_on(send_and_confirm_transaction(&exec.state, None, signed_txn))
        .map(|(_, effects)| effects.into_data())
}
