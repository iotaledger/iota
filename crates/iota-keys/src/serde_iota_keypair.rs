// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::crypto::IotaKeyPair;
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(keypair: &IotaKeyPair, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let encoded = keypair.encode().map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&encoded)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<IotaKeyPair, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    IotaKeyPair::decode(&s).map_err(serde::de::Error::custom)
}
