// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Display, str::FromStr};

use iota_types::base_types::IotaAddress;
use serde::Serialize;

/// An address or an alias associated with a key in the wallet.
/// This is used to distinguish between an address or an alias,
/// enabling a user to use an alias for any command that requires an address.
/// When `iota-names` feature is enabled, it can also be a name.
#[derive(Debug, Serialize, Clone)]
pub enum KeyIdentity {
    Address(IotaAddress),
    Alias(String),
    #[cfg(feature = "iota-names")]
    Name(iota_names::name::Name),
}

impl From<IotaAddress> for KeyIdentity {
    fn from(address: IotaAddress) -> Self {
        Self::Address(address)
    }
}

impl FromStr for KeyIdentity {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(address) = s.parse() {
            Ok(KeyIdentity::Address(address))
        } else {
            #[cfg(feature = "iota-names")]
            if let Ok(name) = s.parse() {
                return Ok(KeyIdentity::Name(name));
            }
            Ok(KeyIdentity::Alias(s.to_string()))
        }
    }
}

impl Display for KeyIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = match self {
            KeyIdentity::Address(x) => x.to_string(),
            KeyIdentity::Alias(x) => x.to_string(),
            #[cfg(feature = "iota-names")]
            KeyIdentity::Name(x) => x.to_string(),
        };
        write!(f, "{v}")
    }
}
