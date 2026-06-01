// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]
use anyhow::{Context, Result};
#[cfg(not(target_arch = "wasm32"))]
use serde::{Serialize, de::DeserializeOwned};
#[cfg(not(target_arch = "wasm32"))]
use tracing::trace;

#[cfg(not(target_arch = "wasm32"))]
pub mod certificate_deny_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod genesis;
#[cfg(not(target_arch = "wasm32"))]
pub mod local_ip_utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod migration_tx_data;
#[cfg(not(target_arch = "wasm32"))]
pub mod node;
#[cfg(not(target_arch = "wasm32"))]
pub mod node_config_metrics;
#[cfg(not(target_arch = "wasm32"))]
pub mod object_storage_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod p2p;
// `transaction_deny_config` + `verifier_signing_config` are the two modules
// the execution path needs at sign-time. Everything else pulls in network /
// filesystem / config-file machinery and is gated.
pub mod transaction_deny_config;
pub mod verifier_signing_config;

#[cfg(not(target_arch = "wasm32"))]
use iota_types::multiaddr::Multiaddr;
#[cfg(not(target_arch = "wasm32"))]
pub use node::{ConsensusConfig, ExecutionCacheConfig, NodeConfig, WritebackCacheConfig};

#[cfg(not(target_arch = "wasm32"))]
const IOTA_DIR: &str = ".iota";
pub const IOTA_CONFIG_DIR: &str = "iota_config";
pub const IOTA_NETWORK_CONFIG: &str = "network.yaml";
pub const IOTA_FULLNODE_CONFIG: &str = "fullnode.yaml";
pub const IOTA_CLIENT_CONFIG: &str = "client.yaml";
pub const IOTA_KEYSTORE_FILENAME: &str = "iota.keystore";
pub const IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME: &str = "benchmark.keystore";
pub const IOTA_GENESIS_FILENAME: &str = "genesis.blob";
pub const IOTA_GENESIS_MIGRATION_TX_DATA_FILENAME: &str = "migration.blob";
pub const IOTA_DEV_NET_URL: &str = "https://api.devnet.iota.cafe:443";

pub const AUTHORITIES_DB_NAME: &str = "authorities_db";
pub const CONSENSUS_DB_NAME: &str = "consensus_db";
pub const FULL_NODE_DB_PATH: &str = "full_node_db";

#[cfg(not(target_arch = "wasm32"))]
pub fn iota_config_dir() -> Result<PathBuf, anyhow::Error> {
    match std::env::var_os("IOTA_CONFIG_DIR") {
        Some(config_env) => Ok(config_env.into()),
        None => match dirs::home_dir() {
            Some(v) => Ok(v.join(IOTA_DIR).join(IOTA_CONFIG_DIR)),
            None => anyhow::bail!("cannot obtain home directory path"),
        },
    }
    .and_then(|dir| {
        if !dir.exists() {
            fs::create_dir_all(dir.clone())?;
        }
        Ok(dir)
    })
}

/// Check if the genesis blob exists in the given directory or the default
/// directory.
#[cfg(not(target_arch = "wasm32"))]
pub fn genesis_blob_exists(config_dir: Option<PathBuf>) -> bool {
    if let Some(dir) = config_dir {
        dir.join(IOTA_GENESIS_FILENAME).exists()
    } else if let Some(config_env) = std::env::var_os("IOTA_CONFIG_DIR") {
        Path::new(&config_env).join(IOTA_GENESIS_FILENAME).exists()
    } else if let Some(home) = dirs::home_dir() {
        let mut config = PathBuf::new();
        config.push(&home);
        config.extend([IOTA_DIR, IOTA_CONFIG_DIR, IOTA_GENESIS_FILENAME]);
        config.exists()
    } else {
        false
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn validator_config_file(address: Multiaddr, i: usize) -> String {
    multiaddr_to_filename(address).unwrap_or(format!("validator-config-{i}.yaml"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn ssfn_config_file(address: Multiaddr, i: usize) -> String {
    multiaddr_to_filename(address).unwrap_or(format!("ssfn-config-{i}.yaml"))
}

#[cfg(not(target_arch = "wasm32"))]
fn multiaddr_to_filename(address: Multiaddr) -> Option<String> {
    if let Some(hostname) = address.hostname() {
        if let Some(port) = address.port() {
            return Some(format!("{hostname}-{port}.yaml"));
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub trait Config
where
    Self: DeserializeOwned + Serialize,
{
    fn persisted(self, path: &Path) -> PersistedConfig<Self> {
        PersistedConfig {
            inner: self,
            path: path.to_path_buf(),
        }
    }

    fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        trace!("Reading config from {}", path.display());
        let reader = fs::File::open(path)
            .with_context(|| format!("unable to load config from {}", path.display()))?;
        Ok(serde_yaml::from_reader(reader)?)
    }

    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();
        trace!("Writing config to {}", path.display());
        let mut write = BufWriter::new(fs::File::create(path)?);
        serde_yaml::to_writer(&mut write, &self)
            .with_context(|| format!("unable to save config to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct PersistedConfig<C> {
    inner: C,
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl<C> PersistedConfig<C>
where
    C: Config,
{
    pub fn read(path: &Path) -> Result<C, anyhow::Error> {
        Config::load(path)
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        self.inner.save(&self.path)
    }

    pub fn into_inner(self) -> C {
        self.inner
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<C> std::ops::Deref for PersistedConfig<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<C> std::ops::DerefMut for PersistedConfig<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
