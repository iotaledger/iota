// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Serialize,Deserialize};
use crate::AuthenticatorKind;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountState {
    pub created_at_unix_ms: u128,
    pub rpc: String,

    pub sender: String,
    pub publish_tx_digest: String,
    pub package_id: String,
    pub package_metadata_object_id: String,

    pub aa_account_object_id: String,
    pub aa_account_version: u64,
    pub aa_account_digest: String,
    pub aa_address: String,

    pub authenticator: AuthenticatorKind,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RegistryState {
    pub active_account: Option<String>,

   pub accounts: std::collections::BTreeMap<String, AccountState>,
}

pub fn load_registry(path: &PathBuf) -> Result<RegistryState> {
    if !path.exists() {
        return Ok(RegistryState::default());
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

pub fn save_registry(path: &PathBuf, st: &RegistryState) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(st)?)?;
    Ok(())
}

