// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter, Write};

use anyhow::anyhow;
use iota_config::Config;
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_types::base_types::*;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    IOTA_DEVNET_URL, IOTA_LOCAL_NETWORK_URL, IOTA_TESTNET_URL, IotaClient, IotaClientBuilder,
};

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct IotaClientConfig {
    pub keystore: Keystore,
    pub envs: Vec<IotaEnv>,
    pub active_env: Option<String>,
    pub active_address: Option<IotaAddress>,
}

impl IotaClientConfig {
    pub fn new(keystore: Keystore) -> Self {
        IotaClientConfig {
            keystore,
            envs: vec![],
            active_env: None,
            active_address: None,
        }
    }

    pub fn get_env(&self, alias: &Option<String>) -> Option<&IotaEnv> {
        if let Some(alias) = alias {
            self.envs.iter().find(|env| &env.alias == alias)
        } else {
            self.envs.first()
        }
    }

    pub fn get_active_env(&self) -> Result<&IotaEnv, anyhow::Error> {
        self.get_env(&self.active_env).ok_or_else(|| {
            anyhow!(
                "Environment configuration not found for env [{}]",
                self.active_env.as_deref().unwrap_or("None")
            )
        })
    }

    pub fn add_env(&mut self, env: IotaEnv) {
        if !self
            .envs
            .iter()
            .any(|other_env| other_env.alias == env.alias)
        {
            self.envs.push(env)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IotaEnv {
    pub alias: String,
    pub rpc: String,
    pub ws: Option<String>,
    /// Basic HTTP access authentication in the format of username:password, if
    /// needed.
    pub basic_auth: Option<String>,
}

impl IotaEnv {
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

    pub fn devnet() -> Self {
        Self {
            alias: "devnet".to_string(),
            rpc: IOTA_DEVNET_URL.into(),
            ws: None,
            basic_auth: None,
        }
    }
    pub fn testnet() -> Self {
        Self {
            alias: "testnet".to_string(),
            rpc: IOTA_TESTNET_URL.into(),
            ws: None,
            basic_auth: None,
        }
    }

    pub fn localnet() -> Self {
        Self {
            alias: "local".to_string(),
            rpc: IOTA_LOCAL_NETWORK_URL.into(),
            ws: None,
            basic_auth: None,
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
            write!(writer, "Basic Auth: {}", basic_auth)?;
        }
        write!(f, "{}", writer)
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
