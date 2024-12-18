// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::abi::EthBridgeConfig;
use crate::abi::{EthBridgeCommittee, EthBridgeEvent, EthERC20, EthIotaBridge, EthIotaBridgeEvents};
use crate::config::default_ed25519_key_pair;
use crate::crypto::BridgeAuthorityKeyPair;
use crate::crypto::BridgeAuthorityPublicKeyBytes;
use crate::crypto::BridgeAuthoritySignInfo;
use crate::events::*;
use crate::metrics::BridgeMetrics;
use crate::server::BridgeNodePublicMetadata;
use crate::iota_transaction_builder::build_add_tokens_on_iota_transaction;
use crate::iota_transaction_builder::build_committee_register_transaction;
use crate::types::BridgeCommitteeValiditySignInfo;
use crate::types::CertifiedBridgeAction;
use crate::types::VerifiedCertifiedBridgeAction;
use crate::types::{BridgeAction, BridgeActionStatus, IotaToEthBridgeAction};
use crate::utils::get_eth_signer_client;
use crate::utils::publish_and_register_coins_return_add_coins_on_iota_action;
use crate::utils::wait_for_server_to_be_up;
use crate::utils::EthSigner;
use ethers::types::Address as EthAddress;
use futures::future::join_all;
use futures::Future;
use move_core_types::language_storage::{StructTag, TypeTag};
use prometheus::Registry;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::fs::{self, DirBuilder};
use std::io::{Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;
use iota_json_rpc_api::BridgeReadApiClient;
use iota_json_rpc_types::IotaEvent;
use iota_json_rpc_types::IotaExecutionStatus;
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_json_rpc_types::IotaTransactionBlockResponseQuery;
use iota_json_rpc_types::TransactionFilter;
use iota_sdk::wallet_context::WalletContext;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::base_types::{ObjectID, ObjectRef};
use iota_types::bridge::get_bridge_obj_initial_shared_version;
use iota_types::bridge::BridgeChainId;
use iota_types::bridge::BridgeSummary;
use iota_types::bridge::BridgeTrait;
use iota_types::bridge::{get_bridge, BRIDGE_MODULE_NAME};
use iota_types::bridge::{TOKEN_ID_BTC, TOKEN_ID_ETH, TOKEN_ID_USDC, TOKEN_ID_USDT};
use iota_types::committee::TOTAL_VOTING_POWER;
use iota_types::crypto::get_key_pair;
use iota_types::crypto::ToFromBytes;
use iota_types::digests::TransactionDigest;
use iota_types::object::Object;
use iota_types::transaction::{ObjectArg, Transaction, TransactionData};
use iota_types::{BRIDGE_PACKAGE_ID, IOTA_BRIDGE_OBJECT_ID};
use tokio::join;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use tracing::error;
use tracing::info;

use crate::config::{BridgeNodeConfig, EthConfig, IotaConfig};
use crate::node::run_bridge_node;
use crate::iota_client::IotaBridgeClient;
use crate::BRIDGE_ENABLE_PROTOCOL_VERSION;
use anyhow::anyhow;
use ethers::prelude::*;
use move_core_types::ident_str;
use std::process::Child;
use iota_config::local_ip_utils::get_available_port;
use iota_sdk::IotaClient;
use iota_types::base_types::IotaAddress;
use iota_types::crypto::EncodeDecodeBase64;
use iota_types::crypto::KeypairTraits;
use iota_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use tap::TapFallible;
use tempfile::tempdir;
use test_cluster::TestCluster;
use test_cluster::TestClusterBuilder;

const BRIDGE_COMMITTEE_NAME: &str = "BridgeCommittee";
const IOTA_BRIDGE_NAME: &str = "IotaBridge";
const BRIDGE_CONFIG_NAME: &str = "BridgeConfig";
const BRIDGE_LIMITER_NAME: &str = "BridgeLimiter";
const BRIDGE_VAULT_NAME: &str = "BridgeVault";
const BTC_NAME: &str = "BTC";
const ETH_NAME: &str = "ETH";
const USDC_NAME: &str = "USDC";
const USDT_NAME: &str = "USDT";
const KA_NAME: &str = "KA";

pub const TEST_PK: &str = "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356";

/// A helper struct that holds TestCluster and other Bridge related
/// structs that are needed for testing.
pub struct BridgeTestCluster {
    pub num_validators: usize,
    pub test_cluster: TestClusterWrapper,
    bridge_client: IotaBridgeClient,
    eth_environment: EthBridgeEnvironment,
    bridge_node_handles: Option<Vec<JoinHandle<()>>>,
    approved_governance_actions_for_next_start: Option<Vec<Vec<BridgeAction>>>,
    bridge_tx_cursor: Option<TransactionDigest>,
    eth_chain_id: BridgeChainId,
    iota_chain_id: BridgeChainId,
}

pub struct BridgeTestClusterBuilder {
    with_eth_env: bool,
    with_bridge_cluster: bool,
    num_validators: usize,
    approved_governance_actions: Option<Vec<Vec<BridgeAction>>>,
    eth_chain_id: BridgeChainId,
    iota_chain_id: BridgeChainId,
}

impl Default for BridgeTestClusterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BridgeTestClusterBuilder {
    pub fn new() -> Self {
        BridgeTestClusterBuilder {
            with_eth_env: false,
            with_bridge_cluster: false,
            num_validators: 4,
            approved_governance_actions: None,
            eth_chain_id: BridgeChainId::EthCustom,
            iota_chain_id: BridgeChainId::IotaCustom,
        }
    }

    pub fn with_eth_env(mut self, with_eth_env: bool) -> Self {
        self.with_eth_env = with_eth_env;
        self
    }

    pub fn with_bridge_cluster(mut self, with_bridge_cluster: bool) -> Self {
        self.with_bridge_cluster = with_bridge_cluster;
        self
    }

    pub fn with_num_validators(mut self, num_validators: usize) -> Self {
        self.num_validators = num_validators;
        self
    }

    pub fn with_approved_governance_actions(
        mut self,
        approved_governance_actions: Vec<Vec<BridgeAction>>,
    ) -> Self {
        assert_eq!(approved_governance_actions.len(), self.num_validators);
        self.approved_governance_actions = Some(approved_governance_actions);
        self
    }

    pub fn with_iota_chain_id(mut self, chain_id: BridgeChainId) -> Self {
        self.iota_chain_id = chain_id;
        self
    }

    pub fn with_eth_chain_id(mut self, chain_id: BridgeChainId) -> Self {
        self.eth_chain_id = chain_id;
        self
    }

    pub async fn build(self) -> BridgeTestCluster {
        init_all_struct_tags();
        std::env::set_var("__TEST_ONLY_CONSENSUS_USE_LONG_MIN_ROUND_DELAY", "1");
        let metrics = Arc::new(BridgeMetrics::new_for_testing());
        let mut bridge_keys = vec![];
        let mut bridge_keys_copy = vec![];
        for _ in 0..self.num_validators {
            let (_, kp): (_, BridgeAuthorityKeyPair) = get_key_pair();
            bridge_keys.push(kp.copy());
            bridge_keys_copy.push(kp);
        }
        let start_cluster_task = tokio::task::spawn(Self::start_test_cluster(bridge_keys));
        let start_eth_env_task = tokio::task::spawn(Self::start_eth_env(bridge_keys_copy));
        let (start_cluster_res, start_eth_env_res) = join!(start_cluster_task, start_eth_env_task);
        let test_cluster = start_cluster_res.unwrap();
        let eth_environment = start_eth_env_res.unwrap();

        let mut bridge_node_handles = None;
        if self.with_bridge_cluster {
            let approved_governace_actions = self
                .approved_governance_actions
                .clone()
                .unwrap_or(vec![vec![]; self.num_validators]);
            bridge_node_handles = Some(
                start_bridge_cluster(&test_cluster, &eth_environment, approved_governace_actions)
                    .await,
            );
        }
        let bridge_client =
            IotaBridgeClient::new(&test_cluster.inner.fullnode_handle.rpc_url, metrics)
                .await
                .unwrap();
        info!(
            "Bridge committee: {:?}",
            bridge_client
                .get_bridge_committee()
                .await
                .unwrap()
                .to_string()
        );
        BridgeTestCluster {
            num_validators: self.num_validators,
            test_cluster,
            bridge_client,
            eth_environment,
            bridge_node_handles,
            approved_governance_actions_for_next_start: self.approved_governance_actions,
            bridge_tx_cursor: None,
            iota_chain_id: self.iota_chain_id,
            eth_chain_id: self.eth_chain_id,
        }
    }

    async fn start_test_cluster(bridge_keys: Vec<BridgeAuthorityKeyPair>) -> TestClusterWrapper {
        let test_cluster = TestClusterWrapperBuilder::new()
            .with_bridge_authority_keys(bridge_keys)
            .with_deploy_tokens(true)
            .build()
            .await;
        info!("Test cluster built");
        test_cluster
            .trigger_reconfiguration_if_not_yet_and_assert_bridge_committee_initialized()
            .await;
        test_cluster
    }

    async fn start_eth_env(bridge_keys: Vec<BridgeAuthorityKeyPair>) -> EthBridgeEnvironment {
        let anvil_port = get_available_port("127.0.0.1");
        let anvil_url = format!("http://127.0.0.1:{anvil_port}");
        let mut eth_environment = EthBridgeEnvironment::new(&anvil_url, anvil_port)
            .await
            .unwrap();
        // Give anvil a bit of time to start
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        let (eth_signer, eth_pk_hex) = eth_environment
            .get_signer(TEST_PK)
            .await
            .unwrap_or_else(|e| panic!("Failed to get eth signer from anvil at {anvil_url}: {e}"));
        let deployed_contracts =
            deploy_sol_contract(&anvil_url, eth_signer, bridge_keys, eth_pk_hex).await;
        info!("Deployed contracts: {:?}", deployed_contracts);
        eth_environment.contracts = Some(deployed_contracts);
        eth_environment
    }
}

impl BridgeTestCluster {
    pub async fn get_eth_signer_and_private_key(&self) -> anyhow::Result<(EthSigner, String)> {
        self.eth_environment.get_signer(TEST_PK).await
    }

    pub async fn get_eth_signer_and_address(&self) -> anyhow::Result<(EthSigner, EthAddress)> {
        let (eth_signer, _) = self.get_eth_signer_and_private_key().await?;
        let eth_address = eth_signer.address();
        Ok((eth_signer, eth_address))
    }

    pub async fn get_eth_signer(&self) -> EthSigner {
        let (eth_signer, _) = self.get_eth_signer_and_private_key().await.unwrap();
        eth_signer
    }

    pub fn bridge_client(&self) -> &IotaBridgeClient {
        &self.bridge_client
    }

    pub fn iota_client(&self) -> &IotaClient {
        &self.test_cluster.inner.fullnode_handle.iota_client
    }

    pub fn iota_user_address(&self) -> IotaAddress {
        self.test_cluster.inner.get_address_0()
    }

    pub fn iota_chain_id(&self) -> BridgeChainId {
        self.iota_chain_id
    }

    pub fn eth_chain_id(&self) -> BridgeChainId {
        self.eth_chain_id
    }

    pub fn eth_env(&self) -> &EthBridgeEnvironment {
        &self.eth_environment
    }

    pub fn contracts(&self) -> &DeployedSolContracts {
        self.eth_environment.contracts()
    }

    pub fn iota_bridge_address(&self) -> String {
        self.eth_environment.contracts().iota_bridge_address_hex()
    }

    pub fn wallet_mut(&mut self) -> &mut WalletContext {
        self.test_cluster.inner.wallet_mut()
    }

    pub fn wallet(&self) -> &WalletContext {
        &self.test_cluster.inner.wallet
    }

    pub fn bridge_authority_key(&self, index: usize) -> BridgeAuthorityKeyPair {
        self.test_cluster.bridge_authority_keys[index].copy()
    }

    pub fn iota_rpc_url(&self) -> String {
        self.test_cluster.inner.fullnode_handle.rpc_url.clone()
    }

    pub fn eth_rpc_url(&self) -> String {
        self.eth_environment.rpc_url.clone()
    }

    pub async fn get_mut_bridge_arg(&self) -> Option<ObjectArg> {
        self.test_cluster.get_mut_bridge_arg().await
    }

    pub async fn test_transaction_builder_with_sender(
        &self,
        sender: IotaAddress,
    ) -> TestTransactionBuilder {
        self.test_cluster
            .inner
            .test_transaction_builder_with_sender(sender)
            .await
    }

    pub async fn wait_for_bridge_cluster_to_be_up(&self, timeout_sec: u64) {
        self.test_cluster
            .wait_for_bridge_cluster_to_be_up(timeout_sec)
            .await;
    }

    pub async fn sign_and_execute_transaction(
        &self,
        tx_data: &TransactionData,
    ) -> IotaTransactionBlockResponse {
        self.test_cluster
            .inner
            .sign_and_execute_transaction(tx_data)
            .await
    }

    pub fn set_approved_governance_actions_for_next_start(
        &mut self,
        approved_governance_actions: Vec<Vec<BridgeAction>>,
    ) {
        assert_eq!(approved_governance_actions.len(), self.num_validators);
        self.approved_governance_actions_for_next_start = Some(approved_governance_actions);
    }

    pub async fn start_bridge_cluster(&mut self) {
        assert!(self.bridge_node_handles.is_none());
        let approved_governace_actions = self
            .approved_governance_actions_for_next_start
            .clone()
            .unwrap_or(vec![vec![], vec![], vec![], vec![]]);
        self.bridge_node_handles = Some(
            start_bridge_cluster(
                &self.test_cluster,
                &self.eth_environment,
                approved_governace_actions,
            )
            .await,
        );
    }

    /// Returns new bridge transaction. It advanaces the stored tx digest cursor.
    /// When `assert_success` is true, it asserts all transactions are successful.
    pub async fn new_bridge_transactions(
        &mut self,
        assert_success: bool,
    ) -> Vec<IotaTransactionBlockResponse> {
        let resps = self
            .iota_client()
            .read_api()
            .query_transaction_blocks(
                IotaTransactionBlockResponseQuery {
                    filter: Some(TransactionFilter::InputObject(IOTA_BRIDGE_OBJECT_ID)),
                    options: Some(IotaTransactionBlockResponseOptions::full_content()),
                },
                self.bridge_tx_cursor,
                None,
                false,
            )
            .await
            .unwrap();
        self.bridge_tx_cursor = resps.next_cursor;

        for tx in &resps.data {
            if assert_success {
                assert!(tx.status_ok().unwrap());
            }
            let events = &tx.events.as_ref().unwrap().data;
            if events
                .iter()
                .any(|e| &e.type_ == TokenTransferApproved.get().unwrap())
            {
                assert!(events
                    .iter()
                    .any(|e| &e.type_ == TokenTransferClaimed.get().unwrap()
                        || &e.type_ == TokenTransferApproved.get().unwrap()));
            } else if events
                .iter()
                .any(|e| &e.type_ == TokenTransferAlreadyClaimed.get().unwrap())
            {
                assert!(events
                    .iter()
                    .all(|e| &e.type_ == TokenTransferAlreadyClaimed.get().unwrap()
                        || &e.type_ == TokenTransferAlreadyApproved.get().unwrap()));
            }
            // TODO: check for other events e.g. TokenRegistrationEvent, NewTokenEvent etc
        }
        resps.data
    }

    /// Returns events that are emitted in new bridge transaction and match `event_types`.
    /// It advanaces the stored tx digest cursor.
    /// See `new_bridge_transactions` for `assert_success`.
    pub async fn new_bridge_events(
        &mut self,
        event_types: HashSet<StructTag>,
        assert_success: bool,
    ) -> Vec<IotaEvent> {
        let txes = self.new_bridge_transactions(assert_success).await;
        let events = txes
            .iter()
            .flat_map(|tx| {
                tx.events
                    .as_ref()
                    .unwrap()
                    .data
                    .iter()
                    .filter(|e| event_types.contains(&e.type_))
                    .cloned()
            })
            .collect();
        events
    }
}

pub async fn get_eth_signer_client_e2e_test_only(
    eth_rpc_url: &str,
) -> anyhow::Result<(EthSigner, String)> {
    // This private key is derived from the default anvil setting.
    // Mnemonic:          test test test test test test test test test test test junk
    // Derivation path:   m/44'/60'/0'/0/
    // DO NOT USE IT ANYWHERE ELSE EXCEPT FOR RUNNING AUTOMATIC INTEGRATION TESTING
    let url = eth_rpc_url.to_string();
    let private_key_0 = "0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356";
    let signer_0 = get_eth_signer_client(&url, private_key_0).await?;
    Ok((signer_0, private_key_0.to_string()))
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeployedSolContracts {
    pub iota_bridge: EthAddress,
    pub bridge_committee: EthAddress,
    pub bridge_limiter: EthAddress,
    pub bridge_vault: EthAddress,
    pub bridge_config: EthAddress,
    pub btc: EthAddress,
    pub eth: EthAddress,
    pub usdc: EthAddress,
    pub usdt: EthAddress,
    pub ka: EthAddress,
}

impl DeployedSolContracts {
    pub fn eth_address_to_hex(addr: EthAddress) -> String {
        format!("{:x}", addr)
    }

    pub fn iota_bridge_address_hex(&self) -> String {
        Self::eth_address_to_hex(self.iota_bridge)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SolDeployConfig {
    committee_member_stake: Vec<u64>,
    committee_members: Vec<String>,
    min_committee_stake_required: u64,
    source_chain_id: u64,
    supported_chain_ids: Vec<u64>,
    supported_chain_limits_in_dollars: Vec<u64>,
    supported_tokens: Vec<String>,
    token_ids: Vec<u64>,
    iota_decimals: Vec<u64>,
    token_prices: Vec<u64>,
    weth: String,
}

pub(crate) async fn deploy_sol_contract(
    anvil_url: &str,
    eth_signer: EthSigner,
    bridge_authority_keys: Vec<BridgeAuthorityKeyPair>,
    eth_private_key_hex: String,
) -> DeployedSolContracts {
    let sol_path = format!("{}/../../bridge/evm", env!("CARGO_MANIFEST_DIR"));

    // Write the deploy config to a temp file then provide it to the forge late
    let deploy_config_path = tempfile::tempdir()
        .unwrap()
        .into_path()
        .join("sol_deploy_config.json");
    let node_len = bridge_authority_keys.len();
    let stake = TOTAL_VOTING_POWER / (node_len as u64);
    let committee_members = bridge_authority_keys
        .iter()
        .map(|k| {
            format!(
                "0x{:x}",
                BridgeAuthorityPublicKeyBytes::from(&k.public).to_eth_address()
            )
        })
        .collect::<Vec<_>>();
    let mut committee_member_stake = vec![stake; node_len];
    // Adjust it so that the total stake is equal to TOTAL_VOTING_POWER
    committee_member_stake[node_len - 1] = TOTAL_VOTING_POWER - stake * (node_len as u64 - 1);
    let deploy_config = SolDeployConfig {
        committee_member_stake: committee_member_stake.clone(),
        committee_members: committee_members.clone(),
        min_committee_stake_required: 10000,
        source_chain_id: 12,
        supported_chain_ids: vec![1, 2, 3],
        supported_chain_limits_in_dollars: vec![
            1000000000000000,
            1000000000000000,
            1000000000000000,
        ],
        supported_tokens: vec![], // this is set up in the deploy script
        token_ids: vec![],        // this is set up in the deploy script
        iota_decimals: vec![],     // this is set up in the deploy script
        token_prices: vec![12800, 432518900, 25969600, 10000, 10000],
        weth: "".to_string(), // this is set up in the deploy script
    };

    let serialized_config = serde_json::to_string_pretty(&deploy_config).unwrap();
    tracing::info!(
        "Serialized config written to {:?}: {:?}",
        deploy_config_path,
        serialized_config
    );
    let mut file = File::create(deploy_config_path.clone()).unwrap();
    file.write_all(serialized_config.as_bytes()).unwrap();

    // override for the deploy script
    std::env::set_var("OVERRIDE_CONFIG_PATH", deploy_config_path.to_str().unwrap());
    std::env::set_var("PRIVATE_KEY", eth_private_key_hex);
    std::env::set_var("ETHERSCAN_API_KEY", "n/a");

    // We provide a unique out path for each run to avoid conflicts
    let mut rng = SmallRng::from_entropy();
    let random_number = rng.gen::<u32>();
    let forge_out_path = PathBuf::from(format!("out-{random_number}"));
    let _dir = TempDir::new(
        PathBuf::from(sol_path.clone())
            .join(forge_out_path.clone())
            .as_path(),
    )
    .unwrap();
    std::env::set_var("FOUNDRY_OUT", forge_out_path.to_str().unwrap());

    info!("Deploying solidity contracts");
    Command::new("forge")
        .current_dir(sol_path.clone())
        .arg("clean")
        .status()
        .expect("Failed to execute `forge clean`");

    let mut child = Command::new("forge")
        .current_dir(sol_path)
        .arg("script")
        .arg("script/deploy_bridge.s.sol")
        .arg("--fork-url")
        .arg(anvil_url)
        .arg("--broadcast")
        .arg("--ffi")
        .arg("--chain")
        .arg("31337")
        .stdout(std::process::Stdio::piped()) // Capture stdout
        .stderr(std::process::Stdio::piped()) // Capture stderr
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().expect("Failed to open stdout");
    let mut stderr = child.stderr.take().expect("Failed to open stderr");

    // Read stdout/stderr to String
    let mut s = String::new();
    stdout.read_to_string(&mut s).unwrap();
    let mut e = String::new();
    stderr.read_to_string(&mut e).unwrap();

    // Wait for the child process to finish and collect its status
    let status = child.wait().unwrap();
    if status.success() {
        info!("Solidity contract deployment finished successfully");
    } else {
        error!(
            "Solidity contract deployment exited with code: {:?}",
            status.code()
        );
    }
    println!("Stdout: {}", s);
    println!("Stdout: {}", e);

    let mut deployed_contracts = BTreeMap::new();
    // Process the stdout to parse contract addresses
    for line in s.lines() {
        if line.contains("[Deployed]") {
            let replaced_line = line.replace("[Deployed]", "");
            let trimmed_line = replaced_line.trim();
            let parts: Vec<&str> = trimmed_line.split(':').collect();
            if parts.len() == 2 {
                let contract_name = parts[0].to_string().trim().to_string();
                let contract_address = EthAddress::from_str(parts[1].to_string().trim()).unwrap();
                deployed_contracts.insert(contract_name, contract_address);
            }
        }
    }

    let contracts = DeployedSolContracts {
        iota_bridge: deployed_contracts.remove(IOTA_BRIDGE_NAME).unwrap(),
        bridge_committee: deployed_contracts.remove(BRIDGE_COMMITTEE_NAME).unwrap(),
        bridge_config: deployed_contracts.remove(BRIDGE_CONFIG_NAME).unwrap(),
        bridge_limiter: deployed_contracts.remove(BRIDGE_LIMITER_NAME).unwrap(),
        bridge_vault: deployed_contracts.remove(BRIDGE_VAULT_NAME).unwrap(),
        btc: deployed_contracts.remove(BTC_NAME).unwrap(),
        eth: deployed_contracts.remove(ETH_NAME).unwrap(),
        usdc: deployed_contracts.remove(USDC_NAME).unwrap(),
        usdt: deployed_contracts.remove(USDT_NAME).unwrap(),
        ka: deployed_contracts.remove(KA_NAME).unwrap(),
    };
    let eth_bridge_committee =
        EthBridgeCommittee::new(contracts.bridge_committee, eth_signer.clone().into());
    for (i, (m, s)) in committee_members
        .iter()
        .zip(committee_member_stake.iter())
        .enumerate()
    {
        let eth_address = EthAddress::from_str(m).unwrap();
        assert_eq!(
            eth_bridge_committee
                .committee_index(eth_address)
                .await
                .unwrap(),
            i as u8
        );
        assert_eq!(
            eth_bridge_committee
                .committee_stake(eth_address)
                .await
                .unwrap(),
            *s as u16
        );
        assert!(!eth_bridge_committee.blocklist(eth_address).await.unwrap());
    }
    contracts
}

#[derive(Debug)]
pub struct EthBridgeEnvironment {
    pub rpc_url: String,
    process: Child,
    contracts: Option<DeployedSolContracts>,
}

impl EthBridgeEnvironment {
    async fn new(anvil_url: &str, anvil_port: u16) -> anyhow::Result<EthBridgeEnvironment> {
        // Start eth node with anvil
        let eth_environment_process = std::process::Command::new("anvil")
            .arg("--port")
            .arg(anvil_port.to_string())
            .arg("--block-time")
            .arg("1") // 1 second block time
            .arg("--slots-in-an-epoch")
            .arg("1") // 1 slots in an epoch
            .spawn()
            .expect("Failed to start anvil");

        Ok(EthBridgeEnvironment {
            rpc_url: anvil_url.to_string(),
            process: eth_environment_process,
            contracts: None,
        })
    }

    pub(crate) async fn get_signer(
        &self,
        private_key: &str,
    ) -> anyhow::Result<(EthSigner, String)> {
        let signer = get_eth_signer_client(&self.rpc_url, private_key).await?;
        Ok((signer, private_key.to_string()))
    }

    pub(crate) fn contracts(&self) -> &DeployedSolContracts {
        self.contracts.as_ref().unwrap()
    }

    pub fn get_bridge_config(
        &self,
    ) -> EthBridgeConfig<ethers::prelude::Provider<ethers::providers::Http>> {
        let provider = Arc::new(
            ethers::prelude::Provider::<ethers::providers::Http>::try_from(&self.rpc_url)
                .unwrap()
                .interval(std::time::Duration::from_millis(2000)),
        );
        EthBridgeConfig::new(self.contracts().bridge_config, provider.clone())
    }

    pub async fn get_supported_token(&self, token_id: u8) -> (EthAddress, u8, u64) {
        let config = self.get_bridge_config();
        let token_address = config.token_address_of(token_id).call().await.unwrap();
        let token_iota_decimal = config.token_iota_decimal_of(token_id).call().await.unwrap();
        let token_price = config.token_price_of(token_id).call().await.unwrap();
        (token_address, token_iota_decimal, token_price)
    }
}

impl Drop for EthBridgeEnvironment {
    fn drop(&mut self) {
        self.process.kill().unwrap();
    }
}

pub(crate) async fn start_bridge_cluster(
    test_cluster: &TestClusterWrapper,
    eth_environment: &EthBridgeEnvironment,
    approved_governance_actions: Vec<Vec<BridgeAction>>,
) -> Vec<JoinHandle<()>> {
    let bridge_authority_keys = test_cluster
        .bridge_authority_keys
        .iter()
        .map(|k| k.copy())
        .collect::<Vec<_>>();
    let bridge_server_ports = test_cluster.bridge_server_ports.clone();
    assert_eq!(bridge_authority_keys.len(), bridge_server_ports.len());
    assert_eq!(
        bridge_authority_keys.len(),
        approved_governance_actions.len()
    );

    let eth_bridge_contract_address = eth_environment
        .contracts
        .as_ref()
        .unwrap()
        .iota_bridge_address_hex();

    let mut handles = vec![];
    for (i, ((kp, server_listen_port), approved_governance_actions)) in bridge_authority_keys
        .iter()
        .zip(bridge_server_ports.iter())
        .zip(approved_governance_actions.into_iter())
        .enumerate()
    {
        // prepare node config (server + client)
        let tmp_dir = tempdir().unwrap().into_path().join(i.to_string());
        std::fs::create_dir_all(tmp_dir.clone()).unwrap();
        let db_path = tmp_dir.join("client_db");
        // write authority key to file
        let authority_key_path = tmp_dir.join("bridge_authority_key");
        let base64_encoded = kp.encode_base64();
        std::fs::write(authority_key_path.clone(), base64_encoded).unwrap();

        let config = BridgeNodeConfig {
            server_listen_port: *server_listen_port,
            metrics_port: get_available_port("127.0.0.1"),
            bridge_authority_key_path: authority_key_path,
            approved_governance_actions,
            run_client: i == 0,
            db_path: Some(db_path),
            eth: EthConfig {
                eth_rpc_url: eth_environment.rpc_url.clone(),
                eth_bridge_proxy_address: eth_bridge_contract_address.clone(),
                eth_bridge_chain_id: BridgeChainId::EthCustom as u8,
                eth_contracts_start_block_fallback: Some(0),
                eth_contracts_start_block_override: None,
            },
            iota: IotaConfig {
                iota_rpc_url: test_cluster.inner.fullnode_handle.rpc_url.clone(),
                iota_bridge_chain_id: BridgeChainId::IotaCustom as u8,
                bridge_client_key_path: None,
                bridge_client_gas_object: None,
                iota_bridge_module_last_processed_event_id_override: None,
            },
            metrics_key_pair: default_ed25519_key_pair(),
            metrics: None,
            watchdog_config: None,
        };
        // Spawn bridge node in memory
        handles.push(
            run_bridge_node(
                config,
                BridgeNodePublicMetadata::empty_for_testing(),
                Registry::new(),
            )
            .await
            .unwrap(),
        );
    }
    handles
}

pub async fn get_signatures(
    iota_bridge_client: &IotaBridgeClient,
    nonce: u64,
    iota_chain_id: u8,
) -> Vec<Bytes> {
    let sigs = iota_bridge_client
        .get_token_transfer_action_onchain_signatures_until_success(iota_chain_id, nonce)
        .await
        .unwrap();

    sigs.into_iter()
        .map(|sig: Vec<u8>| Bytes::from(sig))
        .collect()
}

pub(crate) async fn send_eth_tx_and_get_tx_receipt<B, M, D>(
    call: FunctionCall<B, M, D>,
) -> TransactionReceipt
where
    M: Middleware,
    B: std::borrow::Borrow<M>,
    D: ethers::abi::Detokenize,
{
    call.send().await.unwrap().await.unwrap().unwrap()
}

/// A simple struct to create a temporary directory that
/// will be removed when it goes out of scope.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(dir_path: &Path) -> std::io::Result<TempDir> {
        DirBuilder::new().recursive(true).create(dir_path)?;
        Ok(TempDir {
            path: dir_path.to_path_buf(),
        })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Use eprintln! here in case logging is not initialized
        if let Err(e) = fs::remove_dir_all(&self.path) {
            eprintln!("Failed to remove temp dir: {:?}", e);
        }
    }
}

pub struct TestClusterWrapperBuilder {
    protocol_version: u64,
    bridge_authority_keys: Vec<BridgeAuthorityKeyPair>,
    deploy_tokens: bool,
}

impl TestClusterWrapperBuilder {
    pub fn new() -> Self {
        Self {
            protocol_version: BRIDGE_ENABLE_PROTOCOL_VERSION,
            bridge_authority_keys: vec![],
            deploy_tokens: false,
        }
    }

    pub fn with_protocol_version(mut self, version: u64) -> Self {
        self.protocol_version = version;
        self
    }

    pub fn with_bridge_authority_keys(mut self, keys: Vec<BridgeAuthorityKeyPair>) -> Self {
        self.bridge_authority_keys = keys;
        self
    }

    pub fn with_deploy_tokens(mut self, deploy_tokens: bool) -> Self {
        self.deploy_tokens = deploy_tokens;
        self
    }

    pub async fn build(self) -> TestClusterWrapper {
        assert_ne!(self.bridge_authority_keys.len(), 0);
        let num_validators = self.bridge_authority_keys.len();
        let builder = TestClusterBuilder::new().with_protocol_version(self.protocol_version.into());

        let timer = Instant::now();
        let gas_objects_for_authority_keys = self
            .bridge_authority_keys
            .iter()
            .map(|k| {
                let address = IotaAddress::from(k.public());
                Object::with_id_owner_for_testing(ObjectID::random(), address)
            })
            .collect::<Vec<_>>();
        let mut test_cluster = builder
            .with_num_validators(num_validators)
            .with_objects(gas_objects_for_authority_keys)
            .build()
            .await;
        info!(
            "TestCluster build took {:?} secs",
            timer.elapsed().as_secs()
        );
        let ref_gas_price = test_cluster.get_reference_gas_price().await;
        let bridge_arg = get_mut_bridge_arg(&test_cluster).await.unwrap();
        assert_eq!(
            self.bridge_authority_keys.len(),
            test_cluster.swarm.active_validators().count()
        );

        // Committee registers themselves
        let mut server_ports = vec![];
        let mut tasks = vec![];
        let quorum_driver_api = test_cluster.quorum_driver_api().clone();
        // Reorder the nodes so that the last node has the largest stake.
        let validator_with_max_stake = test_cluster
            .iota_client()
            .governance_api()
            .get_committee_info(None)
            .await
            .unwrap()
            .validators
            .iter()
            .max_by(|a, b| a.0.cmp(&b.0))
            .unwrap()
            .0;
        let node_with_max_stake = test_cluster
            .swarm
            .active_validators()
            .find(|v| v.config().protocol_public_key() == validator_with_max_stake)
            .unwrap();
        let other_nodes = test_cluster
            .swarm
            .active_validators()
            .filter(|v| v.config().protocol_public_key() != validator_with_max_stake)
            .collect::<Vec<_>>();
        let reordered_nodes = other_nodes
            .iter()
            .chain(std::iter::once(&node_with_max_stake));
        for (node, kp) in reordered_nodes.zip(self.bridge_authority_keys.iter()) {
            let validator_address = node.config().iota_address();
            // create committee registration tx
            let gas = test_cluster
                .wallet
                .get_one_gas_object_owned_by_address(validator_address)
                .await
                .unwrap()
                .unwrap();

            let server_port = get_available_port("127.0.0.1");
            let server_url = format!("http://127.0.0.1:{}", server_port);
            server_ports.push(server_port);
            let data = build_committee_register_transaction(
                validator_address,
                &gas,
                bridge_arg,
                kp.public().as_bytes().to_vec(),
                &server_url,
                ref_gas_price,
                1000000000,
            )
            .unwrap();

            let tx = Transaction::from_data_and_signer(
                data,
                vec![node.config().account_key_pair.keypair()],
            );
            let api_clone = quorum_driver_api.clone();
            tasks.push(async move {
                api_clone
                    .execute_transaction_block(
                        tx,
                        IotaTransactionBlockResponseOptions::new().with_effects(),
                        None,
                    )
                    .await
            });
        }

        if self.deploy_tokens {
            let timer = Instant::now();
            let token_ids = vec![TOKEN_ID_BTC, TOKEN_ID_ETH, TOKEN_ID_USDC, TOKEN_ID_USDT];
            let token_prices = vec![500_000_000u64, 30_000_000u64, 1_000u64, 1_000u64];
            let action = publish_and_register_coins_return_add_coins_on_iota_action(
                test_cluster.wallet(),
                bridge_arg,
                vec![
                    Path::new("../../bridge/move/tokens/btc").into(),
                    Path::new("../../bridge/move/tokens/eth").into(),
                    Path::new("../../bridge/move/tokens/usdc").into(),
                    Path::new("../../bridge/move/tokens/usdt").into(),
                ],
                token_ids,
                token_prices,
                0,
            );
            let action = action.await;
            info!("register tokens took {:?} secs", timer.elapsed().as_secs());
            let sig_map = self
                .bridge_authority_keys
                .iter()
                .map(|key| {
                    (
                        key.public().into(),
                        BridgeAuthoritySignInfo::new(&action, key).signature,
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let certified_action = CertifiedBridgeAction::new_from_data_and_sig(
                action,
                BridgeCommitteeValiditySignInfo {
                    signatures: sig_map.clone(),
                },
            );
            let verifired_action_cert =
                VerifiedCertifiedBridgeAction::new_from_verified(certified_action);
            let sender_address = test_cluster.get_address_0();

            await_committee_register_tasks(&test_cluster, tasks).await;

            // Wait until committee is set up
            trigger_reconfiguration_if_not_yet_and_assert_bridge_committee_initialized(
                &test_cluster,
            )
            .await;

            let tx = build_add_tokens_on_iota_transaction(
                sender_address,
                &test_cluster
                    .wallet
                    .get_one_gas_object_owned_by_address(sender_address)
                    .await
                    .unwrap()
                    .unwrap(),
                verifired_action_cert,
                bridge_arg,
                ref_gas_price,
            )
            .unwrap();

            let response = test_cluster.sign_and_execute_transaction(&tx).await;
            assert_eq!(
                response.effects.unwrap().status(),
                &IotaExecutionStatus::Success
            );
            info!("Deploy tokens took {:?} secs", timer.elapsed().as_secs());
        } else {
            await_committee_register_tasks(&test_cluster, tasks).await;
        }
        async fn await_committee_register_tasks(
            test_cluster: &TestCluster,
            tasks: Vec<
                impl Future<Output = Result<IotaTransactionBlockResponse, iota_sdk::error::Error>>,
            >,
        ) {
            // The tx may fail if a member tries to register when the committee is already finalized.
            // In that case, we just need to check the committee members is not empty since once
            // the committee is finalized, it should not be empty.
            let responses = join_all(tasks).await;
            let mut has_failure = false;
            for response in responses {
                if response.unwrap().effects.unwrap().status() != &IotaExecutionStatus::Success {
                    has_failure = true;
                }
            }
            if has_failure {
                let bridge_summary = get_bridge_summary(test_cluster).await;
                assert_ne!(bridge_summary.committee.members.len(), 0);
            }
        }

        info!(
            "TestCluster build_with_bridge took {:?} secs",
            timer.elapsed().as_secs()
        );
        TestClusterWrapper {
            inner: test_cluster,
            bridge_authority_keys: self.bridge_authority_keys,
            bridge_server_ports: server_ports,
        }
    }
}

impl Default for TestClusterWrapperBuilder {
    fn default() -> Self {
        Self::new()
    }
}
pub struct TestClusterWrapper {
    pub inner: TestCluster,
    pub bridge_authority_keys: Vec<BridgeAuthorityKeyPair>,
    pub bridge_server_ports: Vec<u16>,
}

impl TestClusterWrapper {
    pub fn authority_keys_clone(&self) -> Vec<BridgeAuthorityKeyPair> {
        self.bridge_authority_keys
            .iter()
            .map(|k| k.copy())
            .collect()
    }

    pub async fn trigger_reconfiguration_if_not_yet_and_assert_bridge_committee_initialized(&self) {
        trigger_reconfiguration_if_not_yet_and_assert_bridge_committee_initialized(&self.inner)
            .await
    }

    // Wait for bridge node in the cluster to be up and running.
    pub async fn wait_for_bridge_cluster_to_be_up(&self, timeout_sec: u64) {
        let bridge_ports = self.bridge_server_ports.clone();
        let mut tasks = vec![];
        for port in bridge_ports.iter() {
            let server_url = format!("http://127.0.0.1:{}", port);
            tasks.push(wait_for_server_to_be_up(server_url, timeout_sec));
        }
        join_all(tasks)
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()
            .unwrap();
    }

    pub async fn get_mut_bridge_arg(&self) -> Option<ObjectArg> {
        get_mut_bridge_arg(&self.inner).await
    }

    pub async fn get_bridge_summary(&self) -> BridgeSummary {
        get_bridge_summary(&self.inner).await
    }
}

async fn get_bridge_summary(test_cluster: &TestCluster) -> BridgeSummary {
    test_cluster
        .iota_client()
        .http()
        .get_latest_bridge()
        .await
        .unwrap()
}

async fn get_mut_bridge_arg(test_cluster: &TestCluster) -> Option<ObjectArg> {
    get_bridge_obj_initial_shared_version(
        test_cluster
            .fullnode_handle
            .iota_node
            .state()
            .get_object_store(),
    )
    .unwrap()
    .map(|seq| ObjectArg::SharedObject {
        id: IOTA_BRIDGE_OBJECT_ID,
        initial_shared_version: seq,
        mutable: true,
    })
}

async fn trigger_reconfiguration_if_not_yet_and_assert_bridge_committee_initialized(
    test_cluster: &TestCluster,
) {
    let mut bridge = get_bridge(
        test_cluster
            .fullnode_handle
            .iota_node
            .state()
            .get_object_store(),
    )
    .unwrap();
    if !bridge.committee().members.contents.is_empty() {
        assert_eq!(
            test_cluster.swarm.active_validators().count(),
            bridge.committee().members.contents.len()
        );
        return;
    }
    // wait for next epoch
    test_cluster.trigger_reconfiguration().await;
    bridge = get_bridge(
        test_cluster
            .fullnode_handle
            .iota_node
            .state()
            .get_object_store(),
    )
    .unwrap();
    // Committee should be initiated
    assert!(bridge.committee().member_registrations.contents.is_empty());
    assert_eq!(
        test_cluster.swarm.active_validators().count(),
        bridge.committee().members.contents.len()
    );
}

pub async fn initiate_bridge_eth_to_iota(
    bridge_test_cluster: &BridgeTestCluster,
    amount: u64,
    nonce: u64,
) -> Result<(), anyhow::Error> {
    info!("Depositing native Ether to Solidity contract, nonce: {nonce}, amount: {amount}");
    let (eth_signer, eth_address) = bridge_test_cluster
        .get_eth_signer_and_address()
        .await
        .unwrap();

    let iota_address = bridge_test_cluster.iota_user_address();
    let iota_chain_id = bridge_test_cluster.iota_chain_id();
    let eth_chain_id = bridge_test_cluster.eth_chain_id();
    let token_id = TOKEN_ID_ETH;

    let iota_amount = (U256::from(amount) * U256::exp10(8)).as_u64(); // DP for Ether on Iota

    let eth_tx = deposit_native_eth_to_sol_contract(
        &eth_signer,
        bridge_test_cluster.contracts().iota_bridge,
        iota_address,
        iota_chain_id,
        amount,
    )
    .await;
    let tx_receipt = send_eth_tx_and_get_tx_receipt(eth_tx).await;
    let eth_bridge_event = tx_receipt
        .logs
        .iter()
        .find_map(EthBridgeEvent::try_from_log)
        .unwrap();
    let EthBridgeEvent::EthIotaBridgeEvents(EthIotaBridgeEvents::TokensDepositedFilter(
        eth_bridge_event,
    )) = eth_bridge_event
    else {
        unreachable!();
    };
    // assert eth log matches
    assert_eq!(eth_bridge_event.source_chain_id, eth_chain_id as u8);
    assert_eq!(eth_bridge_event.nonce, nonce);
    assert_eq!(eth_bridge_event.destination_chain_id, iota_chain_id as u8);
    assert_eq!(eth_bridge_event.token_id, token_id);
    assert_eq!(eth_bridge_event.iota_adjusted_amount, iota_amount);
    assert_eq!(eth_bridge_event.sender_address, eth_address);
    assert_eq!(eth_bridge_event.recipient_address, iota_address.to_vec());
    info!(
        "Deposited Eth to Solidity contract, block: {:?}",
        tx_receipt.block_number
    );

    wait_for_transfer_action_status(
        bridge_test_cluster.bridge_client(),
        eth_chain_id,
        nonce,
        BridgeActionStatus::Claimed,
    )
    .await
    .tap_ok(|_| {
        info!("Eth to Iota bridge transfer claimed");
    })
}

pub async fn initiate_bridge_iota_to_eth(
    bridge_test_cluster: &BridgeTestCluster,
    eth_address: EthAddress,
    token: ObjectRef,
    nonce: u64,
    iota_amount: u64,
) -> Result<IotaToEthBridgeAction, anyhow::Error> {
    let bridge_object_arg = bridge_test_cluster
        .bridge_client()
        .get_mutable_bridge_object_arg_must_succeed()
        .await;
    let iota_client = bridge_test_cluster.iota_client();
    let token_types = bridge_test_cluster
        .bridge_client()
        .get_token_id_map()
        .await
        .unwrap();
    let iota_address = bridge_test_cluster.iota_user_address();

    let resp = match deposit_eth_to_iota_package(
        iota_client,
        iota_address,
        bridge_test_cluster.wallet(),
        bridge_test_cluster.eth_chain_id(),
        eth_address,
        token,
        bridge_object_arg,
        &token_types,
    )
    .await
    {
        Ok(resp) => {
            if !resp.status_ok().unwrap() {
                return Err(anyhow!("Iota TX error"));
            } else {
                resp
            }
        }
        Err(e) => return Err(e),
    };

    let iota_events = resp.events.unwrap().data;
    let bridge_event = iota_events
        .iter()
        .filter_map(|e| {
            let iota_bridge_event = IotaBridgeEvent::try_from_iota_event(e).unwrap()?;
            iota_bridge_event.try_into_bridge_action(e.id.tx_digest, e.id.event_seq as u16)
        })
        .find_map(|e| {
            if let BridgeAction::IotaToEthBridgeAction(a) = e {
                Some(a)
            } else {
                None
            }
        })
        .unwrap();
    info!("Deposited Eth to move package");
    assert_eq!(bridge_event.iota_bridge_event.nonce, nonce);
    assert_eq!(
        bridge_event.iota_bridge_event.iota_chain_id,
        bridge_test_cluster.iota_chain_id()
    );
    assert_eq!(
        bridge_event.iota_bridge_event.eth_chain_id,
        bridge_test_cluster.eth_chain_id()
    );
    assert_eq!(bridge_event.iota_bridge_event.iota_address, iota_address);
    assert_eq!(bridge_event.iota_bridge_event.eth_address, eth_address);
    assert_eq!(bridge_event.iota_bridge_event.token_id, TOKEN_ID_ETH);
    assert_eq!(
        bridge_event.iota_bridge_event.amount_iota_adjusted,
        iota_amount
    );

    // Wait for the bridge action to be approved
    wait_for_transfer_action_status(
        bridge_test_cluster.bridge_client(),
        bridge_test_cluster.iota_chain_id(),
        nonce,
        BridgeActionStatus::Approved,
    )
    .await
    .unwrap();
    info!("Iota to Eth bridge transfer approved.");

    Ok(bridge_event)
}

async fn wait_for_transfer_action_status(
    iota_bridge_client: &IotaBridgeClient,
    chain_id: BridgeChainId,
    nonce: u64,
    status: BridgeActionStatus,
) -> Result<(), anyhow::Error> {
    // Wait for the bridge action to be approved
    let now = std::time::Instant::now();
    info!(
        "Waiting for onchain status {:?}. chain: {:?}, nonce: {nonce}",
        status, chain_id as u8
    );
    loop {
        let timer = std::time::Instant::now();
        let res = iota_bridge_client
            .get_token_transfer_action_onchain_status_until_success(chain_id as u8, nonce)
            .await;
        info!(
            "get_token_transfer_action_onchain_status_until_success took {:?}, status: {:?}",
            timer.elapsed(),
            res
        );

        if res == status {
            info!(
                "detected on chain status {:?}. chain: {:?}, nonce: {nonce}",
                status, chain_id as u8
            );
            return Ok(());
        }
        if now.elapsed().as_secs() > 60 {
            return Err(anyhow!(
                "Timeout waiting for token transfer action to be {:?}. chain_id: {chain_id:?}, nonce: {nonce}. Time elapsed: {:?}",
                status,
                now.elapsed(),
            ));
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

async fn deposit_eth_to_iota_package(
    iota_client: &IotaClient,
    iota_address: IotaAddress,
    wallet_context: &WalletContext,
    target_chain: BridgeChainId,
    target_address: EthAddress,
    token: ObjectRef,
    bridge_object_arg: ObjectArg,
    iota_token_type_tags: &HashMap<u8, TypeTag>,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let mut builder = ProgrammableTransactionBuilder::new();
    let arg_target_chain = builder.pure(target_chain as u8).unwrap();
    let arg_target_address = builder.pure(target_address.as_bytes()).unwrap();
    let arg_token = builder.obj(ObjectArg::ImmOrOwnedObject(token)).unwrap();
    let arg_bridge = builder.obj(bridge_object_arg).unwrap();

    builder.programmable_move_call(
        BRIDGE_PACKAGE_ID,
        BRIDGE_MODULE_NAME.to_owned(),
        ident_str!("send_token").to_owned(),
        vec![iota_token_type_tags.get(&TOKEN_ID_ETH).unwrap().clone()],
        vec![arg_bridge, arg_target_chain, arg_target_address, arg_token],
    );

    let pt = builder.finish();
    let gas_object_ref = wallet_context
        .get_one_gas_object_owned_by_address(iota_address)
        .await
        .unwrap()
        .unwrap();
    let tx_data = TransactionData::new_programmable(
        iota_address,
        vec![gas_object_ref],
        pt,
        500_000_000,
        iota_client
            .governance_api()
            .get_reference_gas_price()
            .await
            .unwrap(),
    );
    let tx = wallet_context.sign_transaction(&tx_data);
    wallet_context.execute_transaction_may_fail(tx).await
}

pub async fn initiate_bridge_erc20_to_iota(
    bridge_test_cluster: &BridgeTestCluster,
    amount_u64: u64,
    token_address: EthAddress,
    token_id: u8,
    nonce: u64,
) -> Result<(), anyhow::Error> {
    let (eth_signer, eth_address) = bridge_test_cluster
        .get_eth_signer_and_address()
        .await
        .unwrap();

    // First, mint ERC20 tokens to the signer
    let contract = EthERC20::new(token_address, eth_signer.clone().into());
    let decimal = contract.decimals().await? as usize;
    let amount = U256::from(amount_u64) * U256::exp10(decimal);
    let iota_amount = amount.as_u64();
    let mint_call = contract.mint(eth_address, amount);
    let mint_tx_receipt = send_eth_tx_and_get_tx_receipt(mint_call).await;
    assert_eq!(mint_tx_receipt.status.unwrap().as_u64(), 1);

    // Second, set allowance
    let allowance_call = contract.approve(bridge_test_cluster.contracts().iota_bridge, amount);
    let allowance_tx_receipt = send_eth_tx_and_get_tx_receipt(allowance_call).await;
    assert_eq!(allowance_tx_receipt.status.unwrap().as_u64(), 1);

    // Third, deposit to bridge
    let iota_recipient_address = bridge_test_cluster.iota_user_address();
    let iota_chain_id = bridge_test_cluster.iota_chain_id();
    let eth_chain_id = bridge_test_cluster.eth_chain_id();

    info!(
        "Depositing ERC20 (token id:{}, token_address: {}) to Solidity contract",
        token_id, token_address
    );
    let contract = EthIotaBridge::new(
        bridge_test_cluster.contracts().iota_bridge,
        eth_signer.clone().into(),
    );
    let deposit_call = contract.bridge_erc20(
        token_id,
        amount,
        iota_recipient_address.to_vec().into(),
        iota_chain_id as u8,
    );
    let tx_receipt = send_eth_tx_and_get_tx_receipt(deposit_call).await;
    let eth_bridge_event = tx_receipt
        .logs
        .iter()
        .find_map(EthBridgeEvent::try_from_log)
        .unwrap();
    let EthBridgeEvent::EthIotaBridgeEvents(EthIotaBridgeEvents::TokensDepositedFilter(
        eth_bridge_event,
    )) = eth_bridge_event
    else {
        unreachable!();
    };
    // assert eth log matches
    assert_eq!(eth_bridge_event.source_chain_id, eth_chain_id as u8);
    assert_eq!(eth_bridge_event.nonce, nonce);
    assert_eq!(eth_bridge_event.destination_chain_id, iota_chain_id as u8);
    assert_eq!(eth_bridge_event.token_id, token_id);
    assert_eq!(eth_bridge_event.iota_adjusted_amount, iota_amount);
    assert_eq!(eth_bridge_event.sender_address, eth_address);
    assert_eq!(
        eth_bridge_event.recipient_address,
        iota_recipient_address.to_vec()
    );
    info!(
        "Deposited ERC20 (token id:{}, token_address: {}) to Solidity contract",
        token_id, token_address
    );

    wait_for_transfer_action_status(
        bridge_test_cluster.bridge_client(),
        eth_chain_id,
        nonce,
        BridgeActionStatus::Claimed,
    )
    .await
    .tap_ok(|_| {
        info!(
            nonce,
            token_id, amount_u64, "Eth to Iota bridge transfer claimed"
        );
    })
}

pub(crate) async fn deposit_native_eth_to_sol_contract(
    signer: &EthSigner,
    contract_address: EthAddress,
    iota_recipient_address: IotaAddress,
    iota_chain_id: BridgeChainId,
    amount: u64,
) -> ContractCall<EthSigner, ()> {
    let contract = EthIotaBridge::new(contract_address, signer.clone().into());
    let iota_recipient_address = iota_recipient_address.to_vec().into();
    let amount = U256::from(amount) * U256::exp10(18); // 1 ETH
    contract
        .bridge_eth(iota_recipient_address, iota_chain_id as u8)
        .value(amount)
}
