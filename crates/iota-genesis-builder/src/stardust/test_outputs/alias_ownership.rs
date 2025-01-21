// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk::{
    client::secret::{SecretManage, mnemonic::MnemonicSecretManager},
    types::block::{
        address::{Address, AliasAddress},
        output::{
            AliasId, AliasOutput, AliasOutputBuilder, BasicOutput, BasicOutputBuilder, Feature,
            FoundryOutput, FoundryOutputBuilder, NftId, NftOutput, NftOutputBuilder,
            OUTPUT_INDEX_RANGE, Output, SimpleTokenScheme, UnlockCondition,
            feature::{Irc27Metadata, IssuerFeature, MetadataFeature},
            unlock_condition::{
                AddressUnlockCondition, GovernorAddressUnlockCondition,
                ImmutableAliasAddressUnlockCondition, StateControllerAddressUnlockCondition,
            },
        },
    },
};
use rand::{Rng, rngs::StdRng};

use crate::stardust::{
    test_outputs::{MERGE_MILESTONE_INDEX, MERGE_TIMESTAMP_SECS},
    types::{output_header::OutputHeader, output_index::OutputIndex},
};

const MNEMONIC: &str = "few hood high omit camp keep burger give happy iron evolve draft few dawn pulp jazz box dash load snake gown bag draft car";
const OWNING_ALIAS_COUNT: u32 = 10;

pub(crate) async fn outputs(
    rng: &mut StdRng,
    coin_type: u32,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();
    let secret_manager = MnemonicSecretManager::try_from_mnemonic(MNEMONIC)?;

    let alias_owners = secret_manager
        .generate_ed25519_addresses(coin_type, 0, 0..OWNING_ALIAS_COUNT, None)
        .await?;

    // create 10 different alias outputs with each owning various other assets
    for alias_owner in alias_owners {
        let alias_output_header = random_output_header(rng);

        let alias_output = AliasOutputBuilder::new_with_amount(
            1_000_000,
            (&alias_output_header.output_id()).into(),
        )
        .add_unlock_condition(GovernorAddressUnlockCondition::new(alias_owner))
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(alias_owner))
        .finish()?;
        let alias_address = AliasAddress::new(*alias_output.alias_id());
        outputs.push((alias_output_header, alias_output.into()));

        // let this alias own various other assets, that may themselves own other assets
        let max_depth = rng.gen_range(1usize..5);
        let mut owning_addresses: VecDeque<(usize, Address)> =
            vec![(0, alias_address.into())].into();

        while let Some((depth, owning_addr)) = owning_addresses.pop_front() {
            if depth > max_depth {
                continue;
            }
            let mut serial_number = 1;
            // create a random number of random assets
            for _ in 0usize..rng.gen_range(1..=5) {
                match rng.gen_range(0..=3) {
                    0 => {
                        // alias
                        let (output_header, alias) = random_alias_output(rng, owning_addr)?;
                        owning_addresses
                            .push_back((depth + 1, AliasAddress::new(*alias.alias_id()).into()));
                        outputs.push((output_header, alias.into()));
                    }
                    1 => {
                        // nft
                        let (output_header, nft) = random_nft_output(rng, owning_addr)?;
                        owning_addresses.push_back((
                            depth + 1,
                            nft.nft_address(&output_header.output_id()).into(),
                        ));
                        outputs.push((output_header, nft.into()));
                    }
                    2 => {
                        // basic
                        let (output_header, basic) = random_basic_output(rng, owning_addr)?;
                        outputs.push((output_header, basic.into()));
                    }
                    3 => {
                        // foundry
                        if let Address::Alias(owning_addr) = owning_addr {
                            let (output_header, foundry) =
                                random_foundry_output(rng, &mut serial_number, owning_addr)?;
                            outputs.push((output_header, foundry.into()));
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    Ok(outputs)
}

fn random_basic_output(
    rng: &mut StdRng,
    owner: impl Into<Address>,
) -> anyhow::Result<(OutputHeader, BasicOutput)> {
    let basic_output_header = random_output_header(rng);

    let amount = rng.gen_range(1_000_000..10_000_000);
    let basic_output = BasicOutputBuilder::new_with_amount(amount)
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .finish()?;

    Ok((basic_output_header, basic_output))
}

fn random_nft_output(
    rng: &mut StdRng,
    owner: impl Into<Address>,
) -> anyhow::Result<(OutputHeader, NftOutput)> {
    let owner = owner.into();
    let nft_output_header = random_output_header(rng);
    let nft_metadata = Irc27Metadata::new("image/png", "https://nft.org/nft.png".parse()?, "NFT")
        .with_issuer_name("issuer_name")
        .with_collection_name("collection_name")
        .with_description("description");

    let amount = rng.gen_range(1_000_000..10_000_000);
    let nft_output = NftOutputBuilder::new_with_amount(amount, NftId::new(rng.gen()))
        .add_unlock_condition(AddressUnlockCondition::new(owner))
        .with_immutable_features(vec![
            Feature::Metadata(MetadataFeature::new(serde_json::to_vec(&nft_metadata)?)?),
            Feature::Issuer(IssuerFeature::new(owner)),
        ])
        .finish()?;

    Ok((nft_output_header, nft_output))
}

fn random_alias_output(
    rng: &mut StdRng,
    owner: impl Into<Address>,
) -> anyhow::Result<(OutputHeader, AliasOutput)> {
    let owner = owner.into();
    let alias_output_header = random_output_header(rng);

    let amount = rng.gen_range(1_000_000..10_000_000);
    let alias_output = AliasOutputBuilder::new_with_amount(amount, AliasId::new(rng.gen()))
        .add_unlock_condition(GovernorAddressUnlockCondition::new(owner))
        .add_unlock_condition(StateControllerAddressUnlockCondition::new(owner))
        .finish()?;

    Ok((alias_output_header, alias_output))
}

fn random_foundry_output(
    rng: &mut StdRng,
    serial_number: &mut u32,
    owner: impl Into<AliasAddress>,
) -> anyhow::Result<(OutputHeader, FoundryOutput)> {
    let foundry_output_header = random_output_header(rng);

    let amount = rng.gen_range(1_000_000..10_000_000);
    let supply = rng.gen_range(1_000_000..100_000_000);
    let token_scheme = SimpleTokenScheme::new(supply, 0, supply)?;
    let foundry_output =
        FoundryOutputBuilder::new_with_amount(amount, *serial_number, token_scheme.into())
            .with_unlock_conditions([UnlockCondition::from(
                ImmutableAliasAddressUnlockCondition::new(owner),
            )])
            .finish()?;

    *serial_number += 1;

    Ok((foundry_output_header, foundry_output))
}

fn random_output_header(rng: &mut StdRng) -> OutputHeader {
    OutputHeader::new(
        rng.gen(),
        OutputIndex::new(rng.gen_range(OUTPUT_INDEX_RANGE))
            .expect("range is guaranteed to be valid"),
        rng.gen(),
        MERGE_MILESTONE_INDEX,
        MERGE_TIMESTAMP_SECS,
    )
}
