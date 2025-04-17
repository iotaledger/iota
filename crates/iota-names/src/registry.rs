// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use iota_types::{
    base_types::{IotaAddress, ObjectID},
    collection_types::VecMap,
    dynamic_field::Field,
    id::ID,
    object::{MoveObject, Object},
};
use serde::{Deserialize, Serialize};

use crate::{config::LEAF_EXPIRATION_TIMESTAMP, domain::Domain, error::IotaNamesError};

/// Rust version of the Move iota::table::Table type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Table<K, V> {
    pub id: ObjectID,
    pub size: u64,

    // TODO: remove these fields and <K, V> if they aren't needed for the indexer
    #[serde(skip)]
    _key: PhantomData<K>,
    #[serde(skip)]
    _value: PhantomData<V>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Registry {
    /// The `registry` table maps `Domain` to `NameRecord`.
    /// Added / replaced in the `add_record` function.
    registry: Table<Domain, NameRecord>,
    /// The `reverse_registry` table maps `address` to `domain_name`.
    /// Updated in the `set_reverse_lookup` function.
    reverse_registry: Table<IotaAddress, Domain>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryEntry {
    pub id: ObjectID,
    pub domain: Domain,
    pub name_record: NameRecord,
}

#[derive(Debug, Deserialize)]
pub struct ReverseRegistryEntry {
    pub id: ObjectID,
    pub address: IotaAddress,
    pub domain: Domain,
}

/// A single record in the registry.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct NameRecord {
    /// The ID of the registration NFT assigned to this record.
    ///
    /// The owner of the corresponding registration NFT has the rights to
    /// be able to change and adjust the `target_address` of this domain.
    ///
    /// It is possible that the ID changes if the record expires and is
    /// purchased by someone else.
    pub nft_id: ID,
    /// Timestamp in milliseconds when the record expires.
    pub expiration_timestamp_ms: u64,
    /// The target address that this domain points to
    pub target_address: Option<IotaAddress>,
    /// Additional data which may be stored in a record
    pub data: VecMap<String, String>,
}

impl TryFrom<Object> for NameRecord {
    type Error = IotaNamesError;

    fn try_from(object: Object) -> Result<Self, IotaNamesError> {
        object
            .to_rust::<Field<Domain, Self>>()
            .map(|record| record.value)
            .ok_or_else(|| IotaNamesError::MalformedObject(object.id()))
    }
}

impl TryFrom<MoveObject> for NameRecord {
    type Error = IotaNamesError;

    fn try_from(object: MoveObject) -> Result<Self, IotaNamesError> {
        object
            .to_rust::<Field<Domain, Self>>()
            .map(|record| record.value)
            .ok_or_else(|| IotaNamesError::MalformedObject(object.id()))
    }
}

impl NameRecord {
    /// Leaf records expire when their parent expires.
    /// The `expiration_timestamp_ms` is set to `0` (on-chain) to indicate this.
    pub fn is_leaf_record(&self) -> bool {
        self.expiration_timestamp_ms == LEAF_EXPIRATION_TIMESTAMP
    }

    /// Validate that a `NameRecord` is a valid parent of a child `NameRecord`.
    ///
    /// WARNING: This only applies for `leaf` records
    pub fn is_valid_leaf_parent(&self, child: &NameRecord) -> bool {
        self.nft_id == child.nft_id
    }

    /// Checks if a `node` name record has expired.
    /// Expects the latest checkpoint's timestamp.
    pub fn is_node_expired(&self, checkpoint_timestamp_ms: u64) -> bool {
        self.expiration_timestamp_ms < checkpoint_timestamp_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expirations() {
        let system_time: u64 = 100;

        let mut name = NameRecord {
            nft_id: iota_types::id::ID::new(ObjectID::random()),
            data: VecMap { contents: vec![] },
            target_address: Some(IotaAddress::random_for_testing_only()),
            expiration_timestamp_ms: system_time + 10,
        };

        assert!(!name.is_node_expired(system_time));

        name.expiration_timestamp_ms = system_time - 10;

        assert!(name.is_node_expired(system_time));
    }
}
