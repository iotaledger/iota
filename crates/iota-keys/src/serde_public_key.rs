// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::crypto::{EncodeDecodeBase64, PublicKey};
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(pk: &PublicKey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let encoded = pk.encode_base64();
    serializer.serialize_str(&encoded)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<PublicKey, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    PublicKey::decode_base64(&s).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use fastcrypto::traits::EncodeDecodeBase64;
    use iota_types::crypto::PublicKey;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct TestStruct {
        #[serde(with = "super")]
        public_key: PublicKey,
    }

    #[test]
    fn test_serialize_deserialize_iota_keypair() {
        let public_key_str = "ADzNJA3sv4EWWqh9RZOZXYTaIEjqiDB9V+EaNeoSQJmt";
        let public_key = PublicKey::decode_base64(public_key_str).unwrap();

        let test_struct = TestStruct {
            public_key: public_key.clone(),
        };

        // Serialize
        let serialized = serde_json::to_string(&test_struct).unwrap();
        // Compare to expected JSON
        let expected_json = format!(r#"{{"public_key":"{public_key_str}"}}"#);
        assert_eq!(serialized, expected_json);

        // Deserialize
        let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.public_key, public_key);
    }
}
