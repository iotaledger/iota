// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types representing token schemes in Stardust.
use bigdecimal::{BigDecimal, ToPrimitive, num_bigint, num_bigint::BigInt};
use iota_sdk::{U256, types::block::output::SimpleTokenScheme};
use iota_types::stardust::error::StardustError;

/// The maximum allowed u64 supply.
pub const MAX_ALLOWED_U64_SUPPLY: u64 = u64::MAX - 1;

/// This struct represents a conversion from a `SimpleTokenScheme` to a
/// `SimpleTokenSchemeU64`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleTokenSchemeU64 {
    // Maximum supply of tokens controlled by a foundry.
    maximum_supply: u64,
    // Circulating supply of tokens controlled by a foundry.
    circulating_supply: u64,
    // The ratio that the original circulating_supply (U256) was adjusted by in regards to the
    // adjusted maximum supply (u64). During migration, native token balances need to be
    // multiplied by this ratio to account for that the original maximum supply may exceeded
    // u64::MAX. In case the original maximum supply was below u64::MAX, this value is None.
    token_adjustment_ratio: Option<BigDecimal>,
}

impl SimpleTokenSchemeU64 {
    /// The maximum supply of tokens controlled by a foundry.
    pub fn maximum_supply(&self) -> u64 {
        self.maximum_supply
    }
    /// The circulating supply of tokens controlled by a foundry.
    pub fn circulating_supply(&self) -> u64 {
        self.circulating_supply
    }
    /// The ratio that the original circulating_supply (U256) was adjusted by in
    /// regards to the adjusted maximum supply (u64).
    pub fn token_adjustment_ratio(&self) -> &Option<BigDecimal> {
        &self.token_adjustment_ratio
    }

    /// Constrain U256 tokens to a u64 using the token adjustment ratio.
    pub fn adjust_tokens(&self, tokens: U256) -> u64 {
        if let Some(ratio) = self.token_adjustment_ratio() {
            (u256_to_bigdecimal(tokens) * ratio)
                .to_u64()
                .expect("should be a valid u64")
        } else {
            tokens.as_u64()
        }
    }
}

impl TryFrom<&SimpleTokenScheme> for SimpleTokenSchemeU64 {
    type Error = StardustError;
    fn try_from(token_scheme: &SimpleTokenScheme) -> Result<Self, StardustError> {
        let minted_tokens_u256 = token_scheme.minted_tokens();
        let melted_tokens_u256 = token_scheme.melted_tokens();
        let maximum_supply_u256 = token_scheme.maximum_supply();
        let circulating_supply_u256 = token_scheme.circulating_supply();

        let (circulating_supply, maximum_supply, token_adjustment_ratio) = {
            // Check if circulating supply is larger than maximum supply.
            if circulating_supply_u256 > maximum_supply_u256 {
                return Err(StardustError::CirculatingSupplyMustNotBeGreaterThanMaximumSupply);
            }

            // Check if melted tokens is greater than minted tokens.
            if melted_tokens_u256 > minted_tokens_u256 {
                return Err(StardustError::MeltingTokensMustNotBeGreaterThanMintedTokens);
            }

            // Check if maximum supply can't be converted to u64.
            let maximum_supply_u64 = if maximum_supply_u256 > U256::from(MAX_ALLOWED_U64_SUPPLY) {
                MAX_ALLOWED_U64_SUPPLY
            } else {
                maximum_supply_u256.as_u64()
            };

            // Check if circulating supply can't be converted to max allowed u64 supply.
            if circulating_supply_u256 > U256::from(MAX_ALLOWED_U64_SUPPLY) {
                (
                    MAX_ALLOWED_U64_SUPPLY,
                    MAX_ALLOWED_U64_SUPPLY,
                    Some(
                        BigDecimal::from(MAX_ALLOWED_U64_SUPPLY)
                            / u256_to_bigdecimal(circulating_supply_u256),
                    ),
                )
            } else {
                (circulating_supply_u256.as_u64(), maximum_supply_u64, None)
            }
        };

        Ok(Self {
            circulating_supply,
            maximum_supply,
            token_adjustment_ratio,
        })
    }
}

/// Converts a U256 to a BigDecimal.
fn u256_to_bigdecimal(u256_value: U256) -> BigDecimal {
    // Allocate a mutable array for the big-endian bytes
    let mut bytes = [0u8; 32];
    u256_value.to_big_endian(&mut bytes);

    // Convert the byte array to BigInt
    let bigint_value = BigInt::from_bytes_be(num_bigint::Sign::Plus, &bytes);

    // Convert BigInt to BigDecimal
    BigDecimal::from(bigint_value)
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, ops::Div, str::FromStr};

    use bigdecimal::ToPrimitive;
    use iota_sdk::{U256, types::block::output::SimpleTokenScheme};

    use super::*;

    #[test]
    fn calculate_token_adjustment_ratio_below_max() {
        let minted_tokens = U256::from(1_000_000_u64);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::from(1_000_000_u64);

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();
        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert!(token_scheme_u64.token_adjustment_ratio.is_none());
    }

    #[test]
    fn calculate_token_adjustment_ratio_at_max() {
        let minted_tokens = U256::from(MAX_ALLOWED_U64_SUPPLY);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::from(MAX_ALLOWED_U64_SUPPLY);

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();
        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert!(token_scheme_u64.token_adjustment_ratio.is_none());
    }

    #[test]
    fn calculate_token_adjustment_ratio_above_max() {
        let minted_tokens = U256::from(MAX_ALLOWED_U64_SUPPLY) + U256::from(1_u64);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::from(MAX_ALLOWED_U64_SUPPLY) + U256::from(1_u64);
        let circulating_supply_u256 = minted_tokens - melted_tokens;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();
        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let u64_max_bd = BigDecimal::from(MAX_ALLOWED_U64_SUPPLY);
        let circulating_supply_256_bd = u256_to_bigdecimal(circulating_supply_u256);
        let expected_ratio = u64_max_bd / circulating_supply_256_bd;

        assert_eq!(
            token_scheme_u64.token_adjustment_ratio.unwrap(),
            expected_ratio
        );
    }

    #[test]
    fn calculate_token_adjustment_ratio_zero() {
        let minted_tokens = U256::from(0);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::from(u64::MAX);

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();
        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert!(token_scheme_u64.token_adjustment_ratio.is_none());
    }

    #[test]
    fn valid_conversion_within_u64_limits() {
        let minted_tokens = U256::from(5000);
        let melted_tokens = U256::from(1000);
        let maximum_supply = U256::from(10000);

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert_eq!(token_scheme_u64.circulating_supply, 4000);
        assert_eq!(token_scheme_u64.maximum_supply, 10000);
        assert!(token_scheme_u64.token_adjustment_ratio.is_none());
    }

    #[test]
    fn maximum_supply_exceeds_u64() {
        let minted_tokens = U256::from(5000);
        let melted_tokens = U256::from(1000);
        let maximum_supply = U256::from(u64::MAX) + U256::from(1);

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert_eq!(token_scheme_u64.circulating_supply, 4000);
        assert_eq!(token_scheme_u64.maximum_supply, MAX_ALLOWED_U64_SUPPLY);
        assert!(token_scheme_u64.token_adjustment_ratio.is_none());
    }

    #[test]
    fn circulating_supply_ratio_calculation() {
        let minted_tokens = U256::from(MAX_ALLOWED_U64_SUPPLY) * U256::from(10);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let expected_ratio =
            BigDecimal::from(MAX_ALLOWED_U64_SUPPLY) / u256_to_bigdecimal(minted_tokens);
        assert_eq!(token_scheme_u64.circulating_supply, MAX_ALLOWED_U64_SUPPLY);
        assert_eq!(
            token_scheme_u64.token_adjustment_ratio.unwrap(),
            expected_ratio
        );
        assert_eq!(token_scheme_u64.maximum_supply, MAX_ALLOWED_U64_SUPPLY);
    }

    #[test]
    fn circulating_verify_token_balance() {
        let minted_tokens = U256::MAX;
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert_eq!(token_scheme_u64.maximum_supply, MAX_ALLOWED_U64_SUPPLY);
        assert_eq!(token_scheme_u64.circulating_supply, MAX_ALLOWED_U64_SUPPLY);

        // Verify that the adjusted balance divided by the token_adjustment_ratio is
        // equal the minted tokens.
        let reversed = token_scheme_u64.circulating_supply()
            / token_scheme_u64.token_adjustment_ratio.unwrap();
        assert_eq!(reversed, u256_to_bigdecimal(minted_tokens));
    }

    #[test]
    fn circulating_supply_exceeds_u64_with_one_holder() {
        let minted_tokens = U256::from(MAX_ALLOWED_U64_SUPPLY) + U256::from(1);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let address_balance = u256_to_bigdecimal(minted_tokens);
        let adjusted_address_balance = (address_balance
            * token_scheme_u64.token_adjustment_ratio.unwrap())
        .to_u64()
        .unwrap();

        assert_eq!(adjusted_address_balance, MAX_ALLOWED_U64_SUPPLY);
        assert_eq!(token_scheme_u64.circulating_supply, MAX_ALLOWED_U64_SUPPLY);
        assert_eq!(token_scheme_u64.maximum_supply, MAX_ALLOWED_U64_SUPPLY);
    }

    #[test]
    fn circulating_supply_exceeds_u64_with_two_equal_holders() {
        let minted_tokens = U256::from(MAX_ALLOWED_U64_SUPPLY) + U256::from(1);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let balance_share: BigDecimal = u256_to_bigdecimal(minted_tokens).div(2);

        let holder1_balance = &balance_share;
        let adjusted_holder1_balance = (holder1_balance
            * &token_scheme_u64.token_adjustment_ratio.clone().unwrap())
            .to_u64()
            .unwrap();

        let holder2_balance = &balance_share;
        let adjusted_holder2_balance = (holder2_balance
            * &token_scheme_u64.token_adjustment_ratio.unwrap())
            .to_u64()
            .unwrap();

        assert_eq!(adjusted_holder1_balance, MAX_ALLOWED_U64_SUPPLY / 2);

        assert_eq!(adjusted_holder2_balance, MAX_ALLOWED_U64_SUPPLY / 2);

        assert_eq!(
            adjusted_holder1_balance + adjusted_holder2_balance,
            MAX_ALLOWED_U64_SUPPLY
        );

        assert_eq!(token_scheme_u64.circulating_supply, MAX_ALLOWED_U64_SUPPLY);

        assert_eq!(token_scheme_u64.maximum_supply, MAX_ALLOWED_U64_SUPPLY);
    }

    #[test]
    fn zero_minted_and_melted_tokens() {
        let minted_tokens = U256::from(0);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::from(10000);

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert_eq!(token_scheme_u64.circulating_supply, 0);
        assert_eq!(token_scheme_u64.maximum_supply, 10000);
        assert!(token_scheme_u64.token_adjustment_ratio.is_none());
    }

    #[test]
    fn u256_to_bigdecimal_zero() {
        let value = U256::zero();
        let result = u256_to_bigdecimal(value);
        assert_eq!(result, BigDecimal::from(0));
    }

    #[test]
    fn u256_to_bigdecimal_mid() {
        let value = U256::from_dec_str("1234567890123456789012345678901234567890").unwrap();
        let expected = BigDecimal::from_str("1234567890123456789012345678901234567890").unwrap();
        let result = u256_to_bigdecimal(value);
        assert_eq!(result, expected);
    }

    #[test]
    fn u256_to_bigdecimal_max() {
        let value = U256::max_value();
        let expected = BigInt::from_bytes_be(
            num_bigint::Sign::Plus,
            &[
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff,
            ],
        );
        let expected = BigDecimal::from(expected);
        let result = u256_to_bigdecimal(value);
        assert_eq!(result, expected);
    }
}
