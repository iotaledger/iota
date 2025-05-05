// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod config;
pub mod constants;
pub mod domain;
pub mod error;
pub mod registry;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iota_types::base_types::ObjectID;
use move_core_types::{
    account_address::AccountAddress, ident_str, identifier::IdentStr, language_storage::StructTag,
};
use serde::{Deserialize, Serialize};

use self::domain::Domain;

/// An object to manage a second-level domain (SLD).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct IotaNamesRegistration {
    id: ObjectID,
    domain: Domain,
    domain_name: String,
    expiration_timestamp_ms: u64,
}

/// An object to manage a subdomain.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubdomainRegistration {
    id: ObjectID,
    nft: IotaNamesRegistration,
}

impl SubdomainRegistration {
    pub fn into_inner(self) -> IotaNamesRegistration {
        self.nft
    }
}

/// Unifying trait for [`IotaNamesRegistration`] and [`SubdomainRegistration`]
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

    fn domain(&self) -> &Domain;

    fn domain_name(&self) -> &str;

    fn expiration_timestamp_ms(&self) -> u64;

    fn expiration_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.expiration_timestamp_ms())
    }

    fn has_expired(&self) -> bool {
        self.expiration_time() <= SystemTime::now()
    }

    fn id(&self) -> ObjectID;
}

impl IotaNamesNft for IotaNamesRegistration {
    const MODULE: &IdentStr = ident_str!("iota_names_registration");
    const TYPE_NAME: &IdentStr = ident_str!("IotaNamesRegistration");

    fn domain(&self) -> &Domain {
        &self.domain
    }

    fn domain_name(&self) -> &str {
        &self.domain_name
    }

    fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    fn id(&self) -> ObjectID {
        self.id
    }
}

impl IotaNamesNft for SubdomainRegistration {
    const MODULE: &IdentStr = ident_str!("subdomain_registration");
    const TYPE_NAME: &IdentStr = ident_str!("SubdomainRegistration");

    fn domain(&self) -> &Domain {
        self.nft.domain()
    }

    fn domain_name(&self) -> &str {
        self.nft.domain_name()
    }

    fn expiration_timestamp_ms(&self) -> u64 {
        self.nft.expiration_timestamp_ms()
    }

    fn id(&self) -> ObjectID {
        self.id
    }
}
