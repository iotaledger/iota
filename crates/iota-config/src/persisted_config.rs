// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Filesystem-backed configuration helpers.

use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use iota_types::multiaddr::Multiaddr;
use serde::{Serialize, de::DeserializeOwned};
use tracing::trace;

use crate::{IOTA_CONFIG_DIR, IOTA_GENESIS_FILENAME};

const IOTA_DIR: &str = ".iota";

/// Return the IOTA config directory, creating it if it doesn't exist.
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

/// Config file name for the validator at the given address (or index).
pub fn validator_config_file(address: Multiaddr, i: usize) -> String {
    multiaddr_to_filename(address).unwrap_or(format!("validator-config-{i}.yaml"))
}

/// Config file name for the State Sync Full Node at the given address (or
/// index).
pub fn ssfn_config_file(address: Multiaddr, i: usize) -> String {
    multiaddr_to_filename(address).unwrap_or(format!("ssfn-config-{i}.yaml"))
}

/// Derive a `<hostname>-<port>.yaml` file name from a multiaddr, if possible.
fn multiaddr_to_filename(address: Multiaddr) -> Option<String> {
    if let Some(hostname) = address.hostname() {
        if let Some(port) = address.port() {
            return Some(format!("{hostname}-{port}.yaml"));
        }
    }
    None
}

/// A config type that can be loaded from and saved to a YAML file on disk.
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

/// A [`Config`] paired with the on-disk path it is persisted to.
pub struct PersistedConfig<C> {
    inner: C,
    path: PathBuf,
}

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

impl<C> std::ops::Deref for PersistedConfig<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<C> std::ops::DerefMut for PersistedConfig<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
