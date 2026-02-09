// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::cli::AuthenticatorKind;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeploymentState {
    pub created_at_unix_ms: u128,
    pub rpc: String,
    pub package_path: String,
    pub publish_tx_digest: String,

    pub package_id: String,
    pub package_metadata_object_id: String,
    pub package_metadata_version: u64,
    pub package_metadata_digest: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountState {
    pub created_at_unix_ms: u128,
    pub rpc: String,
    pub sender: String,

    #[serde(default)]
    pub deployment: Option<String>,

    pub publish_tx_digest: String,
    pub package_id: String,
    pub package_metadata_object_id: String,
    pub aa_account_object_id: String,
    pub aa_account_version: u64,
    pub aa_account_digest: String,
    pub aa_address: String,
    pub authenticator: AuthenticatorKind,

    pub bench_objects: Vec<StoredObjectRef>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RegistryState {
    pub active_account: Option<String>,
    pub accounts: std::collections::BTreeMap<String, AccountState>,
    /// deployment_name -> deployment data
    #[serde(default)]
    pub deployments: std::collections::BTreeMap<String, DeploymentState>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredObjectRef {
    pub object_id: String,
    pub version: u64,
    pub digest: String,
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

impl StoredObjectRef {
    pub fn from_object_ref(r: iota_types::base_types::ObjectRef) -> Self {
        Self {
            object_id: r.0.to_string(),
            version: r.1.value(),
            digest: r.2.to_string(),
        }
    }

    pub fn to_object_ref(&self) -> anyhow::Result<iota_types::base_types::ObjectRef> {
        use iota_types::{
            base_types::{ObjectID, SequenceNumber},
            digests::ObjectDigest,
        };

        Ok((
            self.object_id.parse::<ObjectID>()?,
            SequenceNumber::from_u64(self.version),
            self.digest.parse::<ObjectDigest>()?,
        ))
    }
}
