// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod validator_peer;
pub mod validator_v2;

use bytes::Bytes;
use iota_types::error::IotaError;
use serde::{Serialize, de::DeserializeOwned};

pub(crate) fn bcs_serialize<T: Serialize>(value: &T, type_info: &str) -> Result<Bytes, IotaError> {
    bcs::to_bytes(value)
        .map(Into::into)
        .map_err(|e| IotaError::TransactionSerialization {
            error: format!("{type_info}: {e}"),
        })
}

pub(crate) fn bcs_deserialize<T: DeserializeOwned>(
    bytes: &[u8],
    type_info: &str,
) -> Result<T, IotaError> {
    bcs::from_bytes(bytes).map_err(|e| IotaError::TransactionSerialization {
        error: format!("{type_info}: {e}"),
    })
}
