// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use iota_config::object_storage_config::ObjectStoreConfig;
use serde::{Deserialize, Serialize};
use url::Url;

const GENESIS_FILE_NAME: &str = "genesis.blob";
const CHECKPOINTS_FILE_NAME: &str = "checkpoints.yaml";

/// The config file for the light client.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    /// An RPC endpoint to a full node.
    pub rpc_url: String,
    /// A GraphQL endpoint to a full node.
    pub graphql_url: Option<String>,
    /// The directory containing synced checkpoints.
    pub checkpoints_dir: PathBuf,
    /// Flag to enable automatic syncing before running one of the check
    /// commands.
    pub sync_before_check: bool,
    /// A URL to an object store storing checkpoint summaries.
    pub object_store_url: Option<String>,
    /// An config to sync the light client from an archive store.
    pub archive_store_config: Option<ObjectStoreConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:9000".to_string(),
            graphql_url: Some("http://localhost:9125".to_string()),
            checkpoints_dir: std::env::current_dir()
                .expect("error getting current directory")
                .join("checkpoints_localnet"),
            sync_before_check: false,
            object_store_url: None,
            archive_store_config: None,
        }
    }
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = fs::File::open(path)?;
        let config: Config = serde_yaml::from_reader(file)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        Url::parse(&self.rpc_url).map_err(|_| anyhow!("Invalid full node URL"))?;
        if !self.checkpoints_dir.is_dir() {
            bail!("Checkpoint directory does not exist");
        }
        if let Some(url) = &self.object_store_url {
            Url::parse(url).map_err(|_| anyhow!("Invalid object store URL"))?;
        }
        if let Some(url) = &self.graphql_url {
            Url::parse(url).map_err(|_| anyhow!("Invalid GraphQL URL"))?;
        }
        if !self.checkpoints_list_file_path().is_file() {
            bail!(
                "Sync file is missing at {}",
                self.checkpoints_list_file_path().display()
            );
        }
        if !self.genesis_blob_file_path().is_file() {
            bail!(
                "Genesis file is missing at {}",
                self.genesis_blob_file_path().display()
            );
        }
        Ok(())
    }

    pub fn checkpoints_list_file_path(&self) -> PathBuf {
        self.checkpoints_dir.join(CHECKPOINTS_FILE_NAME)
    }

    pub fn genesis_blob_file_path(&self) -> PathBuf {
        self.checkpoints_dir.join(GENESIS_FILE_NAME)
    }

    pub fn full_checkpoint_file_path<'a>(
        &self,
        seq: u64,
        custom_path: impl Into<Option<&'a str>>,
    ) -> PathBuf {
        let mut path = self.checkpoints_dir.clone();
        if let Some(custom) = custom_path.into() {
            path.push(custom);
        }
        path.push(format!("{seq}.chk"));
        path
    }

    pub fn checkpoint_summary_file_path<'a>(
        &self,
        seq: u64,
        custom_path: impl Into<Option<&'a str>>,
    ) -> PathBuf {
        let mut path = self.checkpoints_dir.clone();
        if let Some(custom) = custom_path.into() {
            path.push(custom);
        }
        path.push(format!("{seq}.sum"));
        path
    }
}

#[cfg(test)]
mod tests {
    use iota_config::object_storage_config::ObjectStoreType;
    use tempfile::TempDir;

    use super::*;

    fn create_test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        std::fs::File::create(temp_dir.path().join("genesis.blob")).unwrap();
        std::fs::File::create(temp_dir.path().join("checkpoints.yaml")).unwrap();
        let config = Config {
            checkpoints_dir: temp_dir.path().to_path_buf(),
            rpc_url: "http://localhost:9000".to_string(),
            object_store_url: Some("http://localhost:9001".to_string()),
            archive_store_config: Some(ObjectStoreConfig {
                object_store: Some(ObjectStoreType::File),
                directory: Some(temp_dir.path().to_path_buf()),
                ..Default::default()
            }),
            graphql_url: Some("http://localhost:9003".to_string()),
            sync_before_check: false,
        };
        (config, temp_dir)
    }

    #[test]
    fn test_config_validation() {
        let (config, _temp_dir) = create_test_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_checkpoint_paths() {
        let (config, _temp_dir) = create_test_config();

        let list_path = config.checkpoints_list_file_path();
        assert_eq!(list_path.file_name().unwrap(), "checkpoints.yaml");

        let checkpoint_path = config.full_checkpoint_file_path(123, None);
        assert_eq!(checkpoint_path.file_name().unwrap(), "123.chk");

        let custom_checkpoint_path = config.full_checkpoint_file_path(456, Some("custom"));
        assert!(custom_checkpoint_path.to_str().unwrap().contains("custom"));
        assert_eq!(custom_checkpoint_path.file_name().unwrap(), "456.chk");
    }

    #[test]
    fn test_genesis_path() {
        let (config, _temp_dir) = create_test_config();
        let genesis_path = config.genesis_blob_file_path();
        assert_eq!(genesis_path.file_name().unwrap(), "genesis.blob");
    }
}
