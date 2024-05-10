// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The [`package_data`] module provides the [`NativeTokenPackageData`] struct, which encapsulates all the data necessary to build a Stardust native token package.

use crate::stardust::error::StardustError;
use anyhow::Result;
use iota_sdk::types::block::address::AliasAddress;
use iota_sdk::types::block::output::feature::Irc30Metadata;
use iota_sdk::types::block::output::{FoundryId, FoundryOutput};
use iota_sdk::Url;
use rand::Rng;
use regex::Regex;

/// The [`NativeTokenPackageData`] struct encapsulates all the data necessary to build a Stardust native token package.
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

/// The [`NativeTokenModuleData`] struct encapsulates all the data necessary to build a Stardust native token module.
#[derive(Debug)]
pub struct NativeTokenModuleData {
    pub foundry_id: FoundryId,
    pub module_name: String,
    pub otw_name: String,
    pub decimals: u8,
    pub symbol: String,
    pub circulating_tokens: u64,
    pub maximum_supply: u64,
    pub coin_name: String,
    pub coin_description: String,
    pub icon_url: Option<Url>,
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
        circulating_tokens: u64,
        maximum_supply: u64,
        coin_name: impl Into<String>,
        coin_description: impl Into<String>,
        icon_url: Option<Url>,
        alias_address: AliasAddress,
    ) -> Self {
        Self {
            foundry_id,
            module_name: module_name.into(),
            otw_name: otw_name.into(),
            decimals,
            symbol: symbol.into(),
            circulating_tokens,
            maximum_supply,
            coin_name: coin_name.into(),
            coin_description: coin_description.into(),
            icon_url,
            alias_address,
        }
    }
}

impl TryFrom<FoundryOutput> for NativeTokenPackageData {
    type Error = StardustError;
    fn try_from(output: FoundryOutput) -> Result<Self, StardustError> {
        let metadata =
            output
                .features()
                .metadata()
                .ok_or(StardustError::FoundryConversionError {
                    foundry_id: output.id(),
                    err: anyhow::anyhow!("metadata not found"),
                })?;
        let irc_30_metadata: Irc30Metadata =
            serde_json::from_slice(metadata.data()).map_err(|e| {
                StardustError::FoundryConversionError {
                    foundry_id: output.id(),
                    err: e.into(),
                }
            })?;

        // Derive a valid, lowercase move identifier from the symbol field in the irc30 metadata
        let identifier = derive_lowercase_identifier(&irc_30_metadata.symbol())?;

        let decimals = u8::try_from(*irc_30_metadata.decimals()).map_err(|e| {
            StardustError::FoundryConversionError {
                foundry_id: output.id(),
                err: e.into(),
            }
        })?;

        let maximum_supply_u256 = output.token_scheme().as_simple().maximum_supply();
        let maximum_supply_u64 = if maximum_supply_u256.bits() > 64 {
            u64::MAX
        } else {
            maximum_supply_u256.as_u64()
        };

        let native_token_data = NativeTokenPackageData {
            package_name: identifier.clone(),
            module: NativeTokenModuleData {
                foundry_id: output.id(),
                module_name: identifier.clone(),
                otw_name: identifier.clone().to_ascii_uppercase(),
                decimals,
                symbol: identifier,
                circulating_tokens: output.token_scheme().as_simple().minted_tokens().as_u64()
                    - output.token_scheme().as_simple().melted_tokens().as_u64(), // we know that "Melted Tokens must not be greater than Minted Tokens"
                maximum_supply: maximum_supply_u64,
                coin_name: irc_30_metadata.name().to_owned(),
                coin_description: irc_30_metadata.description().clone().unwrap_or_default(),
                icon_url: irc_30_metadata.url().clone(),
                alias_address: *output.alias_address(),
            },
        };

        Ok(native_token_data)
    }
}

fn derive_lowercase_identifier(input: &String) -> Result<String, StardustError> {
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
    let final_identifier = concatenated.trim_end_matches('_').to_string();

    if final_identifier.len() > 0 {
        if move_core_types::identifier::is_valid(&final_identifier) {
            Ok(final_identifier)
        } else {
            Err(StardustError::InvalidMoveIdentifierDerived {
                symbol: input,
                identifier: final_identifier,
            })
        }
    } else {
        // Generate a new valid random identifier if still invalid
        let mut rng = rand::thread_rng();
        let gen = rand_regex::Regex::compile(VALID_IDENTIFIER_PATTERN, 100).unwrap();
        let res: String = rng.sample(&gen);
        if res.len() > 7 {
            Ok(res[..7].to_string())
        } else {
            Ok(res)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::stardust::native_token::package_builder;
    use iota_sdk::types::block::address::AliasAddress;
    use iota_sdk::types::block::output::feature::MetadataFeature;
    use iota_sdk::types::block::output::unlock_condition::ImmutableAliasAddressUnlockCondition;
    use iota_sdk::types::block::output::{
        AliasId, Feature, FoundryOutputBuilder, SimpleTokenScheme, TokenScheme,
    };
    use iota_sdk::U256;

    use super::*;

    #[test]
    fn test_foundry_output_with_default_metadata() -> Result<()> {
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
        let native_token_data = NativeTokenPackageData::try_from(output)?;

        // Step 3: Verify the conversion
        assert!(package_builder::build_and_compile(native_token_data).is_ok());

        Ok(())
    }

    #[test]
    fn test_foundry_output_with_additional_metadata() -> Result<()> {
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
        let native_token_data = NativeTokenPackageData::try_from(output)?;

        // Step 3: Verify the conversion
        assert!(package_builder::build_and_compile(native_token_data).is_ok());

        Ok(())
    }

    #[test]
    fn test_empty_identifier() {
        let identifier = "".to_string();
        let result = derive_lowercase_identifier(&identifier).unwrap();
        assert_eq!(7, result.len());
    }

    #[test]
    fn test_identifier_with_only_invalid_chars() {
        let identifier = "!@#$%^".to_string();
        let result = derive_lowercase_identifier(&identifier).unwrap();
        assert_eq!(7, result.len());
    }

    #[test]
    fn test_identifier_with_only_one_char() {
        let identifier = "a".to_string();
        assert_eq!(
            derive_lowercase_identifier(&identifier).unwrap(),
            "a".to_string()
        );
    }

    #[test]
    fn test_identifier_with_whitespaces_and_ending_underscore() {
        let identifier = " a bc-d e_".to_string();
        assert_eq!(
            derive_lowercase_identifier(&identifier).unwrap(),
            "abcde".to_string()
        );
    }

    #[test]
    fn test_identifier_with_minus() {
        let identifier = "hello-world".to_string();
        assert_eq!(
            derive_lowercase_identifier(&identifier).unwrap(),
            "helloworld".to_string()
        );
    }

    #[test]
    fn test_identifier_with_multiple_invalid_chars() {
        let identifier = "#hello-move_world/token&".to_string();
        assert_eq!(
            derive_lowercase_identifier(&identifier).unwrap(),
            "hellomove_worldtoken"
        );
    }
    #[test]
    fn test_valid_identifier() {
        let identifier = "valid_identifier".to_string();
        assert_eq!(
            derive_lowercase_identifier(&identifier).unwrap(),
            identifier
        );
    }
}
