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
        Option::<String>::deserialize(deserializer)?
            .map(|s| super::deserialize(serde::de::IntoDeserializer::into_deserializer(s)))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bip32::DerivationPath;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct TestStruct {
        #[serde(with = "super")]
        path: DerivationPath,
        #[serde(
            with = "super::option",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        optional_path: Option<DerivationPath>,
    }

    #[test]
    fn test_serialize_deserialize_derivation_path() {
        let path_str = "m/44'/4218'/1'/2'/3'";
        let derivation_path = DerivationPath::from_str(path_str).unwrap();

        let other_path_str = "m/44'/4218'/4'/5'/6'";
        let other_derivation_path = DerivationPath::from_str(other_path_str).unwrap();

        let test_struct = TestStruct {
            path: derivation_path.clone(),
            optional_path: None,
        };

        // Serialize with optional None
        let serialized = serde_json::to_string(&test_struct).unwrap();
        let expected_json = format!(r#"{{"path":"{}"}}"#, path_str);
        assert_eq!(serialized, expected_json);

        // Deserialize
        let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.path, derivation_path);
        assert_eq!(deserialized.optional_path, None);

        // Test with optional_path having a value
        let test_struct_with_optional = TestStruct {
            path: derivation_path.clone(),
            optional_path: Some(other_derivation_path.clone()),
        };

        let serialized_with_optional = serde_json::to_string(&test_struct_with_optional).unwrap();
        // Should match the expected JSON with both path and optional_path
        let expected_json_with_optional = format!(
            r#"{{"path":"{}","optional_path":"{}"}}"#,
            path_str, other_path_str
        );
        assert_eq!(serialized_with_optional, expected_json_with_optional);

        let deserialized_with_optional: TestStruct =
            serde_json::from_str(&serialized_with_optional).unwrap();
        assert_eq!(deserialized_with_optional.path, derivation_path);
        assert_eq!(
            deserialized_with_optional.optional_path,
            Some(other_derivation_path)
        );
    }
}
