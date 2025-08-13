// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod string {
    use alloc::string::String;
    use core::{fmt::Display, str::FromStr};

    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

pub mod option_string {
    use alloc::string::String;
    use core::{fmt::Display, str::FromStr};

    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<T, S>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        match value {
            Some(value) => serializer.collect_str(value),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|string| string.parse().map_err(de::Error::custom))
            .transpose()
    }
}

pub mod prefix_hex_bytes {
    use alloc::string::String;

    use prefix_hex::{FromHexPrefixed, ToHexPrefixed};
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        for<'a> &'a T: ToHexPrefixed,
    {
        serializer.serialize_str(&prefix_hex::encode(value))
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromHexPrefixed,
    {
        prefix_hex::decode(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

pub mod option_prefix_hex_bytes {
    use alloc::string::String;

    use prefix_hex::{FromHexPrefixed, ToHexPrefixed};
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S, T>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        for<'a> &'a T: ToHexPrefixed,
    {
        match value {
            Some(bytes) => super::prefix_hex_bytes::serialize(bytes, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: FromHexPrefixed,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|string| prefix_hex::decode(string).map_err(de::Error::custom))
            .transpose()
    }
}

pub mod string_prefix {
    use alloc::string::String;

    use packable::{bounded::Bounded, prefix::StringPrefix};
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<T: Bounded, S>(
        value: &StringPrefix<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&**value)
    }

    pub fn deserialize<'de, T: Bounded, D>(deserializer: D) -> Result<StringPrefix<T>, D::Error>
    where
        D: Deserializer<'de>,
        <T as TryFrom<usize>>::Error: core::fmt::Display,
    {
        String::deserialize(deserializer)
            .map_err(de::Error::custom)
            .and_then(|s| s.try_into().map_err(de::Error::custom))
    }
}
