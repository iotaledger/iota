// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use anyhow::anyhow;
use iota_sdk::types::block::{
    address::{AliasAddress, Ed25519Address, Hrp, NftAddress, ToBech32Ext},
    output::{
        AliasId, AliasOutputBuilder, Feature, FoundryOutputBuilder, NativeToken, NftId,
        NftOutput as StardustNft, NftOutputBuilder, SimpleTokenScheme, TokenScheme,
        feature::{
            Attribute, Irc30Metadata, IssuerFeature, MetadataFeature, SenderFeature, TagFeature,
        },
        unlock_condition::{
            AddressUnlockCondition, ExpirationUnlockCondition, GovernorAddressUnlockCondition,
            ImmutableAliasAddressUnlockCondition, StateControllerAddressUnlockCondition,
            StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
        },
    },
};
use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    collection_types::VecMap,
    dynamic_field::{DynamicFieldInfo, derive_dynamic_field_id},
    id::UID,
    object::{Object, Owner},
    stardust::{
        coin_type::CoinType,
        output::{
            ALIAS_OUTPUT_MODULE_NAME, FixedPoint32, Irc27Metadata, NFT_DYNAMIC_OBJECT_FIELD_KEY,
            NFT_DYNAMIC_OBJECT_FIELD_KEY_TYPE, NFT_OUTPUT_MODULE_NAME, Nft, NftOutput,
        },
        stardust_to_iota_address,
    },
};
use move_core_types::ident_str;

use crate::stardust::{
    migration::tests::{
        ExpectedAssets, UnlockObjectTestResult, extract_native_tokens_from_bag,
        object_migration_with_object_owner, random_output_header, run_migration, unlock_object,
    },
    types::output_header::OutputHeader,
};

fn migrate_nft(
    header: OutputHeader,
    stardust_nft: StardustNft,
    coin_type: CoinType,
) -> anyhow::Result<(ObjectID, Nft, NftOutput, Object, Object)> {
    let output_id = header.output_id();
    let nft_id: NftId = stardust_nft
        .nft_id()
        .or_from_output_id(&output_id)
        .to_owned();

    let (executor, objects_map) = run_migration(
        stardust_nft.amount(),
        [(header, stardust_nft.into())],
        coin_type,
    )?;

    // Ensure the migrated objects exist under the expected identifiers.
    let nft_object_id = ObjectID::new(*nft_id);
    let created_objects = objects_map
        .get(&output_id)
        .ok_or_else(|| anyhow!("nft output should have created objects"))?;

    let nft_object = executor
        .store()
        .objects()
        .values()
        .find(|obj| obj.id() == nft_object_id)
        .ok_or_else(|| anyhow!("nft object should be present in the migrated snapshot"))?;
    assert_eq!(
        nft_object
            .struct_tag()
            .ok_or_else(|| anyhow!("missing struct tag on nft object"))?,
        Nft::tag()
    );

    let nft_output_object = executor
        .store()
        .get_object(created_objects.output()?)
        .ok_or_else(|| anyhow!("missing nft output"))?;
    assert_eq!(
        nft_output_object
            .struct_tag()
            .ok_or_else(|| anyhow!("missing struct tag on output nft object"))?,
        NftOutput::tag(coin_type.to_type_tag())
    );

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

    let nft_output: NftOutput = bcs::from_bytes(
        nft_output_object
            .data
            .try_as_move()
            .ok_or_else(|| anyhow!("nft output is not a move object"))?
            .contents(),
    )?;
    let nft: Nft = bcs::from_bytes(
        nft_object
            .data
            .try_as_move()
            .ok_or_else(|| anyhow!("nft is not a move object"))?
            .contents(),
    )?;

    Ok((
        nft_object_id,
        nft,
        nft_output,
        nft_object.clone(),
        nft_output_object.clone(),
    ))
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
            Feature::Tag(TagFeature::new(b"tag").unwrap()),
        ])
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new([0xaa; 1]).unwrap()),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .finish()
        .unwrap();

    let (nft_object_id, nft, nft_output, nft_object, nft_output_object) =
        migrate_nft(header, stardust_nft.clone(), CoinType::Iota).unwrap();
    let expected_nft = Nft::try_from_stardust(nft_object_id, &stardust_nft).unwrap();

    // The bag is tested separately.
    assert_eq!(stardust_nft.amount(), nft_output.balance.value());
    // The ID is newly generated, so we don't know the exact value, but it should
    // not be zero.
    assert_ne!(nft_output.id, UID::new(ObjectID::ZERO));
    assert_ne!(
        nft_output.id,
        UID::new(ObjectID::new(
            stardust_nft.nft_id().as_slice().try_into().unwrap()
        ))
    );

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
        &bcs::to_bytes(NFT_DYNAMIC_OBJECT_FIELD_KEY).unwrap(),
    )
    .unwrap();
    assert_eq!(nft_object.owner, Owner::ObjectOwner(nft_owner.into()));

    let nft_output_owner =
        Owner::AddressOwner(stardust_to_iota_address(stardust_nft.address()).unwrap());
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
    migrate_nft(header, stardust_nft, CoinType::Iota).unwrap();
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
            *alias.alias_id(),
        )))
        .finish()
        .unwrap();

    object_migration_with_object_owner(
        alias_header.output_id(),
        nft_header.output_id(),
        3_000_000,
        [
            (nft_header.clone(), nft.into()),
            (alias_header.clone(), alias.into()),
        ],
        ALIAS_OUTPUT_MODULE_NAME,
        NFT_OUTPUT_MODULE_NAME,
        ident_str!("unlock_alias_address_owned_nft"),
        CoinType::Iota,
    )
    .unwrap();
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
            *nft1.nft_id(),
        )))
        .finish()
        .unwrap();

    object_migration_with_object_owner(
        nft1_header.output_id(),
        nft2_header.output_id(),
        2_000_000,
        [
            (nft1_header.clone(), nft1.into()),
            (nft2_header.clone(), nft2.into()),
        ],
        NFT_OUTPUT_MODULE_NAME,
        NFT_OUTPUT_MODULE_NAME,
        ident_str!("unlock_nft_address_owned_nft"),
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that an NFT that owns Native Tokens can extract those tokens from the
/// contained bag.
#[test]
fn nft_migration_with_native_tokens() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let nft_header = random_output_header();
    let nft_output_id = nft_header.output_id();

    let mut outputs = Vec::new();
    let mut nft_builder = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(random_address));

    for i in 1..=10 {
        let foundry_header = random_output_header();
        let token_scheme = SimpleTokenScheme::new(100_000, 0, 100_000_000).unwrap();
        let irc_30_metadata = Irc30Metadata::new(format!("Rustcoin{i}"), format!("Rust{i}"), 0);
        let foundry_output =
            FoundryOutputBuilder::new_with_amount(0, i, TokenScheme::Simple(token_scheme))
                .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                    AliasAddress::new(AliasId::null()),
                ))
                .add_immutable_feature(Feature::Metadata(
                    MetadataFeature::new(irc_30_metadata).unwrap(),
                ))
                .finish()
                .unwrap();
        let native_token = NativeToken::new(foundry_output.id().into(), 100).unwrap();
        nft_builder = nft_builder.add_native_token(native_token);
        outputs.push((foundry_header, foundry_output.into()));
    }

    let nft_output = nft_builder.finish().unwrap();
    let native_tokens = nft_output.native_tokens().clone();
    outputs.push((nft_header, nft_output.into()));

    extract_native_tokens_from_bag(
        nft_output_id,
        1_000_000,
        outputs,
        NFT_OUTPUT_MODULE_NAME,
        native_tokens,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();
}

#[test]
fn nft_migration_with_valid_irc27_metadata() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let random_address2 = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let hrp = Hrp::from_str_unchecked("atoi");
    let mut attributes = BTreeSet::new();
    attributes.insert(Attribute::new("planet", "earth"));
    attributes.insert(Attribute::new("languages", vec!["english", "rust"]));

    let mut royalties = BTreeMap::new();
    royalties.insert(random_address.to_bech32(hrp), 10.0);
    royalties.insert(random_address2.to_bech32(hrp), 5.0);

    let metadata = iota_sdk::types::block::output::feature::Irc27Metadata::new(
        "image/png",
        "https://nft.org/nft.png".parse().unwrap(),
        "NFT",
    )
    .with_issuer_name("issuer_name")
    .with_collection_name("collection_name")
    .with_royalties(royalties)
    .with_description("description")
    .with_attributes(attributes);

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .with_immutable_features(vec![
            Feature::Metadata(
                MetadataFeature::new(serde_json::to_vec(&metadata).unwrap()).unwrap(),
            ),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .finish()
        .unwrap();

    let (_, nft, _, _, _) = migrate_nft(header, stardust_nft.clone(), CoinType::Iota).unwrap();

    let immutable_metadata = nft.immutable_metadata;
    assert_eq!(&immutable_metadata.media_type, metadata.media_type());
    assert_eq!(immutable_metadata.uri.url(), metadata.uri().to_string());
    assert_eq!(&immutable_metadata.name, metadata.name());
    assert_eq!(&immutable_metadata.issuer_name, metadata.issuer_name());
    assert_eq!(
        &immutable_metadata.collection_name,
        metadata.collection_name()
    );
    assert_eq!(&immutable_metadata.description, metadata.description());

    let migrated_royalties = immutable_metadata
        .royalties
        .contents
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect::<BTreeMap<_, _>>();
    let converted_royalties = metadata
        .royalties()
        .iter()
        .map(|entry| {
            (
                IotaAddress::from_bytes(entry.0.as_ed25519().as_slice()).unwrap(),
                FixedPoint32::try_from(*entry.1).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(migrated_royalties, converted_royalties);

    let migrated_attributes = immutable_metadata
        .attributes
        .contents
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect::<BTreeMap<_, _>>();

    let converted_attributes = metadata
        .attributes()
        .iter()
        .map(|entry| (entry.trait_type().to_owned(), entry.value().to_string()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(migrated_attributes, converted_attributes);
}

#[test]
fn nft_migration_with_invalid_irc27_metadata() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let metadata = iota_sdk::types::block::output::feature::Irc27Metadata::new(
        "image/png",
        "https://nft.org/nft.png".parse().unwrap(),
        "NFT",
    );

    let mut metadata = serde_json::to_value(&metadata).unwrap();
    // Make the IRC-27 Metadata invalid by changing the type of the `uri` key.
    metadata
        .as_object_mut()
        .unwrap()
        .insert("uri".to_owned(), serde_json::Value::Bool(false));
    let metadata_content = serde_json::to_vec(&metadata).unwrap();

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new(metadata_content).unwrap()),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .finish()
        .unwrap();

    let (_, nft, _, _, _) = migrate_nft(header, stardust_nft.clone(), CoinType::Iota).unwrap();

    let mut immutable_metadata = nft.immutable_metadata;
    let mut non_standard_fields = VecMap { contents: vec![] };
    std::mem::swap(
        &mut immutable_metadata.non_standard_fields,
        &mut non_standard_fields,
    );

    let non_standard_fields = non_standard_fields
        .contents
        .into_iter()
        .map(|entry| (entry.key, entry.value))
        .collect::<BTreeMap<_, _>>();

    let converted_metadata = metadata
        .as_object()
        .unwrap()
        .iter()
        .map(|entry| (entry.0.to_owned(), entry.1.to_string()))
        .collect::<BTreeMap<_, _>>();

    // Since the metadata is valid JSON, we expect the fields of the object to be in
    // the non_standard_fields.
    assert_eq!(non_standard_fields, converted_metadata);

    // Since we removed non_standard_fields, the other fields of immutable_metadata
    // should be the defaults.
    assert_eq!(immutable_metadata, Irc27Metadata::default());
}

#[test]
fn nft_migration_with_non_json_metadata() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new([0xde, 0xca, 0xde]).unwrap()),
            Feature::Issuer(IssuerFeature::new(random_address)),
        ])
        .finish()
        .unwrap();

    let (_, nft, _, _, _) = migrate_nft(header, stardust_nft.clone(), CoinType::Iota).unwrap();

    let mut immutable_metadata = nft.immutable_metadata;
    let mut non_standard_fields = VecMap { contents: vec![] };
    std::mem::swap(
        &mut immutable_metadata.non_standard_fields,
        &mut non_standard_fields,
    );

    assert_eq!(non_standard_fields.contents.len(), 1);
    let data = non_standard_fields
        .contents
        .into_iter()
        .find_map(|entry| {
            if entry.key == "data" {
                Some(entry.value)
            } else {
                None
            }
        })
        .unwrap();

    assert_eq!(data, "decade");

    // Since we removed non_standard_fields, the other fields of immutable_metadata
    // should be the defaults.
    assert_eq!(immutable_metadata, Irc27Metadata::default());
}

#[test]
fn nft_migration_without_metadata() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .with_immutable_features(vec![Feature::Issuer(IssuerFeature::new(random_address))])
        .finish()
        .unwrap();

    let (_, nft, _, _, _) = migrate_nft(header, stardust_nft.clone(), CoinType::Iota).unwrap();
    let immutable_metadata = nft.immutable_metadata;

    assert_eq!(immutable_metadata.non_standard_fields.contents.len(), 0);

    // Since we removed non_standard_fields, the other fields of immutable_metadata
    // should be the defaults.
    assert_eq!(immutable_metadata, Irc27Metadata::default());
}

#[test]
fn nft_migration_with_timelock_unlocked() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            TimelockUnlockCondition::new(epoch_start_timestamp_ms / 1000).unwrap(),
        )
        .finish()
        .unwrap();

    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_nft.into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        NFT_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();
}

#[test]
fn nft_migration_with_timelock_still_locked() {
    let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(random_address))
        .add_unlock_condition(
            TimelockUnlockCondition::new((epoch_start_timestamp_ms / 1000) + 1).unwrap(),
        )
        .finish()
        .unwrap();

    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_nft.into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        NFT_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::ERROR_TIMELOCK_NOT_EXPIRED_FAILURE,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that an NFT with an expired Expiration Unlock Condition can/cannot be
/// unlocked, depending on the TX sender.
#[test]
fn nft_migration_with_expired_unlock_condition() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let iota_owner_address = stardust_to_iota_address(owner).unwrap();
    let iota_return_address = stardust_to_iota_address(return_address).unwrap();
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    // Expiration Timestamp is exactly at the epoch start timestamp -> object is
    // expired -> return address can unlock.
    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .add_unlock_condition(
            ExpirationUnlockCondition::new(return_address, epoch_start_timestamp_ms / 1000)
                .unwrap(),
        )
        .finish()
        .unwrap();

    // Owner Address CANNOT unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header.clone(), stardust_nft.clone().into())],
        &iota_owner_address,
        NFT_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::ERROR_WRONG_SENDER_FAILURE,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();

    // Return Address CAN unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_nft.into())],
        &iota_return_address,
        NFT_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that an NFT with an unexpired Expiration Unlock Condition can/cannot be
/// unlocked, depending on the TX sender.
#[test]
fn nft_migration_with_unexpired_unlock_condition() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let iota_owner_address = stardust_to_iota_address(owner).unwrap();
    let iota_return_address = stardust_to_iota_address(return_address).unwrap();
    let header = random_output_header();

    // The epoch timestamp that the executor will use for the test.
    let epoch_start_timestamp_ms = 100_000;

    // Expiration Timestamp is after the epoch start timestamp -> object is not
    // expired -> owner address can unlock.
    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .add_unlock_condition(
            ExpirationUnlockCondition::new(return_address, (epoch_start_timestamp_ms / 1000) + 1)
                .unwrap(),
        )
        .finish()
        .unwrap();

    // Return Address CANNOT unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header.clone(), stardust_nft.clone().into())],
        &iota_return_address,
        NFT_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::ERROR_WRONG_SENDER_FAILURE,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();

    // Owner Address CAN unlock.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header, stardust_nft.into())],
        &iota_owner_address,
        NFT_OUTPUT_MODULE_NAME,
        epoch_start_timestamp_ms as u64,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();
}

/// Test that an NFT with a Storage Deposit Return Unlock Condition can be
/// unlocked.
#[test]
fn nft_migration_with_storage_deposit_return_unlock_condition() {
    let owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let return_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
    let header = random_output_header();

    let stardust_nft = NftOutputBuilder::new_with_amount(1_000_000, NftId::null())
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .add_unlock_condition(
            StorageDepositReturnUnlockCondition::new(return_address, 1_000, 1_000_000_000).unwrap(),
        )
        .finish()
        .unwrap();

    // Simply test that the unlock with the SDRUC succeeds.
    unlock_object(
        header.output_id(),
        1_000_000,
        [(header.clone(), stardust_nft.clone().into())],
        // Sender is not important for this test.
        &IotaAddress::ZERO,
        NFT_OUTPUT_MODULE_NAME,
        // Epoch start time is not important for this test.
        0,
        UnlockObjectTestResult::Success,
        ExpectedAssets::BalanceBagObject,
        CoinType::Iota,
    )
    .unwrap();
}
