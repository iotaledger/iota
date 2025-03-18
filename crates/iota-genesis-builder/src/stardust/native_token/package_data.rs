// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The `package_data` module provides the [`NativeTokenPackageData`] struct,
//! which encapsulates all the data necessary to build a Stardust native token
//! package.

use anyhow::Result;
use iota_sdk::types::block::{
    address::AliasAddress,
    output::{FoundryId, FoundryOutput},
};
use iota_types::stardust::error::StardustError;
use move_compiler::parser::keywords;
use rand::distributions::{Alphanumeric, DistString};
use rand_pcg::Pcg64;
use rand_seeder::{Seeder, SipRng, rand_core::RngCore};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::stardust::types::token_scheme::SimpleTokenSchemeU64;

/// The [`NativeTokenPackageData`] struct encapsulates all the data necessary to
/// build a Stardust native token package.
#[derive(Debug)]
pub struct NativeTokenPackageData {
    package_name: String,
    module: NativeTokenModuleData,
}

impl NativeTokenPackageData {
    /// Creates a new [`NativeTokenPackageData`] instance.
    pub fn new(package_name: impl Into<String>, module: NativeTokenModuleData) -> Self {
        Self {
            package_name: package_name.into(),
            module,
        }
    }

    /// Returns the Move.toml manifest.
    pub fn package_name(&self) -> &String {
        &self.package_name
    }

    /// Returns the native token module data.
    pub fn module(&self) -> &NativeTokenModuleData {
        &self.module
    }
}

/// The [`NativeTokenModuleData`] struct encapsulates all the data necessary to
/// build a Stardust native token module.
#[derive(Debug)]
pub struct NativeTokenModuleData {
    pub foundry_id: FoundryId,
    pub module_name: String,
    pub otw_name: String,
    pub decimals: u8,
    /// This must be a valid ASCII string.
    pub symbol: String,
    pub circulating_supply: u64,
    pub maximum_supply: u64,
    /// This must be a valid UTF-8 string.
    pub coin_name: String,
    /// This must be a valid UTF-8 string.
    pub coin_description: String,
    pub icon_url: Option<String>,
    pub alias_address: AliasAddress,
}

impl NativeTokenModuleData {
    /// Creates a new [`NativeTokenModuleData`] instance.
    pub fn new(
        foundry_id: FoundryId,
        module_name: impl Into<String>,
        otw_name: impl Into<String>,
        decimals: u8,
        symbol: impl Into<String>,
        circulating_supply: u64,
        maximum_supply: u64,
        coin_name: impl Into<String>,
        coin_description: impl Into<String>,
        icon_url: Option<String>,
        alias_address: AliasAddress,
    ) -> Self {
        Self {
            foundry_id,
            module_name: module_name.into(),
            otw_name: otw_name.into(),
            decimals,
            symbol: symbol.into(),
            circulating_supply,
            maximum_supply,
            coin_name: coin_name.into(),
            coin_description: coin_description.into(),
            icon_url,
            alias_address,
        }
    }
}

impl TryFrom<&FoundryOutput> for NativeTokenPackageData {
    type Error = StardustError;
    fn try_from(output: &FoundryOutput) -> Result<Self, StardustError> {
        let irc_30_metadata = extract_irc30_metadata(output);

        // Derive a valid, lowercase move identifier from the symbol field in the irc30
        // metadata
        let identifier = derive_foundry_package_lowercase_identifier(
            irc_30_metadata.symbol.as_str(),
            output.id().as_slice(),
        );

        // Any decimal value that exceeds a u8 is set to zero, as we cannot infer a good
        // alternative.
        let decimals = u8::try_from(irc_30_metadata.decimals).unwrap_or_default();

        let token_scheme_u64: SimpleTokenSchemeU64 =
            output.token_scheme().as_simple().try_into()?;

        let native_token_data = NativeTokenPackageData {
            package_name: identifier.clone(),
            module: NativeTokenModuleData {
                foundry_id: output.id(),
                module_name: identifier.clone(),
                otw_name: identifier.clone().to_ascii_uppercase(),
                decimals,
                symbol: identifier,
                circulating_supply: token_scheme_u64.circulating_supply(),
                maximum_supply: token_scheme_u64.maximum_supply(),
                coin_name: irc_30_metadata.name,
                coin_description: irc_30_metadata.description.unwrap_or_default(),
                icon_url: irc_30_metadata.logo_url,
                alias_address: *output.alias_address(),
            },
        };

        Ok(native_token_data)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "standard", rename = "IRC30")]
pub struct Irc30MetadataAlternative {
    /// The human-readable name of the native token.
    name: String,
    /// The symbol/ticker of the token.
    symbol: String,
    /// Number of decimals the token uses (divide the token amount by
    /// `10^decimals` to get its user representation).
    decimals: u32,
    /// The human-readable description of the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// URL pointing to more resources about the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    /// URL pointing to an image resource of the token logo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo_url: Option<String>,
    /// The svg logo of the token encoded as a byte string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<String>,
}

impl Irc30MetadataAlternative {
    fn new_compact(name: String) -> Self {
        Irc30MetadataAlternative {
            name: name.clone(),
            symbol: name,
            decimals: 0,
            description: None,
            url: None,
            logo_url: None,
            logo: None,
        }
    }
}

fn extract_irc30_metadata(output: &FoundryOutput) -> Irc30MetadataAlternative {
    output
        .immutable_features()
        .metadata()
        .and_then(|metadata| {
            serde_json::from_slice::<Irc30MetadataAlternative>(metadata.data()).ok()
        })
        .and_then(|metadata| {
            metadata
                .logo_url
                .as_ref()
                .map(|url| url.is_ascii())
                .unwrap_or(true)
                .then_some(metadata)
        })
        .unwrap_or_else(|| {
            Irc30MetadataAlternative::new_compact(derive_foundry_package_lowercase_identifier(
                "",
                output.id().as_slice(),
            ))
        })
}

fn derive_foundry_package_lowercase_identifier(input: &str, seed: &[u8]) -> String {
    let input = input.to_ascii_lowercase();

    static VALID_IDENTIFIER_PATTERN: &str = r"[a-z][a-z0-9_]*";

    // Define a regex pattern to capture the valid parts of the identifier
    let valid_parts_re =
        Regex::new(VALID_IDENTIFIER_PATTERN).expect("should be valid regex pattern");
    let valid_parts: Vec<&str> = valid_parts_re
        .find_iter(&input)
        .map(|mat| mat.as_str())
        .collect();
    let concatenated = valid_parts.concat();

    // Ensure no trailing underscore at the end of the identifier
    let refined_identifier = concatenated.trim_end_matches('_').to_string();
    let is_valid = move_core_types::identifier::is_valid(&refined_identifier);

    if is_valid
        && !keywords::KEYWORDS.contains(&refined_identifier.as_str())
        && !keywords::CONTEXTUAL_KEYWORDS.contains(&refined_identifier.as_str())
        && !keywords::PRIMITIVE_TYPES.contains(&refined_identifier.as_str())
        && !keywords::BUILTINS.contains(&refined_identifier.as_str())
    {
        refined_identifier
    } else {
        let mut final_identifier = String::from("foundry_");
        let additional_part = if is_valid {
            refined_identifier
        } else {
            let mut rng: SipRng = Seeder::from(seed).make_rng();
            fn next_u128(rng: &mut SipRng) -> u128 {
                ((rng.next_u64() as u128) << 64) | rng.next_u64() as u128
            }
            // Generate a new valid random identifier if the identifier is empty.
            Alphanumeric
                .sample_string(&mut Pcg64::new(next_u128(&mut rng), next_u128(&mut rng)), 7)
                .to_lowercase()
        };
        final_identifier.push_str(&additional_part);
        final_identifier
    }
}

#[cfg(test)]
mod tests {
    use std::ops::{Add, Sub};

    use iota_sdk::{
        U256, Url,
        types::block::{
            address::AliasAddress,
            output::{
                AliasId, Feature, FoundryOutputBuilder, SimpleTokenScheme, TokenScheme,
                feature::{Irc30Metadata, MetadataFeature},
                unlock_condition::ImmutableAliasAddressUnlockCondition,
            },
        },
    };

    use super::*;
    use crate::stardust::{
        native_token::package_builder, types::token_scheme::MAX_ALLOWED_U64_SUPPLY,
    };

    #[test]
    fn foundry_output_with_default_metadata() -> Result<()> {
        // Step 1: Create a FoundryOutput with an IRC30Metadata feature
        let token_scheme = SimpleTokenScheme::new(
            U256::from(100_000_000),
            U256::from(0),
            U256::from(100_000_000),
        )
        .unwrap();

        let irc_30_metadata = Irc30Metadata::new("Dogecoin", "DOGE❤", 0);

        let alias_id = AliasId::new([0; AliasId::LENGTH]);
        let builder = FoundryOutputBuilder::new_with_amount(
            100_000_000_000,
            1,
            TokenScheme::Simple(token_scheme),
        )
        .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
            AliasAddress::new(alias_id),
        ))
        .add_feature(Feature::Metadata(
            MetadataFeature::new(irc_30_metadata).unwrap(),
        ));
        let output = builder.finish().unwrap();

        // Step 2: Convert the FoundryOutput to NativeTokenPackageData
        let native_token_data = NativeTokenPackageData::try_from(&output)?;

        // Step 3: Verify the conversion
        assert!(package_builder::build_and_compile(native_token_data).is_ok());

        Ok(())
    }

    #[test]
    fn foundry_output_with_additional_metadata() -> Result<()> {
        // Step 1: Create a FoundryOutput with an IRC30Metadata feature
        let token_scheme = SimpleTokenScheme::new(
            U256::from(100_000_000),
            U256::from(0),
            U256::from(100_000_000),
        )
        .unwrap();

        let irc_30_metadata = Irc30Metadata::new("Dogecoin", "DOGE", 0)
            .with_description("Much wow")
            .with_url(Url::parse("https://dogecoin.com").unwrap())
            .with_logo_url(Url::parse("https://dogecoin.com/logo.png").unwrap())
            .with_logo("0x54654");

        let alias_id = AliasId::new([0; AliasId::LENGTH]);
        let builder = FoundryOutputBuilder::new_with_amount(
            100_000_000_000,
            1,
            TokenScheme::Simple(token_scheme),
        )
        .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
            AliasAddress::new(alias_id),
        ))
        .add_feature(Feature::Metadata(
            MetadataFeature::new(irc_30_metadata).unwrap(),
        ));
        let output = builder.finish().unwrap();

        // Step 2: Convert the FoundryOutput to NativeTokenPackageData
        let native_token_data = NativeTokenPackageData::try_from(&output)?;

        // Step 3: Verify the conversion
        assert!(package_builder::build_and_compile(native_token_data).is_ok());

        Ok(())
    }

    #[test]
    fn foundry_output_with_exceeding_max_supply() -> Result<()> {
        let minted_tokens = U256::from(MAX_ALLOWED_U64_SUPPLY).add(1);
        let melted_tokens = U256::from(1);
        let maximum_supply = U256::MAX;

        // Step 1: Create a FoundryOutput with an IRC30Metadata feature
        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let irc_30_metadata = Irc30Metadata::new("Dogecoin", "DOGE", 0)
            .with_description("Much wow")
            .with_url(Url::parse("https://dogecoin.com").unwrap())
            .with_logo_url(Url::parse("https://dogecoin.com/logo.png").unwrap())
            .with_logo("0x54654");

        let alias_id = AliasId::new([0; AliasId::LENGTH]);
        let builder = FoundryOutputBuilder::new_with_amount(
            100_000_000_000,
            1,
            TokenScheme::Simple(token_scheme),
        )
        .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
            AliasAddress::new(alias_id),
        ))
        .add_feature(Feature::Metadata(
            MetadataFeature::new(irc_30_metadata).unwrap(),
        ));
        let output = builder.finish().unwrap();

        // Step 2: Convert the FoundryOutput to NativeTokenPackageData
        let native_token_data = NativeTokenPackageData::try_from(&output)?;
        assert_eq!(
            native_token_data.module().circulating_supply,
            minted_tokens.sub(melted_tokens).as_u64()
        );
        assert_eq!(
            native_token_data.module().maximum_supply,
            MAX_ALLOWED_U64_SUPPLY
        );

        // Step 3: Verify the conversion
        assert!(package_builder::build_and_compile(native_token_data).is_ok());

        Ok(())
    }

    #[test]
    fn foundry_output_with_exceeding_circulating_supply() -> Result<()> {
        let minted_tokens = U256::from(u64::MAX).add(1);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        // Step 1: Create a FoundryOutput with an IRC30Metadata feature
        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let irc_30_metadata = Irc30Metadata::new("Dogecoin", "DOGE", 0)
            .with_description("Much wow")
            .with_url(Url::parse("https://dogecoin.com").unwrap())
            .with_logo_url(Url::parse("https://dogecoin.com/logo.png").unwrap())
            .with_logo("0x54654");

        let alias_id = AliasId::new([0; AliasId::LENGTH]);
        let builder = FoundryOutputBuilder::new_with_amount(
            100_000_000_000,
            1,
            TokenScheme::Simple(token_scheme),
        )
        .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
            AliasAddress::new(alias_id),
        ))
        .add_feature(Feature::Metadata(
            MetadataFeature::new(irc_30_metadata).unwrap(),
        ));
        let output = builder.finish().unwrap();

        // Step 2: Convert the FoundryOutput to NativeTokenPackageData.
        let native_token_data = NativeTokenPackageData::try_from(&output)?;

        assert_eq!(
            native_token_data.module().circulating_supply,
            MAX_ALLOWED_U64_SUPPLY
        );
        assert_eq!(
            native_token_data.module().maximum_supply,
            MAX_ALLOWED_U64_SUPPLY
        );

        // Step 3: Verify the conversion
        assert!(package_builder::build_and_compile(native_token_data).is_ok());

        Ok(())
    }

    #[test]
    fn empty_identifier() {
        let identifier = "".to_string();
        let result = derive_foundry_package_lowercase_identifier(&identifier, &[]);
        assert_eq!(15, result.len());
    }

    #[test]
    fn identifier_with_only_invalid_chars() {
        let identifier = "!@#$%^".to_string();
        let result = derive_foundry_package_lowercase_identifier(&identifier, &[]);
        assert_eq!(15, result.len());
    }

    #[test]
    fn identifier_with_only_one_char() {
        let identifier = "a".to_string();
        assert_eq!(
            derive_foundry_package_lowercase_identifier(&identifier, &[]),
            "a".to_string()
        );
    }

    #[test]
    fn identifier_with_whitespaces_and_ending_underscore() {
        let identifier = " a bc-d e_".to_string();
        assert_eq!(
            derive_foundry_package_lowercase_identifier(&identifier, &[]),
            "abcde".to_string()
        );
    }

    #[test]
    fn identifier_with_minus() {
        let identifier = "hello-world".to_string();
        assert_eq!(
            derive_foundry_package_lowercase_identifier(&identifier, &[]),
            "helloworld".to_string()
        );
    }

    #[test]
    fn identifier_with_multiple_invalid_chars() {
        let identifier = "#hello-move_world/token&".to_string();
        assert_eq!(
            derive_foundry_package_lowercase_identifier(&identifier, &[]),
            "hellomove_worldtoken"
        );
    }
    #[test]
    fn valid_identifier() {
        let identifier = "valid_identifier".to_string();
        assert_eq!(
            derive_foundry_package_lowercase_identifier(&identifier, &[]),
            identifier
        );
    }
}
