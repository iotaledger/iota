// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_sdk::types::block::address::AliasAddress;
use iota_sdk::types::block::output::feature::Irc30Metadata;
use iota_sdk::types::block::output::feature::MetadataFeature;
use iota_sdk::types::block::output::unlock_condition::ImmutableAliasAddressUnlockCondition;
use iota_sdk::types::block::output::AliasId;
use iota_sdk::types::block::output::Feature;
use iota_sdk::types::block::output::FoundryOutput;
use iota_sdk::types::block::output::FoundryOutputBuilder;
use iota_sdk::types::block::output::Output;
use iota_sdk::types::block::output::OutputId;
use iota_sdk::types::block::output::SimpleTokenScheme;
use iota_sdk::types::block::output::TokenScheme;
use move_core_types::identifier::IdentStr;

use crate::stardust::migration::executor::Executor;
use crate::stardust::migration::migration::Migration;
use crate::stardust::migration::verification::created_objects::CreatedObjects;
use crate::stardust::types::snapshot::OutputHeader;

use crate::stardust::migration::migration::NATIVE_TOKEN_BAG_KEY_TYPE;
use crate::stardust::migration::migration::PACKAGE_DEPS;
use iota_sdk::types::block::output::{NativeToken, TokenId};
use move_core_types::ident_str;
use sui_types::balance::Balance;
use sui_types::base_types::SuiAddress;
use sui_types::coin::Coin;
use sui_types::gas_coin::GAS;
use sui_types::inner_temporary_store::InnerTemporaryStore;
use sui_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_types::transaction::{Argument, CheckedInputObjects, ObjectArg};
use sui_types::TypeTag;
use sui_types::{STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID};

mod alias;
mod executor;

fn random_output_header() -> OutputHeader {
    OutputHeader::new_testing(
        rand::random(),
        rand::random(),
        rand::random(),
        rand::random(),
    )
}

fn run_migration(
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
) -> (Executor, HashMap<OutputId, CreatedObjects>) {
    let mut migration = Migration::new(1).unwrap();
    migration.run_migration(outputs).unwrap();
    migration.into_parts()
}

fn create_foundry(
    iota_amount: u64,
    token_scheme: SimpleTokenScheme,
    irc_30_metadata: Irc30Metadata,
    alias_id: AliasId,
) -> (OutputHeader, FoundryOutput) {
    let builder =
        FoundryOutputBuilder::new_with_amount(iota_amount, 1, TokenScheme::Simple(token_scheme))
            .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                AliasAddress::new(alias_id),
            ))
            .add_feature(Feature::Metadata(
                MetadataFeature::new(irc_30_metadata).unwrap(),
            ));
    let foundry_output = builder.finish().unwrap();

    (random_output_header(), foundry_output)
}

/// Test that an Output that owns Native Tokens can extract those tokens from the contained bag.
fn extract_native_token_from_bag(
    output_id: OutputId,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    module_name: &IdentStr,
    native_token: NativeToken,
) {
    let native_token_id: &TokenId = native_token.token_id();

    let (mut executor, objects_map) = run_migration(outputs);

    // Find the corresponding objects to the migrated object.
    let output_created_objects = objects_map
        .get(&output_id)
        .expect("output should have created objects");

    let output_object_ref = executor
        .store()
        .get_object(output_created_objects.output().unwrap())
        .unwrap()
        .compute_object_reference();

    // Recreate the key under which the tokens are stored in the bag.
    let foundry_ledger_data = executor
        .native_tokens()
        .get(&native_token_id.clone().into())
        .unwrap();
    let token_type = format!(
        "{}::{}::{}",
        foundry_ledger_data.coin_type_origin.package,
        foundry_ledger_data.coin_type_origin.module_name,
        foundry_ledger_data.coin_type_origin.struct_name
    );
    let token_type_tag = token_type.parse::<TypeTag>().unwrap();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let inner_object_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(output_object_ref))
            .unwrap();

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            module_name.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![inner_object_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        // This is the inner object, i.e. the Alias extracted from an Alias Output
        // or NFT extracted from an NFT Output.
        let inner_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![GAS::type_tag()],
            vec![balance_arg],
        );

        builder.transfer_arg(SuiAddress::default(), coin_arg);
        builder.transfer_arg(SuiAddress::default(), inner_arg);

        let token_type_arg = builder.pure(token_type.clone()).unwrap();
        let balance_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("remove").into(),
            vec![
                NATIVE_TOKEN_BAG_KEY_TYPE
                    .parse()
                    .expect("should be a valid type tag"),
                Balance::type_(token_type_tag.clone()).into(),
            ],
            vec![bag_arg, token_type_arg],
        );

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![token_type_tag.clone()],
            vec![balance_arg],
        );

        // Destroying the bag only works if it's empty, hence asserting that it is in fact empty.
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        builder.transfer_arg(SuiAddress::default(), coin_arg);

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([output_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    let InnerTemporaryStore { written, .. } =
        executor.execute_pt_unmetered(input_objects, pt).unwrap();

    let coin_token_struct_tag = Coin::type_(token_type_tag);
    let coin_token = written
        .values()
        .find(|obj| {
            obj.struct_tag()
                .map(|tag| tag == coin_token_struct_tag)
                .unwrap_or(false)
        })
        .map(|obj| obj.as_coin_maybe())
        .flatten()
        .expect("coin token object should exist");

    assert_eq!(coin_token.balance.value(), native_token.amount().as_u64());
}
