// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::stardust::migration::migration::NATIVE_TOKEN_BAG_KEY_TYPE;
use crate::stardust::migration::migration::PACKAGE_DEPS;
use crate::stardust::migration::tests::run_migration;
use crate::stardust::migration::tests::{create_foundry, random_output_header};
use crate::stardust::types::ALIAS_OUTPUT_MODULE_NAME;
use crate::stardust::types::{snapshot::OutputHeader, Alias, AliasOutput};
use iota_sdk::types::block::address::Address;
use iota_sdk::types::block::output::feature::Irc30Metadata;
use iota_sdk::types::block::output::{NativeToken, SimpleTokenScheme, TokenId};
use iota_sdk::types::block::{
    address::Ed25519Address,
    output::{
        feature::{IssuerFeature, MetadataFeature, SenderFeature},
        unlock_condition::{GovernorAddressUnlockCondition, StateControllerAddressUnlockCondition},
        AliasId, AliasOutput as StardustAlias, AliasOutputBuilder, Feature,
    },
};
use iota_sdk::U256;
use move_core_types::ident_str;
use move_core_types::language_storage::StructTag;
use std::str::FromStr;
use sui_types::balance::Balance;
use sui_types::base_types::SuiAddress;
use sui_types::coin::Coin;
use sui_types::gas_coin::GAS;
use sui_types::inner_temporary_store::InnerTemporaryStore;
use sui_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_types::transaction::{Argument, CheckedInputObjects, ObjectArg};
use sui_types::TypeTag;
use sui_types::{base_types::ObjectID, STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID};

fn migrate_alias(
    header: OutputHeader,
    stardust_alias: StardustAlias,
) -> (ObjectID, Alias, AliasOutput) {
    let alias_id: AliasId = stardust_alias
        .alias_id()
        .or_from_output_id(&header.output_id())
        .to_owned();

    let migrated_objects = run_migration([(header, stardust_alias.into())]).into_objects();

    // Ensure the migrated objects exist under the expected identifiers.
    let alias_object_id = ObjectID::new(*alias_id);
    let alias_object = migrated_objects
        .iter()
        .find(|obj| obj.id() == alias_object_id)
        .expect("alias object should be present in the migrated snapshot");
    assert_eq!(alias_object.struct_tag().unwrap(), Alias::tag(),);
    let alias_output_object = migrated_objects
        .iter()
        .find(|obj| match obj.struct_tag() {
            Some(tag) => tag == AliasOutput::tag(),
            None => false,
        })
        .expect("alias object should be present in the migrated snapshot");

    // Version is set to 1 when the alias is created based on the computed lamport timestamp.
    // When the alias is attached to the alias output, the version should be incremented.
    assert!(
        alias_object.version().value() > 1,
        "alias object version should have been incremented"
    );
    assert!(
        alias_output_object.version().value() > 1,
        "alias output object version should have been incremented"
    );

    let alias_output: AliasOutput =
        bcs::from_bytes(alias_output_object.data.try_as_move().unwrap().contents()).unwrap();
    let alias: Alias =
        bcs::from_bytes(alias_object.data.try_as_move().unwrap().contents()).unwrap();

    (alias_object_id, alias, alias_output)
}

/// Test that the migrated alias objects in the snapshot contain the expected data.
#[test]
fn alias_migration_with_full_features() {
    let alias_id = AliasId::new(rand::random());
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, alias_id)
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
        .with_state_metadata([0xff; 1])
        .with_features(vec![
            Feature::Metadata(MetadataFeature::new([0xdd; 1]).unwrap()),
            Feature::Sender(SenderFeature::new(random_address)),
        ])
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new([0xaa; 1]).unwrap()),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .with_state_index(3)
        .finish()
        .unwrap();

    let (alias_object_id, alias, alias_output) = migrate_alias(header, stardust_alias.clone());
    let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

    // Compare only the balance. The ID is newly generated and the bag is tested separately.
    assert_eq!(stardust_alias.amount(), alias_output.iota.value());

    assert_eq!(expected_alias, alias);
}

/// Test that an Alias with a zeroed ID is migrated to an Alias Object with its UID set to the hashed Output ID.
#[test]
fn alias_migration_with_zeroed_id() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::null())
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    // If this function does not panic, then the created aliases
    // were found at the correct non-zeroed Alias ID.
    migrate_alias(header, stardust_alias);
}

/// Test that an Alias owned by another Alias can be received by the owning object.
///
/// The PTB sends the extracted assets to the null address since it must be used in the transaction.
#[test]
fn test_alias_migration_with_alias_owner() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let alias1_amount = 1_000_000;
    let stardust_alias1 =
        AliasOutputBuilder::new_with_amount(alias1_amount, AliasId::new(rand::random()))
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
            .finish()
            .unwrap();

    let alias2_amount = 2_000_000;
    // stardust_alias1 is the owner of stardust_alias2.
    let stardust_alias2 =
        AliasOutputBuilder::new_with_amount(alias2_amount, AliasId::new(rand::random()))
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(Address::from(
                stardust_alias1.alias_id().clone(),
            )))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(Address::from(
                stardust_alias1.alias_id().clone(),
            )))
            .finish()
            .unwrap();

    let mut executor = run_migration([
        (random_output_header(), stardust_alias1.into()),
        (random_output_header(), stardust_alias2.into()),
    ]);

    // Find the corresponding objects to the migrated aliases, uniquely identified by their amounts.
    // Should be adapted to use the tags from issue 239 to make this much easier.
    let alias_output1_id = executor
        .store()
        .objects()
        .iter()
        .find(|(_, obj)| {
            obj.struct_tag()
                .map(|tag| tag == AliasOutput::tag())
                .unwrap_or(false)
                && bcs::from_bytes::<AliasOutput>(obj.data.try_as_move().unwrap().contents())
                    .unwrap()
                    .iota
                    .value()
                    == alias1_amount
        })
        .expect("alias1 should exist")
        .1
        .id();

    let alias_output2_id = executor
        .store()
        .objects()
        .iter()
        .find(|(_, obj)| {
            obj.struct_tag()
                .map(|tag| tag == AliasOutput::tag())
                .unwrap_or(false)
                && bcs::from_bytes::<AliasOutput>(obj.data.try_as_move().unwrap().contents())
                    .unwrap()
                    .iota
                    .value()
                    == alias2_amount
        })
        .expect("alias2 should exist")
        .1
        .id();

    let alias_output1_object_ref = executor
        .store()
        .get_object(&alias_output1_id)
        .unwrap()
        .compute_object_reference();

    let alias_output2_object_ref = executor
        .store()
        .get_object(&alias_output2_id)
        .unwrap()
        .compute_object_reference();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let alias1_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(alias_output1_object_ref))
            .unwrap();

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ALIAS_OUTPUT_MODULE_NAME.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![alias1_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let alias1_arg = Argument::NestedResult(result_idx, 2);

        let receiving_alias2_arg = builder
            .obj(ObjectArg::Receiving(alias_output2_object_ref))
            .unwrap();
        let received_alias_output2 = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ident_str!("address_unlock_condition").into(),
            ident_str!("unlock_alias_address_owned_alias").into(),
            vec![],
            vec![alias1_arg, receiving_alias2_arg],
        );

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![
                StructTag::from_str(&format!("{}::sui::SUI", SUI_FRAMEWORK_PACKAGE_ID))
                    .unwrap()
                    .into(),
            ],
            vec![balance_arg],
        );

        builder.transfer_arg(SuiAddress::default(), bag_arg);
        builder.transfer_arg(SuiAddress::default(), coin_arg);

        // We have to use Alias Output as we cannot transfer it (since it lacks the `store` ability),
        // so we extract its assets.
        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ALIAS_OUTPUT_MODULE_NAME.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![received_alias_output2],
        );
        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let alias2_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![
                StructTag::from_str(&format!("{}::sui::SUI", SUI_FRAMEWORK_PACKAGE_ID))
                    .unwrap()
                    .into(),
            ],
            vec![balance_arg],
        );

        builder.transfer_arg(SuiAddress::default(), coin_arg);
        builder.transfer_arg(SuiAddress::default(), bag_arg);

        builder.transfer_arg(SuiAddress::default(), alias1_arg);
        builder.transfer_arg(SuiAddress::default(), alias2_arg);

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([alias_output1_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    executor.execute_pt_unmetered(input_objects, pt).unwrap();
}

/// Test that an Alias that owns Native Tokens can extract those tokens from the contained bag.
#[test]
fn alias_migration_with_native_tokens() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let (foundry_header, foundry_output) = create_foundry(
        0,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000_000))
            .unwrap(),
        Irc30Metadata::new("Rustcoin", "Rust", 0),
        AliasId::null(),
    );
    let native_token_id: TokenId = foundry_output.id().into();

    let alias1_amount = 1_000_000;
    let native_token_amount = 100;
    let stardust_alias1 =
        AliasOutputBuilder::new_with_amount(alias1_amount, AliasId::new(rand::random()))
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
            .add_native_token(NativeToken::new(native_token_id, native_token_amount).unwrap())
            .finish()
            .unwrap();

    let mut executor = run_migration([
        (random_output_header(), stardust_alias1.into()),
        (foundry_header, foundry_output.into()),
    ]);

    // Find the corresponding objects to the migrated aliases, uniquely identified by their amounts.
    // Should be adapted to use the tags from issue 239 to make this much easier.
    let alias_output1_id = executor
        .store()
        .objects()
        .values()
        .find(|obj| {
            obj.struct_tag()
                .map(|tag| tag == AliasOutput::tag())
                .unwrap_or(false)
        })
        .expect("alias1 should exist")
        .id();

    let alias_output1_object_ref = executor
        .store()
        .get_object(&alias_output1_id)
        .unwrap()
        .compute_object_reference();

    // Recreate the key under which the tokens are stored in the bag.
    let foundry_ledger_data = executor
        .native_tokens()
        .get(&native_token_id.into())
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
        let alias1_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(alias_output1_object_ref))
            .unwrap();

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ALIAS_OUTPUT_MODULE_NAME.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![alias1_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let alias1_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![GAS::type_tag()],
            vec![balance_arg],
        );

        builder.transfer_arg(SuiAddress::default(), coin_arg);
        builder.transfer_arg(SuiAddress::default(), alias1_arg);

        let token_type_arg = builder.pure(token_type.clone()).unwrap();
        let balance_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("remove").into(),
            vec![
                NATIVE_TOKEN_BAG_KEY_TYPE
                    .parse()
                    .expect("should be a valid struct tag"),
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
            .load_input_objects([alias_output1_object_ref])
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

    assert_eq!(coin_token.balance.value(), native_token_amount);
}
