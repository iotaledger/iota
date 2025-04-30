// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_names::registry::NameRecord;
use iota_types::base_types::{IotaAddress, ObjectID};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// A single record in the registry.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaNameRecord {
    /// The ID of the registration NFT assigned to this record.
    ///
    /// The owner of the corresponding NFT has the rights to
    /// be able to change and adjust the `target_address` of this domain.
    ///
    /// It is possible that the ID changes if the record expires and is
    /// purchased by someone else.
    pub nft_id: ObjectID,
    /// Timestamp in milliseconds when the record expires.
    pub expiration_timestamp_ms: u64,
    /// The target address that this domain points to
    pub target_address: Option<IotaAddress>,
    /// Additional data which may be stored in a record
    pub data: BTreeMap<String, String>,
}

impl From<NameRecord> for IotaNameRecord {
    fn from(record: NameRecord) -> Self {
        Self {
            nft_id: record.nft_id.bytes,
            expiration_timestamp_ms: record.expiration_timestamp_ms,
            target_address: record.target_address,
            data: record
                .data
                .contents
                .into_iter()
                .map(|entry| (entry.key.to_string(), entry.value.to_string()))
                .collect(),
        }
    }
}
