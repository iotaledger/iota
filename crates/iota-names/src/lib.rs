// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod config;
pub mod constants;
pub mod error;
pub mod name;
pub mod registry;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iota_sdk_types::{Identifier, ObjectId, StructTag};
use iota_types::base_types::IotaAddress;
use serde::{Deserialize, Serialize};

use self::name::Name;

/// An object to manage a second-level name (SLN).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NameRegistration {
    id: ObjectId,
    name: Name,
    name_str: String,
    expiration_timestamp_ms: u64,
}

/// An object to manage a subname.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubnameRegistration {
    id: ObjectId,
    nft: NameRegistration,
}

impl SubnameRegistration {
    pub fn into_inner(self) -> NameRegistration {
        self.nft
    }
}

/// Unifying trait for [`NameRegistration`] and [`SubnameRegistration`]
pub trait IotaNamesNft {
    const MODULE: Identifier;
    const TYPE_NAME: Identifier;

    fn type_(package_id: IotaAddress) -> StructTag {
        StructTag::new(package_id, Self::MODULE, Self::TYPE_NAME, vec![])
    }

    fn name(&self) -> &Name;

    fn name_str(&self) -> &str;

    fn expiration_timestamp_ms(&self) -> u64;

    fn expiration_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.expiration_timestamp_ms())
    }

    fn has_expired(&self) -> bool {
        self.expiration_time() <= SystemTime::now()
    }

    fn id(&self) -> ObjectId;
}

impl IotaNamesNft for NameRegistration {
    const MODULE: Identifier = Identifier::from_static("name_registration");
    const TYPE_NAME: Identifier = Identifier::from_static("NameRegistration");

    fn name(&self) -> &Name {
        &self.name
    }

    fn name_str(&self) -> &str {
        &self.name_str
    }

    fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    fn id(&self) -> ObjectId {
        self.id
    }
}

impl IotaNamesNft for SubnameRegistration {
    const MODULE: Identifier = Identifier::from_static("subname_registration");
    const TYPE_NAME: Identifier = Identifier::from_static("SubnameRegistration");

    fn name(&self) -> &Name {
        self.nft.name()
    }

    fn name_str(&self) -> &str {
        self.nft.name_str()
    }

    fn expiration_timestamp_ms(&self) -> u64 {
        self.nft.expiration_timestamp_ms()
    }

    fn id(&self) -> ObjectId {
        self.id
    }
}
