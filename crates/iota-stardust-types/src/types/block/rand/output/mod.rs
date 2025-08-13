// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Module providing random feature generation utilities.
pub mod feature;
/// Module providing random output metadata generation utilities.
pub mod metadata;
/// Module providing random unlock condition generation utilities.
pub mod unlock_condition;

use primitive_types::U256;

pub use self::metadata::rand_output_metadata;
use crate::types::block::{
    output::{
        AliasId, AliasOutput, BasicOutput, FoundryOutput, InputsCommitment, NftId, NftOutput,
        OUTPUT_INDEX_RANGE, Output, OutputId, SimpleTokenScheme, TokenScheme, TreasuryOutput,
        unlock_condition::ImmutableAliasAddressUnlockCondition,
    },
    rand::{
        address::rand_alias_address,
        bytes::rand_bytes_array,
        number::{rand_number, rand_number_range},
        output::{
            feature::rand_allowed_features,
            unlock_condition::{
                rand_address_unlock_condition, rand_address_unlock_condition_different_from,
                rand_governor_address_unlock_condition_different_from,
                rand_state_controller_address_unlock_condition_different_from,
            },
        },
        transaction::rand_transaction_id,
    },
};

/// Generates a random [`OutputId`].
pub fn rand_output_id() -> OutputId {
    OutputId::new(rand_transaction_id(), rand_number_range(OUTPUT_INDEX_RANGE)).unwrap()
}

/// Generates a random treasury output.
pub fn rand_treasury_output(token_supply: u64) -> TreasuryOutput {
    TreasuryOutput::new(rand_number_range(0..token_supply), token_supply).unwrap()
}

/// Generates a random [`BasicOutput`].
pub fn rand_basic_output(token_supply: u64) -> BasicOutput {
    // TODO: Add `NativeTokens`
    BasicOutput::build_with_amount(rand_number_range(Output::AMOUNT_MIN..token_supply))
        .with_features(rand_allowed_features(BasicOutput::ALLOWED_FEATURES))
        .add_unlock_condition(rand_address_unlock_condition())
        .finish_with_params(token_supply)
        .unwrap()
}

/// Generates a random [`AliasId`].
pub fn rand_alias_id() -> AliasId {
    AliasId::from(rand_bytes_array())
}

/// Generates a random [`AliasOutput`].
pub fn rand_alias_output(token_supply: u64) -> AliasOutput {
    // We need to make sure that `AliasId` and `Address` don't match.
    let alias_id = rand_alias_id();

    AliasOutput::build_with_amount(
        rand_number_range(Output::AMOUNT_MIN..token_supply),
        alias_id,
    )
    .with_features(rand_allowed_features(AliasOutput::ALLOWED_FEATURES))
    .add_unlock_condition(rand_state_controller_address_unlock_condition_different_from(&alias_id))
    .add_unlock_condition(rand_governor_address_unlock_condition_different_from(
        &alias_id,
    ))
    .finish_with_params(token_supply)
    .unwrap()
}

/// Generates a random [`TokenScheme`].
pub fn rand_token_scheme() -> TokenScheme {
    let max = U256::from(rand_bytes_array()).saturating_add(U256::one());
    let minted = U256::from(rand_bytes_array()) % max.saturating_add(U256::one());
    let melted = U256::from(rand_bytes_array()) % minted.saturating_add(U256::one());

    TokenScheme::Simple(SimpleTokenScheme::new(minted, melted, max).unwrap())
}

/// Generates a random [`FoundryOutput`].
pub fn rand_foundry_output(token_supply: u64) -> FoundryOutput {
    FoundryOutput::build_with_amount(
        rand_number_range(Output::AMOUNT_MIN..token_supply),
        rand_number(),
        rand_token_scheme(),
    )
    .with_features(rand_allowed_features(FoundryOutput::ALLOWED_FEATURES))
    .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
        rand_alias_address(),
    ))
    .finish_with_params(token_supply)
    .unwrap()
}

/// Generates a random [`NftOutput`].
pub fn rand_nft_output(token_supply: u64) -> NftOutput {
    // We need to make sure that `NftId` and `Address` don't match.
    let nft_id = NftId::from(rand_bytes_array());

    NftOutput::build_with_amount(rand_number_range(Output::AMOUNT_MIN..token_supply), nft_id)
        .with_features(rand_allowed_features(NftOutput::ALLOWED_FEATURES))
        .add_unlock_condition(rand_address_unlock_condition_different_from(&nft_id))
        .finish_with_params(token_supply)
        .unwrap()
}

/// Generates a random [`InputsCommitment`].
pub fn rand_inputs_commitment() -> InputsCommitment {
    InputsCommitment::from(rand_bytes_array())
}

/// Generates a random [`Output`].
pub fn rand_output(token_supply: u64) -> Output {
    match rand_number::<u64>() % 5 {
        0 => rand_treasury_output(token_supply).into(),
        1 => rand_basic_output(token_supply).into(),
        2 => rand_alias_output(token_supply).into(),
        3 => rand_foundry_output(token_supply).into(),
        4 => rand_nft_output(token_supply).into(),
        _ => unreachable!(),
    }
}
