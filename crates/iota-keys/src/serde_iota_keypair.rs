// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_ext::crypto::{ToFromBech32, simple::SimpleKeypair};
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(keypair: &SimpleKeypair, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let encoded = keypair.to_bech32().map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&encoded)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<SimpleKeypair, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    SimpleKeypair::from_bech32(&s).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use iota_sdk_ext::crypto::{ToFromBech32, simple::SimpleKeypair};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct TestStruct {
        #[serde(with = "super")]
        keypair: SimpleKeypair,
    }

    #[test]
    fn test_serialize_deserialize_iota_keypair() {
        let private_key_str =
            "iotaprivkey1qzcw0258grcdka33dj52vxhg36hadad9clcmt4h42z9wr5sha6dl7j4pw06";
        let keypair = SimpleKeypair::from_bech32(private_key_str).unwrap();

        let test_struct = TestStruct {
            keypair: keypair.clone(),
        };

        // Serialize
        let serialized = serde_json::to_string(&test_struct).unwrap();
        // Compare to expected JSON
        let expected_json = format!(r#"{{"keypair":"{private_key_str}"}}"#);
        assert_eq!(serialized, expected_json);

        // Deserialize
        let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.keypair.to_bytes(), keypair.to_bytes());
    }
}
