// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::stardust::migration::migration::PACKAGE_DEPS;
use crate::stardust::migration::tests::extract_native_token_from_bag;
use crate::stardust::migration::tests::run_migration;
use crate::stardust::migration::tests::{create_foundry, random_output_header};
use crate::stardust::types::stardust_to_sui_address;
use crate::stardust::types::ALIAS_DYNAMIC_OBJECT_FIELD_KEY;
use crate::stardust::types::ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE;
use crate::stardust::types::ALIAS_OUTPUT_MODULE_NAME;
use crate::stardust::types::{snapshot::OutputHeader, Alias, AliasOutput};

use iota_sdk::types::block::address::Address;
use iota_sdk::types::block::output::feature::Irc30Metadata;
use iota_sdk::types::block::output::{NativeToken, SimpleTokenScheme};
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
use sui_types::base_types::SuiAddress;
use sui_types::dynamic_field::derive_dynamic_field_id;
use sui_types::dynamic_field::DynamicFieldInfo;
use sui_types::id::UID;
use sui_types::object::Object;
use sui_types::object::Owner;
use sui_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_types::transaction::{Argument, CheckedInputObjects, ObjectArg};
use sui_types::TypeTag;
use sui_types::{base_types::ObjectID, STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID};

fn migrate_alias(
    header: OutputHeader,
    stardust_alias: StardustAlias,
) -> (ObjectID, Alias, AliasOutput, Object, Object) {
    let output_id = header.output_id();
    let alias_id: AliasId = stardust_alias
        .alias_id()
        .or_from_output_id(&output_id)
        .to_owned();

    let (executor, objects_map) = run_migration([(header, stardust_alias.into())]);

    // Ensure the migrated objects exist under the expected identifiers.
    let alias_object_id = ObjectID::new(*alias_id);
    let created_objects = objects_map
        .get(&output_id)
        .expect("alias output should have created objects");

    let alias_object = executor
        .store()
        .objects()
        .values()
        .find(|obj| obj.id() == alias_object_id)
        .expect("alias object should be present in the migrated snapshot");
    assert_eq!(alias_object.struct_tag().unwrap(), Alias::tag());

    let alias_output_object = executor
        .store()
        .get_object(created_objects.output().unwrap())
        .unwrap();
    assert_eq!(
        alias_output_object.struct_tag().unwrap(),
        AliasOutput::tag()
    );

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

    (
        alias_object_id,
        alias,
        alias_output,
        alias_object.clone(),
        alias_output_object.clone(),
    )
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

    let (alias_object_id, alias, alias_output, alias_object, alias_output_object) =
        migrate_alias(header, stardust_alias.clone());
    let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

    // The bag is tested separately.
    assert_eq!(stardust_alias.amount(), alias_output.iota.value());
    // The ID is newly generated, so we don't know the exact value, but it should not be zero.
    assert_ne!(alias_output.id, UID::new(ObjectID::ZERO));

    assert_eq!(expected_alias, alias);

    // The Alias Object should be in a dynamic object field.
    let alias_owner = derive_dynamic_field_id(
        alias_output_object.id(),
        &TypeTag::from(DynamicFieldInfo::dynamic_object_field_wrapper(
            // The key type of the dynamic object field.
            TypeTag::from_str(ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE).unwrap(),
        )),
        &bcs::to_bytes(&ALIAS_DYNAMIC_OBJECT_FIELD_KEY.to_vec()).unwrap(),
    )
    .unwrap();
    assert_eq!(alias_object.owner, Owner::ObjectOwner(alias_owner.into()));

    let alias_output_owner =
        Owner::AddressOwner(stardust_to_sui_address(stardust_alias.governor_address()).unwrap());
    assert_eq!(alias_output_object.owner, alias_output_owner);
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
fn alias_migration_with_alias_owner() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let alias1_header = random_output_header();
    let alias1_amount = 1_000_000;
    let stardust_alias1 =
        AliasOutputBuilder::new_with_amount(alias1_amount, AliasId::new(rand::random()))
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
            .finish()
            .unwrap();

    let alias2_header = random_output_header();
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

    let (mut executor, objects_map) = run_migration([
        (alias1_header.clone(), stardust_alias1.into()),
        (alias2_header.clone(), stardust_alias2.into()),
    ]);

    // Find the corresponding objects to the migrated aliases.
    let alias1_created_objects = objects_map
        .get(&alias1_header.output_id())
        .expect("alias output should have created objects");
    let alias2_created_objects = objects_map
        .get(&alias2_header.output_id())
        .expect("alias output should have created objects");

    let alias_output1_object_ref = executor
        .store()
        .get_object(alias1_created_objects.output().unwrap())
        .unwrap()
        .compute_object_reference();
    let alias_output2_object_ref = executor
        .store()
        .get_object(alias2_created_objects.output().unwrap())
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

        // Destroying the bag only works if it's empty, hence asserting that it is in fact empty.
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        // Transfer the coin to the zero address since we have to move it somewhere.
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
            vec![TypeTag::from_str(&format!("{}::sui::SUI", SUI_FRAMEWORK_PACKAGE_ID)).unwrap()],
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

        // Transfer the coin to the zero address since we have to move it somewhere.
        builder.transfer_arg(SuiAddress::default(), coin_arg);

        // We have successfully extracted the aliases which is what we want to test.
        // Transfer to the zero address so the PTB doesn't fail.
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
    let native_token = NativeToken::new(foundry_output.id().into(), 100).unwrap();

    let alias_header = random_output_header();
    let alias = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::new(rand::random()))
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
        .add_native_token(native_token)
        .finish()
        .unwrap();

    extract_native_token_from_bag(
        alias_header.output_id(),
        [
            (alias_header.clone(), alias.into()),
            (foundry_header, foundry_output.into()),
        ],
        ALIAS_OUTPUT_MODULE_NAME,
        native_token,
    );
}
