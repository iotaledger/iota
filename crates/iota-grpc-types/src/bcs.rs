// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::v0::common as grpc_common;

impl grpc_common::BcsData {
    pub fn serialize_from<T>(data: &T) -> Result<Self, bcs::Error>
    where
        T: Serialize,
    {
        let serialized = bcs::to_bytes(data)?;
        Ok(grpc_common::BcsData { data: serialized })
    }

    pub fn deserialize_into<T>(&self) -> Result<T, bcs::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        bcs::from_bytes(&self.data)
    }
}
