// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_sdk::{
    types::block::{
        address::{AliasAddress, Ed25519Address, NftAddress},
        output::{
            feature::{Irc30Metadata, IssuerFeature, MetadataFeature, SenderFeature},
            unlock_condition::{
                AddressUnlockCondition, GovernorAddressUnlockCondition,
                StateControllerAddressUnlockCondition,
            },
            AliasId, AliasOutputBuilder, Feature, NativeToken, NftId, NftOutput as StardustNft,
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

use crate::stardust::{
    migration::tests::{
        create_foundry, extract_native_token_from_bag, object_migration_with_object_owner,
        random_output_header, run_migration,
    },
    types::{
        snapshot::OutputHeader, stardust_to_sui_address, Nft, NftOutput, ALIAS_OUTPUT_MODULE_NAME,
        NFT_DYNAMIC_OBJECT_FIELD_KEY, NFT_DYNAMIC_OBJECT_FIELD_KEY_TYPE, NFT_OUTPUT_MODULE_NAME,
    },
};

fn migrate_nft(
    header: OutputHeader,
    stardust_nft: StardustNft,
) -> (ObjectID, Nft, NftOutput, Object, Object) {
    let output_id = header.output_id();
    let nft_id: NftId = stardust_nft
        .nft_id()
        .or_from_output_id(&output_id)
        .to_owned();

    let (executor, objects_map) = run_migration([(header, stardust_nft.into())]);

    // Ensure the migrated objects exist under the expected identifiers.
    let nft_object_id = ObjectID::new(*nft_id);
    let created_objects = objects_map
        .get(&output_id)
        .expect("nft output should have created objects");

    let nft_object = executor
        .store()
        .objects()
        .values()
        .find(|obj| obj.id() == nft_object_id)
        .expect("nft object should be present in the migrated snapshot");
    assert_eq!(nft_object.struct_tag().unwrap(), Nft::tag());

    let nft_output_object = executor
        .store()
        .get_object(created_objects.output().unwrap())
        .unwrap();
    assert_eq!(nft_output_object.struct_tag().unwrap(), NftOutput::tag());

    // Version is set to 1 when the nft is created based on the computed lamport
    // timestamp. When the nft is attached to the nft output, the version should
    // be incremented.
    assert!(
        nft_object.version().value() > 1,
        "nft object version should have been incremented"
    );
    assert!(
        nft_output_object.version().value() > 1,
        "nft output object version should have been incremented"
    );

    let nft_output: NftOutput =
        bcs::from_bytes(nft_output_object.data.try_as_move().unwrap().contents()).unwrap();
    let nft: Nft = bcs::from_bytes(nft_object.data.try_as_move().unwrap().contents()).unwrap();

    (
        nft_object_id,
        nft,
        nft_output,
        nft_object.clone(),
        nft_output_object.clone(),
    )
}

/// Test that the migrated nft objects in the snapshot contain the expected
/// data.
#[test]
fn nft_migration_with_full_features() {
    let nft_id = NftId::new(rand::random());
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, nft_id)
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .with_features(vec![
            Feature::Metadata(MetadataFeature::new([0xdd; 1]).unwrap()),
            Feature::Sender(SenderFeature::new(random_address)),
        ])
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new([0xaa; 1]).unwrap()),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .finish()
        .unwrap();

    let (nft_object_id, nft, nft_output, nft_object, nft_output_object) =
        migrate_nft(header, stardust_nft.clone());
    let expected_nft = Nft::try_from_stardust(nft_object_id, &stardust_nft).unwrap();

    // The bag is tested separately.
    assert_eq!(stardust_nft.amount(), nft_output.iota.value());
    // The ID is newly generated, so we don't know the exact value, but it should
    // not be zero.
    assert_ne!(nft_output.id, UID::new(ObjectID::ZERO));
    assert!(nft_output.storage_deposit_return.is_none());
    assert!(nft_output.expiration.is_none());
    assert!(nft_output.timelock.is_none());

    assert_eq!(expected_nft, nft);

    // The NFT Object should be in a dynamic object field.
    let nft_owner = derive_dynamic_field_id(
        nft_output_object.id(),
        &TypeTag::from(DynamicFieldInfo::dynamic_object_field_wrapper(
            TypeTag::from_str(NFT_DYNAMIC_OBJECT_FIELD_KEY_TYPE).unwrap(),
        )),
        &bcs::to_bytes(&NFT_DYNAMIC_OBJECT_FIELD_KEY.to_vec()).unwrap(),
    )
    .unwrap();
    assert_eq!(nft_object.owner, Owner::ObjectOwner(nft_owner.into()));

    let nft_output_owner =
        Owner::AddressOwner(stardust_to_sui_address(stardust_nft.address()).unwrap());
    assert_eq!(nft_output_object.owner, nft_output_owner);
}

/// Test that an Nft with a zeroed ID is migrated to an Nft Object with its UID
/// set to the hashed Output ID.
#[test]
fn nft_migration_with_zeroed_id() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    // If this function does not panic, then the created NFTs
    // were found at the correct non-zeroed Nft ID.
    migrate_nft(header, stardust_nft);
}

#[test]
fn nft_migration_with_alias_owner() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let alias_header = random_output_header();
    let alias = AliasOutputBuilder::new_with_amount(2_000_000, AliasId::new(rand::random()))
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    let nft_header = random_output_header();
    // alias is the owner of nft.
    let nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(AliasAddress::from(
            alias.alias_id().clone(),
        )))
        .finish()
        .unwrap();

    object_migration_with_object_owner(
        alias_header.output_id(),
        nft_header.output_id(),
        [
            (nft_header.clone(), nft.into()),
            (alias_header.clone(), alias.into()),
        ],
        ALIAS_OUTPUT_MODULE_NAME,
        NFT_OUTPUT_MODULE_NAME,
        ident_str!("unlock_alias_address_owned_nft"),
    );
}

#[test]
fn nft_migration_with_nft_owner() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let nft1_header = random_output_header();
    let nft1 = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .finish()
        .unwrap();

    let nft2_header = random_output_header();
    // nft1 is the owner of nft2.
    let nft2 = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(NftAddress::from(
            nft1.nft_id().clone(),
        )))
        .finish()
        .unwrap();

    object_migration_with_object_owner(
        nft1_header.output_id(),
        nft2_header.output_id(),
        [
            (nft1_header.clone(), nft1.into()),
            (nft2_header.clone(), nft2.into()),
        ],
        NFT_OUTPUT_MODULE_NAME,
        NFT_OUTPUT_MODULE_NAME,
        ident_str!("unlock_nft_address_owned_nft"),
    );
}

/// Test that an NFT that owns Native Tokens can extract those tokens from the
/// contained bag.
#[test]
fn nft_migration_with_native_tokens() {
    let (foundry_header, foundry_output) = create_foundry(
        0,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000)).unwrap(),
        Irc30Metadata::new("Rustcoin", "Rust", 0),
        AliasId::null(),
    );
    let native_token = NativeToken::new(foundry_output.id().into(), 100_000).unwrap();

    let nft_header = random_output_header();
    let nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(Ed25519Address::from(
            rand::random::<[u8; Ed25519Address::LENGTH]>(),
        )))
        .add_native_token(native_token)
        .finish()
        .unwrap();

    extract_native_token_from_bag(
        nft_header.output_id(),
        [
            (nft_header.clone(), nft.into()),
            (foundry_header, foundry_output.into()),
        ],
        NFT_OUTPUT_MODULE_NAME,
        native_token,
    );
}
