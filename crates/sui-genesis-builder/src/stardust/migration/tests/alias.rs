// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk::{
    types::block::{
        address::{Address, Ed25519Address},
        output::{
            feature::{Irc30Metadata, IssuerFeature, MetadataFeature, SenderFeature},
            unlock_condition::{
                AddressUnlockCondition, GovernorAddressUnlockCondition,
                StateControllerAddressUnlockCondition,
            },
            AliasId, AliasOutput as StardustAlias, AliasOutputBuilder, Feature, NativeToken, NftId,
            NftOutputBuilder, SimpleTokenScheme,
        },
    },
    U256,
};
use move_core_types::ident_str;
use sui_types::{
    base_types::ObjectID,
    dynamic_field::{derive_dynamic_field_id, DynamicFieldInfo},
    id::UID,
    object::{Object, Owner},
    TypeTag,
};

use super::ExpectedAssets;
use crate::stardust::{
    migration::tests::{
        create_foundry, extract_native_token_from_bag, object_migration_with_object_owner,
        random_output_header, run_migration,
    },
    types::{
        snapshot::OutputHeader, stardust_to_sui_address, Alias, AliasOutput,
        ALIAS_DYNAMIC_OBJECT_FIELD_KEY, ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE,
        ALIAS_OUTPUT_MODULE_NAME, NFT_OUTPUT_MODULE_NAME,
    },
};

fn migrate_alias(
    header: OutputHeader,
    stardust_alias: StardustAlias,
) -> anyhow::Result<(ObjectID, Alias, AliasOutput, Object, Object)> {
    let output_id = header.output_id();
    let alias_id: AliasId = stardust_alias
        .alias_id()
        .or_from_output_id(&output_id)
        .to_owned();

    let (executor, objects_map) = run_migration([(header, stardust_alias.into())])?;

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

    // Version is set to 1 when the alias is created based on the computed lamport
    // timestamp. When the alias is attached to the alias output, the version
    // should be incremented.
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

    Ok((
        alias_object_id,
        alias,
        alias_output,
        alias_object.clone(),
        alias_output_object.clone(),
    ))
}

/// Test that the migrated alias objects in the snapshot contain the expected
/// data.
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
        migrate_alias(header, stardust_alias.clone()).unwrap();
    let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

    // The bag is tested separately.
    assert_eq!(stardust_alias.amount(), alias_output.iota.value());
    // The ID is newly generated, so we don't know the exact value, but it should
    // not be zero.
    assert_ne!(alias_output.id, UID::new(ObjectID::ZERO));
    assert_ne!(
        alias_output.id,
        UID::new(ObjectID::new(
            stardust_alias.alias_id().as_slice().try_into().unwrap()
        ))
    );

    assert_eq!(expected_alias, alias);

    // The Alias Object should be in a dynamic object field.
    let alias_owner = derive_dynamic_field_id(
        alias_output_object.id(),
        &TypeTag::from(DynamicFieldInfo::dynamic_object_field_wrapper(
            // The key type of the dynamic object field.
            TypeTag::from_str(ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE).unwrap(),
        )),
        &bcs::to_bytes(ALIAS_DYNAMIC_OBJECT_FIELD_KEY).unwrap(),
    )
    .unwrap();
    assert_eq!(alias_object.owner, Owner::ObjectOwner(alias_owner.into()));

    let alias_output_owner =
        Owner::AddressOwner(stardust_to_sui_address(stardust_alias.governor_address()).unwrap());
    assert_eq!(alias_output_object.owner, alias_output_owner);
}

/// Test that an Alias with a zeroed ID is migrated to an Alias Object with its
/// UID set to the hashed Output ID.
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
    migrate_alias(header, stardust_alias).unwrap();
}

/// Test that an Alias owned by another Alias can be received by the owning
/// object.
///
/// The PTB sends the extracted assets to the null address since they must be
/// used in the transaction.
#[test]
fn alias_migration_with_alias_owner() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let alias1_header = random_output_header();
    let stardust_alias1 =
        AliasOutputBuilder::new_with_amount(1_000_000, AliasId::new(rand::random()))
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
            .finish()
            .unwrap();

    let alias2_header = random_output_header();
    // stardust_alias1 is the owner of stardust_alias2.
    let stardust_alias2 =
        AliasOutputBuilder::new_with_amount(2_000_000, AliasId::new(rand::random()))
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(Address::from(
                *stardust_alias1.alias_id(),
            )))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(Address::from(
                *stardust_alias1.alias_id(),
            )))
            .finish()
            .unwrap();

    object_migration_with_object_owner(
        alias1_header.output_id(),
        alias2_header.output_id(),
        [
            (alias1_header.clone(), stardust_alias1.into()),
            (alias2_header.clone(), stardust_alias2.into()),
        ],
        ALIAS_OUTPUT_MODULE_NAME,
        ALIAS_OUTPUT_MODULE_NAME,
        ident_str!("unlock_alias_address_owned_alias"),
    )
    .unwrap();
}

/// Test that an Alias owned by an NFT can be received by the owning object.
#[test]
fn alias_migration_with_nft_owner() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let nft_header = random_output_header();
    let nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    let alias_header = random_output_header();
    // nft is the owner (governor) of alias.
    let alias = AliasOutputBuilder::new_with_amount(2_000_000, AliasId::new(rand::random()))
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(
            Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>()),
        ))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(Address::from(
            *nft.nft_id(),
        )))
        .finish()
        .unwrap();

    object_migration_with_object_owner(
        nft_header.output_id(),
        alias_header.output_id(),
        [
            (nft_header.clone(), nft.into()),
            (alias_header.clone(), alias.into()),
        ],
        NFT_OUTPUT_MODULE_NAME,
        ALIAS_OUTPUT_MODULE_NAME,
        ident_str!("unlock_nft_address_owned_alias"),
    )
    .unwrap();
}

/// Test that an Alias that owns Native Tokens can extract those tokens from the
/// contained bag.
#[test]
fn alias_migration_with_native_tokens() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let (foundry_header, foundry_output) = create_foundry(
        0,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000_000))
            .unwrap(),
        Irc30Metadata::new("Rustcoin\u{245}", "Rust''\n\tCöin", 0)
            .with_description("The description of Rustcöin.\n Nice!"),
        AliasId::null(),
    )
    .unwrap();
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
        ExpectedAssets::BalanceBagObject,
    )
    .unwrap();
}
