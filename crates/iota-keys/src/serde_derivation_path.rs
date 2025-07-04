// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use bip32::DerivationPath;
use serde::{Deserialize, Deserializer, Serializer};

/// Serde support for `DerivationPath` as `"m/44'/0'/0'"` string.
pub fn serialize<S>(value: &DerivationPath, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

/// Deserialize a string into `DerivationPath`.
#[expect(dead_code)]
pub fn deserialize<'de, D>(deserializer: D) -> Result<DerivationPath, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    DerivationPath::from_str(&s).map_err(serde::de::Error::custom)
}

/// Serde support for `Option<DerivationPath>`, treating `None` as null or
/// missing, and serializing `Some` like a normal derivation path string.
pub mod option {
    use super::*;

    pub fn serialize<S>(maybe: &Option<DerivationPath>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match maybe {
            Some(path) => super::serialize(path, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DerivationPath>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            Some(s) => Ok(Some(
                DerivationPath::from_str(&s).map_err(serde::de::Error::custom)?,
            )),
            None => Ok(None),
        }
    }
}
