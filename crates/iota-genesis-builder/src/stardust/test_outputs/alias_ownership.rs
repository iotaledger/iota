// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::types::block::{
    address::{AliasAddress, Ed25519Address},
    output::{
        feature::{Irc27Metadata, IssuerFeature, MetadataFeature},
        unlock_condition::{AddressUnlockCondition, ImmutableAliasAddressUnlockCondition},
        AliasId, AliasOutputBuilder, BasicOutputBuilder, Feature, FoundryOutputBuilder, NftId,
        NftOutputBuilder, Output, SimpleTokenScheme, UnlockCondition,
    },
};

use crate::stardust::{test_outputs::random_output_header, types::output_header::OutputHeader};

pub(crate) fn outputs() -> Vec<(OutputHeader, Output)> {
    let alias_output_header = random_output_header();
    let alias_owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

    let alias_output = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(alias_owner))
        .finish()
        .unwrap();
    let alias_address = alias_output.alias_address(&alias_output_header.output_id());

    let mut outputs = Vec::new();

    outputs.push((alias_output_header, Output::from(alias_output)));
    outputs.push(owns_random_basic_output(alias_address));
    outputs.push(owns_random_nft_output(alias_address));
    outputs.push(owns_random_alias_output(alias_address));
    outputs.push(owns_random_foundry_output(alias_address));
    outputs
}

fn owns_random_basic_output(owner: AliasAddress) -> (OutputHeader, Output) {
    let basic_output_header = random_output_header();

    let basic_output = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .finish()
        .unwrap();

    (basic_output_header, Output::from(basic_output))
}

fn owns_random_nft_output(owner: AliasAddress) -> (OutputHeader, Output) {
    let nft_output_header = random_output_header();
    let nft_metadata = Irc27Metadata::new(
        "image/png",
        "https://nft.org/nft.png".parse().unwrap(),
        "NFT",
    )
    .with_issuer_name("issuer_name")
    .with_collection_name("collection_name")
    .with_description("description");

    let nft_output = NftOutputBuilder::new_with_amount(1_000_000, NftId::new(rand::random()))
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .with_immutable_features(vec![
            Feature::Metadata(
                MetadataFeature::new(serde_json::to_vec(&nft_metadata).unwrap()).unwrap(),
            ),
            Feature::Issuer(IssuerFeature::new(owner)),
        ])
        .finish()
        .unwrap();

    (nft_output_header, Output::from(nft_output))
}

fn owns_random_alias_output(owner: AliasAddress) -> (OutputHeader, Output) {
    let alias_output_header = random_output_header();

    let alias_output = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::null())
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .finish()
        .unwrap();

    (alias_output_header, Output::from(alias_output))
}

fn owns_random_foundry_output(owner: AliasAddress) -> (OutputHeader, Output) {
    let foundry_output_header = random_output_header();

    let supply = 1_000_000;
    let token_scheme = SimpleTokenScheme::new(supply, 0, supply).unwrap();
    let foundry_output = FoundryOutputBuilder::new_with_amount(1_000_000, 1, token_scheme.into())
        .with_unlock_conditions([UnlockCondition::from(
            ImmutableAliasAddressUnlockCondition::new(owner),
        )])
        .finish_with_params(supply)
        .unwrap();

    (foundry_output_header, Output::from(foundry_output))
}
