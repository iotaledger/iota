// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{fs::File, io::BufReader, path::PathBuf, str::FromStr};

use http::Uri;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::log::warn;

use crate::types::ReplayEngineError;

pub const DEFAULT_CONFIG_PATH: &str = "~/.sui-replay/network-config.yaml";

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ReplayableNetworkConfigSet {
    #[serde(skip)]
    path: Option<PathBuf>,
    #[serde(default)]
    pub base_network_configs: Vec<ReplayableNetworkBaseConfig>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ReplayableNetworkBaseConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub epoch_zero_start_timestamp: u64,
    #[serde(default)]
    pub epoch_zero_rgp: u64,
    #[serde(default = "default_full_node_address")]
    pub public_full_node: String,
}

impl ReplayableNetworkConfigSet {
    pub fn load_config(override_path: Option<String>) -> Result<Self, ReplayEngineError> {
        let path = override_path.unwrap_or_else(|| {
            warn!(
                "No network config path specified, using default path: {}",
                DEFAULT_CONFIG_PATH
            );
            DEFAULT_CONFIG_PATH.to_string()
        });
        let path = shellexpand::tilde(&path).to_string();
        let path = PathBuf::from_str(&path).unwrap();
        ReplayableNetworkConfigSet::from_file(path.clone()).map_err(|err| {
            ReplayEngineError::UnableToOpenYamlFile {
                path: path.as_os_str().to_string_lossy().to_string(),
                err: err.to_string(),
            }
        })
    }

    pub fn save_config(&self, override_path: Option<String>) -> Result<PathBuf, ReplayEngineError> {
        let path = override_path.unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());
        let path = shellexpand::tilde(&path).to_string();
        let path = PathBuf::from_str(&path).unwrap();
        self.to_file(path.clone())
            .map_err(|err| ReplayEngineError::UnableToOpenYamlFile {
                path: path.as_os_str().to_string_lossy().to_string(),
                err: err.to_string(),
            })?;
        Ok(path)
    }

    pub fn from_file(path: PathBuf) -> Result<Self, ReplayEngineError> {
        let file =
            File::open(path.clone()).map_err(|err| ReplayEngineError::UnableToOpenYamlFile {
                path: path.as_os_str().to_string_lossy().to_string(),
                err: err.to_string(),
            })?;
        let reader = BufReader::new(file);
        let mut config: ReplayableNetworkConfigSet =
            serde_yaml::from_reader(reader).map_err(|err| {
                ReplayEngineError::UnableToOpenYamlFile {
                    path: path.as_os_str().to_string_lossy().to_string(),
                    err: err.to_string(),
                }
            })?;
        config.path = Some(path);
        Ok(config)
    }

    pub fn to_file(&self, path: PathBuf) -> Result<(), ReplayEngineError> {
        let prefix = path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();
        let file =
            File::create(path.clone()).map_err(|err| ReplayEngineError::UnableToOpenYamlFile {
                path: path.as_os_str().to_string_lossy().to_string(),
                err: err.to_string(),
            })?;
        serde_yaml::to_writer(file, self).map_err(|err| {
            ReplayEngineError::UnableToWriteYamlFile {
                path: path.as_os_str().to_string_lossy().to_string(),
                err: err.to_string(),
            }
        })?;
        Ok(())
    }
}

impl Default for ReplayableNetworkConfigSet {
    fn default() -> Self {
        let testnet = ReplayableNetworkBaseConfig {
            name: "testnet".to_string(),
            epoch_zero_start_timestamp: 0,
            epoch_zero_rgp: 0,
            public_full_node: url_from_str("https://fullnode.testnet.sui.io:443")
                .expect("invalid socket address")
                .to_string(),
        };
        let devnet = ReplayableNetworkBaseConfig {
            name: "devnet".to_string(),
            epoch_zero_start_timestamp: 0,
            epoch_zero_rgp: 0,
            public_full_node: url_from_str("https://fullnode.devnet.sui.io:443")
                .expect("invalid socket address")
                .to_string(),
        };
        let mainnet = ReplayableNetworkBaseConfig {
            name: "mainnet".to_string(),
            epoch_zero_start_timestamp: 0,
            epoch_zero_rgp: 0,
            public_full_node: url_from_str("https://fullnode.mainnet.sui.io:443")
                .expect("invalid socket address")
                .to_string(),
        };

        Self {
            path: None,
            base_network_configs: vec![testnet, devnet, mainnet],
        }
    }
}

pub fn default_full_node_address() -> String {
    // Assume local node
    "0.0.0.0:9000".to_string()
}

pub fn url_from_str(s: &str) -> Result<Uri, ReplayEngineError> {
    Uri::from_str(s).map_err(|e| ReplayEngineError::InvalidUrl {
        err: e.to_string(),
        url: s.to_string(),
    })
}

#[test]
fn test_yaml() {
    let mut set = ReplayableNetworkConfigSet::default();

    let path = tempfile::tempdir().unwrap().path().to_path_buf();
    let path_str = path.to_str().unwrap().to_owned();

    let final_path = set.save_config(Some(path_str.clone())).unwrap();

    // Read from file
    let data = ReplayableNetworkConfigSet::load_config(Some(path_str)).unwrap();
    set.path = Some(final_path);
    assert!(set == data);
}
