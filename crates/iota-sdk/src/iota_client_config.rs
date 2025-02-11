// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter, Write};

use anyhow::anyhow;
use getset::{Getters, MutGetters};
use iota_config::Config;
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_types::base_types::*;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    IOTA_DEVNET_GAS_URL, IOTA_DEVNET_URL, IOTA_LOCAL_NETWORK_GAS_URL, IOTA_LOCAL_NETWORK_URL,
    IOTA_TESTNET_GAS_URL, IOTA_TESTNET_URL, IotaClient, IotaClientBuilder,
};

/// Configuration for the IOTA client, containing a
/// [`Keystore`](iota_keys::keystore::Keystore) and potentially multiple
/// [`IotaEnv`]s.
#[serde_as]
#[derive(Serialize, Deserialize, Getters, MutGetters)]
#[getset(get = "pub", get_mut = "pub")]
pub struct IotaClientConfig {
    pub(crate) keystore: Keystore,
    pub(crate) envs: Vec<IotaEnv>,
    pub(crate) active_env: Option<String>,
    pub(crate) active_address: Option<IotaAddress>,
}

impl IotaClientConfig {
    /// Create a new [`IotaClientConfig`] with the given keystore.
    pub fn new(keystore: impl Into<Keystore>) -> Self {
        let keystore = keystore.into();
        IotaClientConfig {
            envs: Default::default(),
            active_env: None,
            active_address: keystore.addresses().first().copied(),
            keystore,
        }
    }

    /// Set the [`IotaEnv`]s.
    pub fn with_envs(mut self, envs: impl IntoIterator<Item = IotaEnv>) -> Self {
        self.set_envs(envs);
        self
    }

    /// Set the [`IotaEnv`]s. Also sets the active env to the first in the list.
    pub fn set_envs(&mut self, envs: impl IntoIterator<Item = IotaEnv>) {
        self.envs = envs.into_iter().collect();
        if let Some(env) = self.envs.first() {
            self.set_active_env(env.alias().clone());
        }
    }

    /// Set the active [`IotaEnv`] by its alias.
    pub fn with_active_env(mut self, env: impl Into<Option<String>>) -> Self {
        self.set_active_env(env);
        self
    }

    /// Set the active [`IotaEnv`] by its alias.
    pub fn set_active_env(&mut self, env: impl Into<Option<String>>) {
        self.active_env = env.into();
    }

    /// Set the active [`IotaAddress`].
    pub fn with_active_address(mut self, address: impl Into<Option<IotaAddress>>) -> Self {
        self.set_active_address(address);
        self
    }

    /// Set the active [`IotaAddress`].
    pub fn set_active_address(&mut self, address: impl Into<Option<IotaAddress>>) {
        self.active_address = address.into();
    }

    /// Get an [`IotaEnv`] by its alias.
    pub fn get_env(&self, alias: &str) -> Option<&IotaEnv> {
        self.envs.iter().find(|env| env.alias == alias)
    }

    /// Get the active [`IotaEnv`].
    pub fn get_active_env(&self) -> Result<&IotaEnv, anyhow::Error> {
        self.active_env
            .as_ref()
            .and_then(|alias| self.get_env(alias))
            .ok_or_else(|| {
                anyhow!(
                    "Environment configuration not found for env [{}]",
                    self.active_env.as_deref().unwrap_or("None")
                )
            })
    }

    /// Add an [`IotaEnv`].
    pub fn add_env(&mut self, env: IotaEnv) {
        if self.get_env(&env.alias).is_none() {
            if self
                .active_env
                .as_ref()
                .and_then(|env| self.get_env(env))
                .is_none()
            {
                self.set_active_env(env.alias.clone());
            }
            self.envs.push(env);
        }
    }
}

/// IOTA environment configuration, containing the RPC URL, and optional
/// websocket, basic auth and faucet options.
#[derive(Debug, Clone, Serialize, Deserialize, Getters, MutGetters)]
#[getset(get = "pub", get_mut = "pub")]
pub struct IotaEnv {
    pub(crate) alias: String,
    pub(crate) rpc: String,
    pub(crate) ws: Option<String>,
    /// Basic HTTP access authentication in the format of username:password, if
    /// needed.
    pub(crate) basic_auth: Option<String>,
    pub(crate) faucet: Option<String>,
}

impl IotaEnv {
    /// Create a new [`IotaEnv`] with the given alias and RPC URL such as <https://api.testnet.iota.cafe>.
    pub fn new(alias: impl Into<String>, rpc: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            rpc: rpc.into(),
            ws: None,
            basic_auth: None,
            faucet: None,
        }
    }

    /// Set a websocket URL.
    pub fn with_ws(mut self, ws: impl Into<Option<String>>) -> Self {
        self.set_ws(ws);
        self
    }

    /// Set a websocket URL.
    pub fn set_ws(&mut self, ws: impl Into<Option<String>>) {
        self.ws = ws.into();
    }

    /// Set basic authentication information in the format of username:password.
    pub fn with_basic_auth(mut self, basic_auth: impl Into<Option<String>>) -> Self {
        self.set_basic_auth(basic_auth);
        self
    }

    /// Set basic authentication information in the format of username:password.
    pub fn set_basic_auth(&mut self, basic_auth: impl Into<Option<String>>) {
        self.basic_auth = basic_auth.into();
    }

    /// Set a faucet URL such as <https://faucet.testnet.iota.cafe/v1/gas>.
    pub fn with_faucet(mut self, faucet: impl Into<Option<String>>) -> Self {
        self.set_faucet(faucet);
        self
    }

    /// Set a faucet URL such as <https://faucet.testnet.iota.cafe/v1/gas>.
    pub fn set_faucet(&mut self, faucet: impl Into<Option<String>>) {
        self.faucet = faucet.into();
    }

    /// Create an [`IotaClient`] with the given request timeout, max
    /// concurrent requests and possible configured websocket URL and basic
    /// auth.
    pub async fn create_rpc_client(
        &self,
        request_timeout: impl Into<Option<std::time::Duration>>,
        max_concurrent_requests: impl Into<Option<u64>>,
    ) -> Result<IotaClient, anyhow::Error> {
        let request_timeout = request_timeout.into();
        let max_concurrent_requests = max_concurrent_requests.into();
        let mut builder = IotaClientBuilder::default();

        if let Some(request_timeout) = request_timeout {
            builder = builder.request_timeout(request_timeout);
        }
        if let Some(ws_url) = &self.ws {
            builder = builder.ws_url(ws_url);
        }
        if let Some(basic_auth) = &self.basic_auth {
            let fields: Vec<_> = basic_auth.split(':').collect();
            if fields.len() != 2 {
                return Err(anyhow!(
                    "Basic auth should be in the format `username:password`"
                ));
            }
            builder = builder.basic_auth(fields[0], fields[1]);
        }

        if let Some(max_concurrent_requests) = max_concurrent_requests {
            builder = builder.max_concurrent_requests(max_concurrent_requests as usize);
        }
        Ok(builder.build(&self.rpc).await?)
    }

    /// Create the env with the default devnet configuration.
    pub fn devnet() -> Self {
        Self {
            alias: "devnet".to_string(),
            rpc: IOTA_DEVNET_URL.into(),
            ws: None,
            basic_auth: None,
            faucet: Some(IOTA_DEVNET_GAS_URL.into()),
        }
    }

    /// Create the env with the default testnet configuration.
    pub fn testnet() -> Self {
        Self {
            alias: "testnet".to_string(),
            rpc: IOTA_TESTNET_URL.into(),
            ws: None,
            basic_auth: None,
            faucet: Some(IOTA_TESTNET_GAS_URL.into()),
        }
    }

    /// Create the env with the default localnet configuration.
    pub fn localnet() -> Self {
        Self {
            alias: "local".to_string(),
            rpc: IOTA_LOCAL_NETWORK_URL.into(),
            ws: None,
            basic_auth: None,
            faucet: Some(IOTA_LOCAL_NETWORK_GAS_URL.into()),
        }
    }
}

impl Display for IotaEnv {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();
        writeln!(writer, "Active environment: {}", self.alias)?;
        write!(writer, "RPC URL: {}", self.rpc)?;
        if let Some(ws) = &self.ws {
            writeln!(writer)?;
            write!(writer, "Websocket URL: {ws}")?;
        }
        if let Some(basic_auth) = &self.basic_auth {
            writeln!(writer)?;
            write!(writer, "Basic Auth: {basic_auth}")?;
        }
        if let Some(faucet) = &self.faucet {
            writeln!(writer)?;
            write!(writer, "Faucet URL: {faucet}")?;
        }
        write!(f, "{writer}")
    }
}

impl Config for IotaClientConfig {}

impl Display for IotaClientConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();

        writeln!(
            writer,
            "Managed addresses: {}",
            self.keystore.addresses().len()
        )?;
        write!(writer, "Active address: ")?;
        match self.active_address {
            Some(r) => writeln!(writer, "{}", r)?,
            None => writeln!(writer, "None")?,
        };
        writeln!(writer, "{}", self.keystore)?;
        if let Ok(env) = self.get_active_env() {
            write!(writer, "{}", env)?;
        }
        write!(f, "{}", writer)
    }
}
