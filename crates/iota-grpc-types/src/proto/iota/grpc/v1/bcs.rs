// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.bcs.rs");
include!("../../../generated/iota.grpc.v1.bcs.field_info.rs");
include!("../../../generated/iota.grpc.v1.bcs.accessors.rs");

impl BcsData {
    pub fn serialize<T: serde::Serialize>(value: &T) -> Result<Self, bcs::Error> {
        bcs::to_bytes(value).map(|bcs| Self { data: bcs.into() })
    }

    pub fn deserialize<'de, T: serde::Deserialize<'de>>(&'de self) -> Result<T, bcs::Error> {
        bcs::from_bytes(self.data.as_ref())
    }

    /// Get the raw BCS bytes as a slice.
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_ref()
    }
}

impl From<Vec<u8>> for BcsData {
    fn from(value: Vec<u8>) -> Self {
        Self { data: value.into() }
    }
}

impl From<&BcsData> for Vec<u8> {
    fn from(value: &BcsData) -> Self {
        value.data.to_vec()
    }
}

impl From<BcsData> for Vec<u8> {
    fn from(value: BcsData) -> Self {
        value.data.to_vec()
    }
}

impl From<prost::bytes::Bytes> for BcsData {
    fn from(value: prost::bytes::Bytes) -> Self {
        Self { data: value }
    }
}

impl From<&BcsData> for prost::bytes::Bytes {
    fn from(value: &BcsData) -> Self {
        value.data.clone()
    }
}

impl From<BcsData> for prost::bytes::Bytes {
    fn from(value: BcsData) -> Self {
        value.data
    }
}
