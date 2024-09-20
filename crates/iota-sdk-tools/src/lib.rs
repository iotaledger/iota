// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Provides tools to simplify usage of the IOTA SDK.

#![warn(missing_docs)]
#![deny(unreachable_pub)]

pub mod client;
pub mod query;
pub mod types;

use std::path::PathBuf;

use iota_keys::keystore::{FileBasedKeystore, Keystore};

pub use self::{
    client::{error::ClientError, Client},
    types::CustomMoveType,
};

/// The default keystore path. See [dirs::home_dir].
pub fn default_keystore_path() -> Result<PathBuf, ClientError> {
    Ok(dirs::home_dir()
        .ok_or_else(|| ClientError::MissingHomeDirectory)?
        .join(".iota")
        .join("iota_config")
        .join("iota.keystore"))
}

/// The default keystore, a [`FileBasedKeystore`] at the default path.
pub fn default_keystore() -> Result<Keystore, ClientError> {
    Ok(Keystore::File(FileBasedKeystore::new(
        &default_keystore_path()?,
    )?))
}
