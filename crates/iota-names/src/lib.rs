// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod config;
pub mod domain;
pub mod error;
pub mod registry;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iota_types::{base_types::ObjectID, id::UID};
use serde::{Deserialize, Serialize};

use self::domain::Domain;

pub const MIN_LABEL_LEN: usize = 3;
pub const MAX_LABEL_LEN: usize = 63;

/// An object to manage a second-level domain (SLD)
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct IotaNamesRegistration {
    id: ObjectID,
    domain: Domain,
    domain_name: String,
    expiration_timestamp_ms: u64,
    image_url: String,
}

impl IotaNamesRegistration {
    pub fn id(&self) -> &ObjectID {
        &self.id
    }

    pub fn domain(&self) -> &Domain {
        &self.domain
    }

    pub fn domain_name(&self) -> &str {
        &self.domain_name
    }

    pub fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    pub fn expiration_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.expiration_timestamp_ms)
    }

    pub fn has_expired(&self) -> bool {
        self.expiration_time() <= SystemTime::now()
    }

    pub fn image_url(&self) -> &str {
        &self.image_url
    }
}

/// A SubDomainRegistration object to manage a subdomain.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubDomainRegistration {
    pub id: UID,
    pub nft: IotaNamesRegistration,
}
