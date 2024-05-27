// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types representing token schemes in Stardust.
use bigdecimal::num_bigint::BigInt;
use bigdecimal::{num_bigint, BigDecimal, One, ToPrimitive};
use iota_sdk::types::block::output::SimpleTokenScheme;
use iota_sdk::U256;

use crate::stardust::error::StardustError;

pub type TokenAdjustmentRatio = BigDecimal;

pub struct SimpleTokenSchemeU64 {
    // Circulating supply of tokens controlled by a foundry.
    circulating_supply: u64,
    // Maximum supply of tokens controlled by a foundry.
    maximum_supply: u64,
    // Ratio that the original circulating supply (U256) was adjusted by.
    // During migration, native token balances need to be multiplied by this ratio to account for that the original circulating supply may exceeded u64::MAX.
    // If the circulating supply is less than or equal to u64::MAX, this ratio is 1.
    token_adjustment_ratio: TokenAdjustmentRatio,
}

impl SimpleTokenSchemeU64 {
    pub fn circulating_supply(&self) -> u64 {
        self.circulating_supply
    }

    pub fn maximum_supply(&self) -> u64 {
        self.maximum_supply
    }

    pub fn token_adjustment_ratio(&self) -> &TokenAdjustmentRatio {
        &self.token_adjustment_ratio
    }
}

pub fn calculate_token_adjustment_ratio(
    circulating_supply_u256: U256,
) -> Result<TokenAdjustmentRatio, StardustError> {
    if circulating_supply_u256 > U256::from(u64::MAX) {
        let u64_max_bd = BigDecimal::from(u64::MAX);
        let circulating_supply_256_bd = u256_to_bigdecimal(circulating_supply_u256);
        Ok(u64_max_bd / &circulating_supply_256_bd)
    } else {
        Ok(BigDecimal::one())
    }
}

impl TryFrom<&SimpleTokenScheme> for SimpleTokenSchemeU64 {
    type Error = StardustError;
    fn try_from(token_scheme: &SimpleTokenScheme) -> Result<Self, StardustError> {
        let (circulating_supply_u64, token_ratio) = {
            let minted_tokens_u256 = token_scheme.minted_tokens();
            let melted_tokens_u256 = token_scheme.melted_tokens();
            let circulating_supply_u256 = minted_tokens_u256 - melted_tokens_u256;

            // Check if melted tokens is greater than minted tokens.
            if melted_tokens_u256 > minted_tokens_u256 {
                return Err(StardustError::MeltingTokensMustNotBeGreaterThanMintedTokens);
            }

            let ratio = calculate_token_adjustment_ratio(circulating_supply_u256)?;

            if circulating_supply_u256 > U256::from(u64::MAX) {
                (
                    (u256_to_bigdecimal(circulating_supply_u256) * &ratio)
                        .to_u64()
                        .expect("should be a valid u64"),
                    ratio,
                )
            } else {
                (circulating_supply_u256.as_u64(), BigDecimal::one())
            }
        };

        let maximum_supply_u64 = {
            let maximum_supply_u256 = token_scheme.maximum_supply();
            if maximum_supply_u256.bits() > 64 {
                u64::MAX
            } else {
                maximum_supply_u256.as_u64()
            }
        };

        Ok(Self {
            circulating_supply: circulating_supply_u64,
            maximum_supply: maximum_supply_u64,
            token_adjustment_ratio: token_ratio,
        })
    }
}

pub fn u256_to_bigdecimal(u256_value: U256) -> BigDecimal {
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
    use std::convert::TryFrom;
    use std::ops::Div;
    use std::str::FromStr;

    use iota_sdk::types::block::output::SimpleTokenScheme;
    use iota_sdk::U256;

    use super::*;

    #[test]
    fn calculate_token_adjustment_ratio_below_max() {
        let circulating_supply_u256 = U256::from(1_000_000_u64);
        let result = calculate_token_adjustment_ratio(circulating_supply_u256);
        assert!(result.is_ok());
        let expected_ratio = BigDecimal::one();
        assert_eq!(result.unwrap(), expected_ratio);
    }

    #[test]
    fn calculate_token_adjustment_ratio_at_max() {
        let circulating_supply_u256 = U256::from(u64::MAX);
        let result = calculate_token_adjustment_ratio(circulating_supply_u256);
        assert!(result.is_ok());
        let expected_ratio = BigDecimal::one();
        assert_eq!(result.unwrap(), expected_ratio);
    }

    #[test]
    fn calculate_token_adjustment_ratio_above_max() {
        let circulating_supply_u256 = U256::from(u64::MAX) + U256::from(1_u64);
        let result = calculate_token_adjustment_ratio(circulating_supply_u256);
        assert!(result.is_ok());

        let u64_max_bd = BigDecimal::from(u64::MAX);
        let circulating_supply_256_bd = u256_to_bigdecimal(circulating_supply_u256);
        let expected_ratio = u64_max_bd / circulating_supply_256_bd;

        assert_eq!(result.unwrap(), expected_ratio);
    }

    #[test]
    fn calculate_token_adjustment_ratio_large_value() {
        let circulating_supply_u256 = U256::from_dec_str(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        )
        .unwrap();
        let result = calculate_token_adjustment_ratio(circulating_supply_u256);
        assert!(result.is_ok());

        let u64_max_bd = BigDecimal::from(u64::MAX);
        let circulating_supply_256_bd = u256_to_bigdecimal(circulating_supply_u256);
        let expected_ratio = u64_max_bd / circulating_supply_256_bd;

        assert_eq!(result.unwrap(), expected_ratio);
    }

    #[test]
    fn calculate_token_adjustment_ratio_zero() {
        let circulating_supply_u256 = U256::from(0);
        let result = calculate_token_adjustment_ratio(circulating_supply_u256);
        assert!(result.is_ok());
        let expected_ratio = BigDecimal::one();
        assert_eq!(result.unwrap(), expected_ratio);
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
        assert_eq!(token_scheme_u64.token_adjustment_ratio, BigDecimal::from(1));
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
        assert_eq!(token_scheme_u64.maximum_supply, u64::MAX);
        assert_eq!(token_scheme_u64.token_adjustment_ratio, BigDecimal::from(1));
    }

    #[test]
    fn circulating_supply_ratio_calculation() {
        let minted_tokens = U256::from(u64::MAX) * U256::from(10);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let expected_ratio = BigDecimal::from(u64::MAX) / u256_to_bigdecimal(minted_tokens);
        assert_eq!(token_scheme_u64.circulating_supply, u64::MAX);
        assert_eq!(token_scheme_u64.token_adjustment_ratio, expected_ratio);
        assert_eq!(token_scheme_u64.maximum_supply, u64::MAX);
    }

    #[test]
    fn circulating_verify_token_balance_with_precision_loss() {
        let minted_tokens = U256::MAX;
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        assert_eq!(token_scheme_u64.maximum_supply, 18446744073709551615);
        // Smaller than maximum supply due to precision loss.
        assert_eq!(token_scheme_u64.circulating_supply, 18446744073709551614);

        // Verify that the adjusted balance divided by the token_adjustment_ratio is smaller than the minted tokens.
        let reversed =
            token_scheme_u64.circulating_supply() / token_scheme_u64.token_adjustment_ratio;
        assert!(reversed < u256_to_bigdecimal(minted_tokens));
    }

    #[test]
    fn circulating_supply_exceeds_u64_with_one_holder() {
        let minted_tokens = U256::from(u64::MAX) + U256::from(1);
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let address_balance = u256_to_bigdecimal(minted_tokens);
        let adjusted_address_balance = (address_balance * token_scheme_u64.token_adjustment_ratio)
            .to_u64()
            .unwrap();

        assert_eq!(token_scheme_u64.circulating_supply, u64::MAX);
        assert_eq!(adjusted_address_balance, u64::MAX);
        assert_eq!(token_scheme_u64.maximum_supply, u64::MAX);
    }

    #[test]
    fn circulating_supply_exceeds_u64_with_two_equal_holders() {
        let minted_tokens = U256::MAX;
        let melted_tokens = U256::from(0);
        let maximum_supply = U256::MAX;

        let token_scheme =
            SimpleTokenScheme::new(minted_tokens, melted_tokens, maximum_supply).unwrap();

        let token_scheme_u64 = SimpleTokenSchemeU64::try_from(&token_scheme).unwrap();

        let balance_share: BigDecimal = u256_to_bigdecimal(minted_tokens).div(2);

        let holder1_balance = &balance_share;
        let adjusted_holder1_balance = (holder1_balance * &token_scheme_u64.token_adjustment_ratio)
            .to_u64()
            .unwrap();

        let holder2_balance = &balance_share;
        let adjusted_holder2_balance = (holder2_balance * &token_scheme_u64.token_adjustment_ratio)
            .to_u64()
            .unwrap();

        assert_eq!(holder1_balance, holder2_balance);
        assert_eq!(
            holder1_balance + holder2_balance,
            u256_to_bigdecimal(U256::MAX)
        );
        assert_eq!(
            adjusted_holder1_balance + adjusted_holder2_balance,
            u64::MAX - 1
        );

        assert_eq!(token_scheme_u64.circulating_supply, u64::MAX - 1);

        assert_eq!(token_scheme_u64.maximum_supply, u64::MAX);
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
        assert_eq!(token_scheme_u64.token_adjustment_ratio, BigDecimal::from(1));
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
