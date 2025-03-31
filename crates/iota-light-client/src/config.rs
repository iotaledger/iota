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

/// The config file for the light client.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    /// The directory containing synced full checkpoints and checkpoint
    /// summaries.
    pub checkpoints_sync_dir: PathBuf,
    pub genesis_filename: String,
    pub full_node_url: String,
    pub graphql_url: Option<String>,
    pub object_store_url: Option<String>,
    pub archive_store_config: Option<ObjectStoreConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            checkpoints_sync_dir: std::env::current_dir()
                .expect("error getting current directory")
                .join("checkpoints_localnet"),
            full_node_url: "http://localhost:9000".to_string(),
            object_store_url: None,
            archive_store_config: None,
            graphql_url: Some("http://localhost:9125".to_string()),
            genesis_filename: "genesis.blob".to_string(),
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
        Url::parse(&self.full_node_url).map_err(|_| anyhow!("Invalid full node URL"))?;
        if !self.checkpoints_sync_dir.is_dir() {
            bail!("Checkpoint directory does not exist");
        }
        if let Some(url) = &self.object_store_url {
            Url::parse(url).map_err(|_| anyhow!("Invalid object store URL"))?;
        }
        if let Some(url) = &self.graphql_url {
            Url::parse(url).map_err(|_| anyhow!("Invalid GraphQL URL"))?;
        }
        if !self.checkpoint_list_path().is_file() {
            bail!(
                "Checkpoint list file is missing at {}",
                self.checkpoint_list_path().display()
            );
        }
        if !self.genesis_file_path().is_file() {
            bail!(
                "Genesis file is missing at {}",
                self.genesis_file_path().display()
            );
        }
        Ok(())
    }

    pub fn checkpoint_list_path(&self) -> PathBuf {
        self.checkpoints_sync_dir.join("checkpoints.yaml")
    }

    pub fn genesis_file_path(&self) -> PathBuf {
        self.checkpoints_sync_dir.join(&self.genesis_filename)
    }

    pub fn checkpoint_path<'a>(
        &self,
        seq: u64,
        custom_path: impl Into<Option<&'a str>>,
    ) -> PathBuf {
        let mut path = self.checkpoints_sync_dir.clone();
        if let Some(custom) = custom_path.into() {
            path.push(custom);
        }
        path.push(format!("{}.yaml", seq));
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
            checkpoints_sync_dir: temp_dir.path().to_path_buf(),
            full_node_url: "http://localhost:9000".to_string(),
            object_store_url: Some("http://localhost:9001".to_string()),
            archive_store_config: Some(ObjectStoreConfig {
                object_store: Some(ObjectStoreType::File),
                directory: Some(temp_dir.path().to_path_buf()),
                ..Default::default()
            }),
            graphql_url: Some("http://localhost:9003".to_string()),
            genesis_filename: "genesis.blob".to_string(),
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

        let list_path = config.checkpoint_list_path();
        assert_eq!(list_path.file_name().unwrap(), "checkpoints.yaml");

        let checkpoint_path = config.checkpoint_path(123, None);
        assert_eq!(checkpoint_path.file_name().unwrap(), "123.yaml");

        let custom_checkpoint_path = config.checkpoint_path(456, Some("custom"));
        assert!(custom_checkpoint_path.to_str().unwrap().contains("custom"));
        assert_eq!(custom_checkpoint_path.file_name().unwrap(), "456.yaml");
    }

    #[test]
    fn test_genesis_path() {
        let (config, _temp_dir) = create_test_config();
        let genesis_path = config.genesis_file_path();
        assert_eq!(genesis_path.file_name().unwrap(), "genesis.blob");
    }
}
