// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use serde::{Deserialize, Serialize};
use tokio::fs::{create_dir_all, read_to_string};
use url::Url;

const GENESIS_FILE_NAME: &str = "genesis.blob";
const CHECKPOINTS_FILE_NAME: &str = "checkpoints.yaml";

/// The config file for the light client.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    /// An RPC endpoint to a full node.
    pub rpc_url: Url,
    /// A GraphQL endpoint to a full node.
    pub graphql_url: Option<Url>,
    /// The directory containing synced checkpoints.
    pub checkpoints_dir: PathBuf,
    /// The URL to download the genesis.blob file from.
    pub genesis_blob_download_url: Option<Url>,
    /// Flag to enable automatic syncing before running one of the check
    /// commands.
    pub sync_before_check: bool,
    /// A config to sync the light client from a checkpoint store. If provided,
    /// will also be used to check objects/transactions for inclusion.
    pub checkpoint_store_config: Option<ObjectStoreConfig>,
    /// A config to sync the light client from an archive store. Since the
    /// archive does not store full checkpoints, it cannot be used to
    /// check objects/transactions.
    pub archive_store_config: Option<ObjectStoreConfig>,
}

impl Config {
    pub fn get_mainnet_config() -> Self {
        Self::get_config_by_network("mainnet")
    }

    pub fn get_testnet_config() -> Self {
        Self::get_config_by_network("testnet")
    }

    pub fn get_devnet_config() -> Self {
        Self::get_config_by_network("devnet")
    }

    fn get_config_by_network(network: &str) -> Self {
        Self {
            rpc_url: Url::parse(&format!("https://api.{network}.iota.cafe")).unwrap(),
            graphql_url: Some(Url::parse(&format!("https://graphql.{network}.iota.cafe")).unwrap()),
            checkpoints_dir: PathBuf::from_str(&format!("checkpoints_{network}")).unwrap(),
            genesis_blob_download_url: Some(
                Url::parse(&format!("https://dbfiles.{network}.iota.cafe/genesis.blob")).unwrap(),
            ),
            sync_before_check: false,
            checkpoint_store_config: Some(ObjectStoreConfig {
                object_store: Some(ObjectStoreType::S3),
                object_store_connection_limit: 20,
                aws_endpoint: Some(format!("https://checkpoints.{network}.iota.cafe")),
                aws_virtual_hosted_style_request: true,
                aws_region: Some("weur".to_string()),
                no_sign_request: true,
                ..Default::default()
            }),
            archive_store_config: Some(ObjectStoreConfig {
                object_store: Some(ObjectStoreType::S3),
                object_store_connection_limit: 20,
                aws_endpoint: Some(format!("https://archive.{network}.iota.cafe")),
                aws_virtual_hosted_style_request: true,
                aws_region: Some("weur".to_string()),
                no_sign_request: true,
                ..Default::default()
            }),
        }
    }
}

impl Config {
    /// Loads the config from file.
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = read_to_string(path).await?;
        let config: Config = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Creates the necessary checkpoint directory and files if not already
    /// present.
    pub async fn setup(&self) -> Result<()> {
        // Create the checkpoints directory if it doesn't exist yet
        if !self.checkpoints_dir.is_dir() {
            create_dir_all(&self.checkpoints_dir).await?;
        }
        // Download or copy the genesis blob if it doesn't exist yet
        if !self.genesis_blob_file_path().is_file() {
            if let Some(url) = &self.genesis_blob_download_url {
                match url.scheme() {
                    "file" => {
                        let path = url
                            .to_file_path()
                            .map_err(|_| anyhow!("invalid file path '{url}'"))?;
                        tokio::fs::copy(path, self.genesis_blob_file_path()).await?;
                    }
                    _ => {
                        let contents = reqwest::get(url.as_str()).await?.bytes().await?;
                        tokio::fs::write(self.genesis_blob_file_path(), contents).await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.graphql_url.is_none() && self.archive_store_config.is_none() {
            bail!("Invalid config: either GraphQL URL or archive store config must be provided");
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

    pub fn checkpoint_summary_file_path(&self, seq: u64) -> PathBuf {
        Path::new(&self.checkpoints_dir).join(format!("{seq}.sum"))
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
        let config = Config {
            rpc_url: "http://localhost:9000".parse().unwrap(),
            graphql_url: Some("http://localhost:9003".parse().unwrap()),
            checkpoints_dir: temp_dir.path().to_path_buf(),
            genesis_blob_download_url: None,
            sync_before_check: false,
            checkpoint_store_config: Some(ObjectStoreConfig {
                object_store: Some(ObjectStoreType::S3),
                aws_endpoint: Some("http://localhost:9001".to_string()),
                ..Default::default()
            }),
            archive_store_config: Some(ObjectStoreConfig {
                object_store: Some(ObjectStoreType::File),
                directory: Some(temp_dir.path().to_path_buf()),
                ..Default::default()
            }),
        };
        config.validate().expect("invalid");
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
