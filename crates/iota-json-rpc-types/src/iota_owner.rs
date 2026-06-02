// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::{IotaAddress, SequenceNumber},
    object::Owner,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, SerializeAs, serde_as};

use crate::iota_primitives::{
    IotaAddress as IotaAddressSchema, SequenceNumberU64 as SequenceNumberU64Schema,
};

/// Enum of different types of ownership for an object.
///
/// # BCS
///
/// The BCS serialized form for this type is defined by the following ABNF:
///
/// ```text
/// owner = owner-address / owner-object / owner-shared / owner-immutable
///
/// owner-address   = %x00 address
/// owner-object    = %x01 object-id
/// owner-shared    = %x02 u64
/// owner-immutable = %x03
/// ```
#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename = "Owner")]
pub enum OwnerSchema {
    /// Object is exclusively owned by a single address, and is mutable.
    #[schemars(with = "IotaAddressSchema")]
    AddressOwner(#[serde_as(as = "IotaAddressSchema")] IotaAddress),
    /// Object is exclusively owned by a single object, and is mutable.
    /// The object ID is converted to IotaAddress as IotaAddress is
    /// universal.
    #[schemars(with = "IotaAddressSchema")]
    ObjectOwner(#[serde_as(as = "IotaAddressSchema")] IotaAddress),
    /// Object is shared, can be used by any address, and is mutable.
    Shared {
        /// The version at which the object became shared
        #[serde_as(as = "SequenceNumberU64Schema")]
        #[schemars(with = "SequenceNumberU64Schema")]
        initial_shared_version: SequenceNumber,
    },
    /// Object is immutable, and hence ownership doesn't matter.
    Immutable,
}

impl SerializeAs<Owner> for OwnerSchema {
    fn serialize_as<S>(source: &Owner, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        OwnerSchema::from(*source).serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, Owner> for OwnerSchema {
    fn deserialize_as<D>(deserializer: D) -> Result<Owner, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let iota_owner = OwnerSchema::deserialize(deserializer)?;
        Ok(Owner::from(iota_owner))
    }
}

impl std::fmt::Display for OwnerSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddressOwner(address) => {
                write!(f, "Account Address ( {address} )")
            }
            Self::ObjectOwner(address) => {
                write!(f, "Object ID: ( {address} )")
            }
            Self::Immutable => {
                write!(f, "Immutable")
            }
            Self::Shared {
                initial_shared_version,
            } => {
                write!(f, "Shared( {initial_shared_version} )")
            }
        }
    }
}

impl From<Owner> for OwnerSchema {
    fn from(value: Owner) -> Self {
        match value {
            Owner::Address(address) => OwnerSchema::AddressOwner(address),
            Owner::Object(object_id) => OwnerSchema::ObjectOwner(*object_id.as_address()),
            Owner::Shared(initial_shared_version) => OwnerSchema::Shared {
                initial_shared_version,
            },
            Owner::Immutable => OwnerSchema::Immutable,
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }
    }
}

impl From<OwnerSchema> for Owner {
    fn from(value: OwnerSchema) -> Self {
        match value {
            OwnerSchema::AddressOwner(address) => Owner::Address(address),
            OwnerSchema::ObjectOwner(address) => Owner::Object(ObjectId::from(address)),
            OwnerSchema::Shared {
                initial_shared_version,
            } => Owner::Shared(initial_shared_version),
            OwnerSchema::Immutable => Owner::Immutable,
        }
    }
}
