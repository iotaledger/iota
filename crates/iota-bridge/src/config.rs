// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use anyhow::anyhow;
use ethers::{providers::Middleware, types::Address as EthAddress};
use futures::{StreamExt, future};
use iota_config::Config;
use iota_json_rpc_types::Coin;
use iota_keys::keypair_file::read_key;
use iota_sdk::{IotaClient as IotaSdkClient, IotaClientBuilder, apis::CoinReadApi};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    bridge::BridgeChainId,
    crypto::{IotaKeyPair, KeypairTraits, NetworkKeyPair, get_key_pair_from_rng},
    digests::{get_mainnet_chain_identifier, get_testnet_chain_identifier},
    event::EventID,
    object::Owner,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::info;

use crate::{
    abi::EthBridgeConfig,
    crypto::BridgeAuthorityKeyPair,
    error::BridgeError,
    eth_client::EthClient,
    iota_client::IotaClient,
    metered_eth_provider::{MeteredEthHttpProvier, new_metered_eth_provider},
    metrics::BridgeMetrics,
    types::{BridgeAction, is_route_valid},
    utils::get_eth_contract_addresses,
};

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct EthConfig {
    /// Rpc url for Eth fullnode, used for query stuff.
    pub eth_rpc_url: String,
    /// The proxy address of IotaBridge
    pub eth_bridge_proxy_address: String,
    /// The expected BridgeChainId on Eth side.
    pub eth_bridge_chain_id: u8,
    /// The starting block for EthSyncer to monitor eth contracts.
    /// It is required when `run_client` is true. Usually this is
    /// the block number when the bridge contracts are deployed.
    /// When BridgeNode starts, it reads the contract watermark from storage.
    /// If the watermark is not found, it will start from this fallback block
    /// number. If the watermark is found, it will start from the watermark.
    /// this v.s.`eth_contracts_start_block_override`:
    pub eth_contracts_start_block_fallback: Option<u64>,
    /// The starting block for EthSyncer to monitor eth contracts. It overrides
    /// the watermark in storage. This is useful when we want to reprocess the
    /// events from a specific block number.
    /// Note: this field has to be reset after starting the BridgeNode,
    /// otherwise it will reprocess the events from this block number every
    /// time it starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eth_contracts_start_block_override: Option<u64>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct IotaConfig {
    /// Rpc url for Iota fullnode, used for query stuff and submit transactions.
    pub iota_rpc_url: String,
    /// The expected BridgeChainId on Iota side.
    pub iota_bridge_chain_id: u8,
    /// Path of the file where bridge client key (any IotaKeyPair) is stored.
    /// If `run_client` is true, and this is None, then use
    /// `bridge_authority_key_path` as client key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_client_key_path: Option<PathBuf>,
    /// The gas object to use for paying for gas fees for the client. It needs
    /// to be owned by the address associated with bridge client key. If not
    /// set and `run_client` is true, it will query and use the gas object
    /// with highest amount for the account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_client_gas_object: Option<ObjectID>,
    /// Override the last processed EventID for bridge module `bridge`.
    /// When set, IotaSyncer will start from this cursor (exclusively) instead
    /// of the one in storage. If the cursor is not found in storage or
    /// override, the query will start from genesis. Key: iota module,
    /// Value: last processed EventID (tx_digest, event_seq). Note 1: This
    /// field should be rarely used. Only use it when you understand how to
    /// follow up. Note 2: the EventID needs to be valid, namely it must
    /// exist and matches the filter. Otherwise, it will miss one event
    /// because of fullnode Event query semantics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iota_bridge_module_last_processed_event_id_override: Option<EventID>,
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BridgeNodeConfig {
    /// The port that the server listens on.
    pub server_listen_port: u16,
    /// The port that for metrics server.
    pub metrics_port: u16,
    /// Path of the file where bridge authority key (Secp256k1) is stored.
    pub bridge_authority_key_path: PathBuf,
    /// Whether to run client. If true, `iota.bridge_client_key_path`
    /// and `db_path` needs to be provided.
    pub run_client: bool,
    /// Path of the client storage. Required when `run_client` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<PathBuf>,
    /// A list of approved governance actions. Action in this list will be
    /// signed when requested by client.
    pub approved_governance_actions: Vec<BridgeAction>,
    /// Iota configuration
    pub iota: IotaConfig,
    /// Eth configuration
    pub eth: EthConfig,
    /// Network key used for metrics pushing
    #[serde(default = "default_ed25519_key_pair")]
    pub metrics_key_pair: NetworkKeyPair,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub watchdog_config: Option<WatchdogConfig>,
}

pub fn default_ed25519_key_pair() -> NetworkKeyPair {
    get_key_pair_from_rng(&mut rand::rngs::OsRng).1
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MetricsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_interval_seconds: Option<u64>,
    pub push_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WatchdogConfig {
    /// Total supplies to watch on Iota. Mapping from coin name to coin type tag
    pub total_supplies: BTreeMap<String, String>,
}

impl Config for BridgeNodeConfig {}

impl BridgeNodeConfig {
    pub async fn validate(
        &self,
        metrics: Arc<BridgeMetrics>,
    ) -> anyhow::Result<(BridgeServerConfig, Option<BridgeClientConfig>)> {
        if !is_route_valid(
            BridgeChainId::try_from(self.iota.iota_bridge_chain_id)?,
            BridgeChainId::try_from(self.eth.eth_bridge_chain_id)?,
        ) {
            return Err(anyhow!(
                "Route between Iota chain id {} and Eth chain id {} is not valid",
                self.iota.iota_bridge_chain_id,
                self.eth.eth_bridge_chain_id,
            ));
        };

        let bridge_authority_key = match read_key(&self.bridge_authority_key_path, true)? {
            IotaKeyPair::Secp256k1(key) => key,
            _ => unreachable!("we required secp256k1 key in `read_key`"),
        };

        // we do this check here instead of `prepare_for_iota` below because
        // that is only called when `run_client` is true.
        let iota_client = Arc::new(
            IotaClient::<IotaSdkClient>::new(&self.iota.iota_rpc_url, metrics.clone()).await?,
        );
        let bridge_committee = iota_client
            .get_bridge_committee()
            .await
            .map_err(|e| anyhow!("Error getting bridge committee: {:?}", e))?;
        if !bridge_committee.is_active_member(&bridge_authority_key.public().into()) {
            return Err(anyhow!(
                "Bridge authority key is not part of bridge committee"
            ));
        }

        let (eth_client, eth_contracts) = self.prepare_for_eth(metrics.clone()).await?;
        let bridge_summary = iota_client
            .get_bridge_summary()
            .await
            .map_err(|e| anyhow!("Error getting bridge summary: {:?}", e))?;
        if bridge_summary.chain_id != self.iota.iota_bridge_chain_id {
            anyhow::bail!(
                "Bridge chain id mismatch: expected {}, but connected to {}",
                self.iota.iota_bridge_chain_id,
                bridge_summary.chain_id
            );
        }

        // Validate approved actions that must be governace actions
        for action in &self.approved_governance_actions {
            if !action.is_governace_action() {
                anyhow::bail!(format!(
                    "{:?}",
                    BridgeError::ActionIsNotGovernanceAction(action.clone())
                ));
            }
        }
        let approved_governance_actions = self.approved_governance_actions.clone();

        let bridge_server_config = BridgeServerConfig {
            key: bridge_authority_key,
            metrics_port: self.metrics_port,
            eth_bridge_proxy_address: eth_contracts[0], // the first contract is bridge proxy
            server_listen_port: self.server_listen_port,
            iota_client: iota_client.clone(),
            eth_client: eth_client.clone(),
            approved_governance_actions,
        };
        if !self.run_client {
            return Ok((bridge_server_config, None));
        }

        // If client is enabled, prepare client config
        let (bridge_client_key, client_iota_address, gas_object_ref) =
            self.prepare_for_iota(iota_client.clone(), metrics).await?;

        let db_path = self
            .db_path
            .clone()
            .ok_or(anyhow!("`db_path` is required when `run_client` is true"))?;

        let bridge_client_config = BridgeClientConfig {
            iota_address: client_iota_address,
            key: bridge_client_key,
            gas_object_ref,
            metrics_port: self.metrics_port,
            iota_client: iota_client.clone(),
            eth_client: eth_client.clone(),
            db_path,
            eth_contracts,
            // in `prepare_for_eth` we check if this is None when `run_client` is true. Safe to
            // unwrap here.
            eth_contracts_start_block_fallback: self
                .eth
                .eth_contracts_start_block_fallback
                .unwrap(),
            eth_contracts_start_block_override: self.eth.eth_contracts_start_block_override,
            iota_bridge_module_last_processed_event_id_override: self
                .iota
                .iota_bridge_module_last_processed_event_id_override,
        };

        Ok((bridge_server_config, Some(bridge_client_config)))
    }

    async fn prepare_for_eth(
        &self,
        metrics: Arc<BridgeMetrics>,
    ) -> anyhow::Result<(Arc<EthClient<MeteredEthHttpProvier>>, Vec<EthAddress>)> {
        let bridge_proxy_address = EthAddress::from_str(&self.eth.eth_bridge_proxy_address)?;
        let provider = Arc::new(
            new_metered_eth_provider(&self.eth.eth_rpc_url, metrics.clone())
                .unwrap()
                .interval(std::time::Duration::from_millis(2000)),
        );
        let chain_id = provider.get_chainid().await?;
        let (
            committee_address,
            limiter_address,
            vault_address,
            config_address,
            _weth_address,
            _usdt_address,
        ) = get_eth_contract_addresses(bridge_proxy_address, &provider).await?;
        let config = EthBridgeConfig::new(config_address, provider.clone());

        if self.run_client && self.eth.eth_contracts_start_block_fallback.is_none() {
            return Err(anyhow!(
                "eth_contracts_start_block_fallback is required when run_client is true"
            ));
        }

        // If bridge chain id is Eth Mainent or Sepolia, we expect to see chain
        // identifier to match accordingly.
        let bridge_chain_id: u8 = config.chain_id().call().await?;
        if self.eth.eth_bridge_chain_id != bridge_chain_id {
            return Err(anyhow!(
                "Bridge chain id mismatch: expected {}, but connected to {}",
                self.eth.eth_bridge_chain_id,
                bridge_chain_id
            ));
        }
        if bridge_chain_id == BridgeChainId::EthMainnet as u8 && chain_id.as_u64() != 1 {
            anyhow::bail!(
                "Expected Eth chain id 1, but connected to {}",
                chain_id.as_u64()
            );
        }
        if bridge_chain_id == BridgeChainId::EthSepolia as u8 && chain_id.as_u64() != 11155111 {
            anyhow::bail!(
                "Expected Eth chain id 11155111, but connected to {}",
                chain_id.as_u64()
            );
        }
        info!(
            "Connected to Eth chain: {}, Bridge chain id: {}",
            chain_id.as_u64(),
            bridge_chain_id,
        );

        let eth_client = Arc::new(
            EthClient::<MeteredEthHttpProvier>::new(
                &self.eth.eth_rpc_url,
                HashSet::from_iter(vec![
                    bridge_proxy_address,
                    committee_address,
                    config_address,
                    limiter_address,
                    vault_address,
                ]),
                metrics,
            )
            .await?,
        );
        let contract_addresses = vec![
            bridge_proxy_address,
            committee_address,
            config_address,
            limiter_address,
            vault_address,
        ];
        Ok((eth_client, contract_addresses))
    }

    async fn prepare_for_iota(
        &self,
        iota_client: Arc<IotaClient<IotaSdkClient>>,
        metrics: Arc<BridgeMetrics>,
    ) -> anyhow::Result<(IotaKeyPair, IotaAddress, ObjectRef)> {
        let bridge_client_key = match &self.iota.bridge_client_key_path {
            None => read_key(&self.bridge_authority_key_path, true),
            Some(path) => read_key(path, false),
        }?;

        // If bridge chain id is Iota Mainent or Testnet, we expect to see chain
        // identifier to match accordingly.
        let iota_identifier = iota_client
            .get_chain_identifier()
            .await
            .map_err(|e| anyhow!("Error getting chain identifier from Iota: {:?}", e))?;
        if self.iota.iota_bridge_chain_id == BridgeChainId::IotaMainnet as u8
            && iota_identifier != get_mainnet_chain_identifier().to_string()
        {
            anyhow::bail!(
                "Expected iota chain identifier {}, but connected to {}",
                self.iota.iota_bridge_chain_id,
                iota_identifier
            );
        }
        if self.iota.iota_bridge_chain_id == BridgeChainId::IotaTestnet as u8
            && iota_identifier != get_testnet_chain_identifier().to_string()
        {
            anyhow::bail!(
                "Expected iota chain identifier {}, but connected to {}",
                self.iota.iota_bridge_chain_id,
                iota_identifier
            );
        }
        info!(
            "Connected to Iota chain: {}, Bridge chain id: {}",
            iota_identifier, self.iota.iota_bridge_chain_id,
        );

        let client_iota_address = IotaAddress::from(&bridge_client_key.public());

        let gas_object_id = match self.iota.bridge_client_gas_object {
            Some(id) => id,
            None => {
                let iota_client = IotaClientBuilder::default()
                    .build(&self.iota.iota_rpc_url)
                    .await?;
                let coin =
                    // Minimum balance for gas object is 10 IOTA
                    pick_highest_balance_coin(iota_client.coin_read_api(), client_iota_address, 10_000_000_000)
                        .await?;
                coin.coin_object_id
            }
        };
        let (gas_coin, gas_object_ref, owner) = iota_client
            .get_gas_data_panic_if_not_gas(gas_object_id)
            .await;
        if owner != Owner::AddressOwner(client_iota_address) {
            return Err(anyhow!(
                "Gas object {:?} is not owned by bridge client key's associated iota address {:?}, but {:?}",
                gas_object_id,
                client_iota_address,
                owner
            ));
        }
        let balance = gas_coin.value();
        metrics.gas_coin_balance.set(balance as i64);
        info!(
            "Starting bridge client with address: {:?}, gas object {:?}, balance: {}",
            client_iota_address, gas_object_ref.0, balance,
        );

        Ok((bridge_client_key, client_iota_address, gas_object_ref))
    }
}

pub struct BridgeServerConfig {
    pub key: BridgeAuthorityKeyPair,
    pub server_listen_port: u16,
    pub eth_bridge_proxy_address: EthAddress,
    pub metrics_port: u16,
    pub iota_client: Arc<IotaClient<IotaSdkClient>>,
    pub eth_client: Arc<EthClient<MeteredEthHttpProvier>>,
    /// A list of approved governance actions. Action in this list will be
    /// signed when requested by client.
    pub approved_governance_actions: Vec<BridgeAction>,
}

pub struct BridgeClientConfig {
    pub iota_address: IotaAddress,
    pub key: IotaKeyPair,
    pub gas_object_ref: ObjectRef,
    pub metrics_port: u16,
    pub iota_client: Arc<IotaClient<IotaSdkClient>>,
    pub eth_client: Arc<EthClient<MeteredEthHttpProvier>>,
    pub db_path: PathBuf,
    pub eth_contracts: Vec<EthAddress>,
    // See `BridgeNodeConfig` for the explanation of following two fields.
    pub eth_contracts_start_block_fallback: u64,
    pub eth_contracts_start_block_override: Option<u64>,
    pub iota_bridge_module_last_processed_event_id_override: Option<EventID>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BridgeCommitteeConfig {
    pub bridge_authority_port_and_key_path: Vec<(u64, PathBuf)>,
}

impl Config for BridgeCommitteeConfig {}

pub async fn pick_highest_balance_coin(
    coin_read_api: &CoinReadApi,
    address: IotaAddress,
    minimal_amount: u64,
) -> anyhow::Result<Coin> {
    let mut highest_balance = 0;
    let mut highest_balance_coin = None;
    coin_read_api
        .get_coins_stream(address, None)
        .for_each(|coin: Coin| {
            if coin.balance > highest_balance {
                highest_balance = coin.balance;
                highest_balance_coin = Some(coin.clone());
            }
            future::ready(())
        })
        .await;
    if highest_balance_coin.is_none() {
        return Err(anyhow!("No Iota coins found for address {:?}", address));
    }
    if highest_balance < minimal_amount {
        return Err(anyhow!(
            "Found no single coin that has >= {} balance Iota for address {:?}",
            minimal_amount,
            address,
        ));
    }
    Ok(highest_balance_coin.unwrap())
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct EthContractAddresses {
    pub iota_bridge: EthAddress,
    pub bridge_committee: EthAddress,
    pub bridge_config: EthAddress,
    pub bridge_limiter: EthAddress,
    pub bridge_vault: EthAddress,
}
