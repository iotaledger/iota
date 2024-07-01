// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk::types::block::{
    address::{Address, AliasAddress, Ed25519Address},
    output::{
        feature::{Irc27Metadata, IssuerFeature, MetadataFeature},
        unlock_condition::{
            AddressUnlockCondition, GovernorAddressUnlockCondition,
            ImmutableAliasAddressUnlockCondition,
        },
        AliasId, AliasOutput, AliasOutputBuilder, BasicOutput, BasicOutputBuilder, Feature,
        FoundryOutput, FoundryOutputBuilder, NftId, NftOutput, NftOutputBuilder, Output,
        SimpleTokenScheme, UnlockCondition,
    },
};
use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::stardust::{test_outputs::random_output_header, types::output_header::OutputHeader};

pub(crate) fn outputs() -> Vec<(OutputHeader, Output)> {
    let mut outputs = Vec::new();

    // create a randomized ownership dependency tree
    let randomness_seed = rand::random();
    let mut rng = StdRng::seed_from_u64(randomness_seed);
    println!("alias ownership randomness seed: {randomness_seed}");

    // create 10 different alias outputs with each owning various other assets
    for _ in 0..10 {
        let alias_output_header = random_output_header();
        let alias_owner = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
        let alias_output =
            AliasOutputBuilder::new_with_amount(1_000_000, AliasId::new(rand::random()))
                .add_unlock_condition(GovernorAddressUnlockCondition::new(alias_owner))
                .finish()
                .unwrap();
        let alias_address = alias_output.alias_address(&alias_output_header.output_id());

        // let this alias own various other assets, that may themselves own other assets
        let max_depth = rng.gen_range(1usize..5);
        let mut owning_addresses: VecDeque<(usize, Address)> =
            vec![(0, alias_address.into())].into();

        while let Some((depth, owning_addr)) = owning_addresses.pop_front() {
            if depth > max_depth {
                continue;
            }
            // create a random number of random assets
            for _ in 0..rng.gen_range(1usize..=5) {
                match rng.gen_range(0u8..=3) {
                    0 /* alias */ => {
                        let (output_header, alias) = random_alias_output(owning_addr);
                        owning_addresses
                            .push_back((depth + 1, alias.alias_address(&output_header.output_id()).into()));
                        outputs.push((output_header, alias.into()));
                    }
                    1 /* nft */ => {
                        let (output_header, nft) = random_nft_output(owning_addr);
                        owning_addresses
                            .push_back((depth + 1, nft.nft_address(&output_header.output_id()).into()));
                        outputs.push((output_header, nft.into()));
                    }
                    2 /* basic */ => {
                        let (output_header, basic) = random_basic_output(owning_addr);
                        outputs.push((output_header, basic.into()));
                    }
                    3 /* foundry */=> {
                        if let Address::Alias(child) = owning_addr {
                            let (output_header, foundry) = random_foundry_output(child);
                            outputs.push((output_header, foundry.into()));
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    outputs
}

fn random_basic_output(owner: impl Into<Address>) -> (OutputHeader, BasicOutput) {
    let basic_output_header = random_output_header();

    let basic_output = BasicOutputBuilder::new_with_amount(1_000_000)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .finish()
        .unwrap();

    (basic_output_header, basic_output)
}

fn random_nft_output(owner: impl Into<Address>) -> (OutputHeader, NftOutput) {
    let owner = owner.into();
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
        .add_unlock_condition(AddressUnlockCondition::new(owner.clone()))
        .with_immutable_features(vec![
            Feature::Metadata(
                MetadataFeature::new(serde_json::to_vec(&nft_metadata).unwrap()).unwrap(),
            ),
            Feature::Issuer(IssuerFeature::new(owner)),
        ])
        .finish()
        .unwrap();

    (nft_output_header, nft_output)
}

fn random_alias_output(owner: impl Into<Address>) -> (OutputHeader, AliasOutput) {
    let alias_output_header = random_output_header();

    let alias_output = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::null())
        .add_unlock_condition(GovernorAddressUnlockCondition::new(owner))
        .finish()
        .unwrap();

    (alias_output_header, alias_output)
}

fn random_foundry_output(owner: impl Into<AliasAddress>) -> (OutputHeader, FoundryOutput) {
    let foundry_output_header = random_output_header();

    let supply = 1_000_000;
    let token_scheme = SimpleTokenScheme::new(supply, 0, supply).unwrap();
    let foundry_output = FoundryOutputBuilder::new_with_amount(1_000_000, 1, token_scheme.into())
        .with_unlock_conditions([UnlockCondition::from(
            ImmutableAliasAddressUnlockCondition::new(owner),
        )])
        .finish_with_params(supply)
        .unwrap();

    (foundry_output_header, foundry_output)
}
