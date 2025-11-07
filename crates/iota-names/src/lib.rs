// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod config;
pub mod constants;
pub mod error;
pub mod name;
pub mod registry;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iota_types::base_types::ObjectID;
use move_core_types::{
    account_address::AccountAddress, ident_str, identifier::IdentStr, language_storage::StructTag,
};
use serde::{Deserialize, Serialize};

use self::name::Name;

/// An object to manage a second-level name (SLN).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NameRegistration {
    id: ObjectID,
    name: Name,
    name_str: String,
    expiration_timestamp_ms: u64,
}

/// An object to manage a subname.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubnameRegistration {
    id: ObjectID,
    nft: NameRegistration,
}

impl SubnameRegistration {
    pub fn into_inner(self) -> NameRegistration {
        self.nft
    }
}

/// Unifying trait for [`NameRegistration`] and [`SubnameRegistration`]
pub trait IotaNamesNft {
    const MODULE: &IdentStr;
    const TYPE_NAME: &IdentStr;

    fn type_(package_id: AccountAddress) -> StructTag {
        StructTag {
            address: package_id,
            module: Self::MODULE.into(),
            name: Self::TYPE_NAME.into(),
            type_params: Vec::new(),
        }
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

    fn id(&self) -> ObjectID;
}

impl IotaNamesNft for NameRegistration {
    const MODULE: &IdentStr = ident_str!("name_registration");
    const TYPE_NAME: &IdentStr = ident_str!("NameRegistration");

    fn name(&self) -> &Name {
        &self.name
    }

    fn name_str(&self) -> &str {
        &self.name_str
    }

    fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    fn id(&self) -> ObjectID {
        self.id
    }
}

impl IotaNamesNft for SubnameRegistration {
    const MODULE: &IdentStr = ident_str!("subname_registration");
    const TYPE_NAME: &IdentStr = ident_str!("SubnameRegistration");

    fn name(&self) -> &Name {
        self.nft.name()
    }

    fn name_str(&self) -> &str {
        self.nft.name_str()
    }

    fn expiration_timestamp_ms(&self) -> u64 {
        self.nft.expiration_timestamp_ms()
    }

    fn id(&self) -> ObjectID {
        self.id
    }
}
