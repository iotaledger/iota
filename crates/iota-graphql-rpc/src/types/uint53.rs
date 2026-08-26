// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use async_graphql::*;
use iota_sdk_types::Version;
use iota_types::iota_serde::BigInt;

use crate::error::Error;

/// The largest value that a `UInt53` can hold, 2^53 - 1.
const MAX_UINT53: u64 = (1 << 53) - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct UInt53(u64);

/// An unsigned integer that can hold values up to 2^53 - 1. This can be treated
/// similarly to `Int`, but it is guaranteed to be non-negative, and it may be
/// larger than 2^32 - 1.
#[Scalar(name = "UInt53")]
impl ScalarType for UInt53 {
    fn parse(value: Value) -> InputValueResult<Self> {
        let Value::Number(n) = value else {
            return Err(InputValueError::expected_type(value));
        };

        let Some(n) = n.as_u64() else {
            return Err(InputValueError::custom("Expected an unsigned integer."));
        };

        if n > MAX_UINT53 {
            return Err(InputValueError::custom(
                "Value exceeds the maximum of UInt53 (2^53 - 1).",
            ));
        }

        Ok(UInt53(n))
    }

    fn to_value(&self) -> Value {
        Value::Number(self.0.into())
    }
}

impl fmt::Display for UInt53 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u64> for UInt53 {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value > MAX_UINT53 {
            return Err(Error::Internal(format!(
                "value {value} exceeds the maximum of UInt53 (2^53 - 1)"
            )));
        }
        Ok(Self(value))
    }
}

impl From<u32> for UInt53 {
    fn from(value: u32) -> Self {
        Self(value.into())
    }
}

impl From<UInt53> for Version {
    fn from(value: UInt53) -> Self {
        Version::from(value.0)
    }
}

impl From<UInt53> for BigInt<u64> {
    fn from(value: UInt53) -> Self {
        BigInt::from(value.0)
    }
}

impl From<UInt53> for u64 {
    fn from(value: UInt53) -> Self {
        value.0
    }
}

impl From<UInt53> for i64 {
    fn from(value: UInt53) -> Self {
        value.0 as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bounds() {
        assert_eq!(
            <UInt53 as ScalarType>::parse(Value::Number(MAX_UINT53.into())).unwrap(),
            UInt53(MAX_UINT53)
        );
        assert!(<UInt53 as ScalarType>::parse(Value::Number((MAX_UINT53 + 1).into())).is_err());
        assert!(<UInt53 as ScalarType>::parse(Value::Number((-1).into())).is_err());
        assert!(<UInt53 as ScalarType>::parse(Value::String("1".to_string())).is_err());
    }

    #[test]
    fn try_from_bounds() {
        assert_eq!(UInt53::try_from(MAX_UINT53).unwrap(), UInt53(MAX_UINT53));
        assert!(UInt53::try_from(MAX_UINT53 + 1).is_err());
    }
}
