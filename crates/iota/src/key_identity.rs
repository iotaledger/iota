// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Display, str::FromStr};

use anyhow::Error;
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_sdk::wallet_context::WalletContext;
use iota_types::base_types::IotaAddress;
use serde::Serialize;

/// An address or an alias associated with a key in the wallet
/// This is used to distinguish between an address or an alias,
/// enabling a user to use an alias for any command that requires an address.
#[derive(Serialize, Clone)]
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

/// Get the IotaAddress corresponding to this key identity.
/// If no string is provided, then the current active address is returned.
pub async fn get_identity_address(
    input: Option<KeyIdentity>,
    ctx: &WalletContext,
) -> Result<IotaAddress, Error> {
    if let Some(addr) = input {
        match addr {
            KeyIdentity::Address(x) => Ok(x),
            KeyIdentity::Alias(x) => Ok(*ctx.config().keystore().get_address_by_alias(x)?),
            #[cfg(feature = "iota-names")]
            KeyIdentity::Name(name) => {
                let client = ctx.get_client().await?;
                // Check alias first as it can override a name
                if let Ok(alias) = ctx
                    .config()
                    .keystore()
                    .get_address_by_alias(name.to_string())
                {
                    Ok(*alias)
                } else {
                    let entry = crate::name_commands::get_registry_entry(&name, &client).await?;
                    entry
                        .name_record
                        .target_address
                        .ok_or_else(|| anyhow::anyhow!("no target address set for {name}"))
                }
            }
        }
    } else {
        Ok(ctx.active_address()?)
    }
}

pub fn get_identity_address_from_keystore(
    input: KeyIdentity,
    keystore: &Keystore,
) -> Result<IotaAddress, Error> {
    match input {
        KeyIdentity::Address(x) => Ok(x),
        KeyIdentity::Alias(x) => Ok(*keystore.get_address_by_alias(x)?),
        #[cfg(feature = "iota-names")]
        KeyIdentity::Name(_) => anyhow::bail!("cannot fetch an IOTA Name from the keystore"),
    }
}

pub fn get_identity_alias_from_keystore(
    input: KeyIdentity,
    keystore: &Keystore,
) -> Result<String, Error> {
    match input {
        KeyIdentity::Address(x) => Ok(keystore.get_alias_by_address(&x)?),
        KeyIdentity::Alias(x) => Ok(x),
        #[cfg(feature = "iota-names")]
        KeyIdentity::Name(x) => Ok(x.to_string()),
    }
}
