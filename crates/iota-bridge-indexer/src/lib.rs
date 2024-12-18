// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    sync::Arc,
};

use ethers::{
    providers::{Http, Provider},
    types::Address as EthAddress,
};
use iota_bridge::{
    eth_client::EthClient, metered_eth_provider::MeteredEthHttpProvier, metrics::BridgeMetrics,
    utils::get_eth_contract_addresses,
};
use iota_data_ingestion_core::DataIngestionMetrics;
use iota_indexer_builder::{
    indexer_builder::{BackfillStrategy, Datasource, Indexer, IndexerBuilder},
    iota_datasource::IotaCheckpointDatasource,
    metrics::IndexerMetricProvider,
    progress::{OutOfOrderSaveAfterDurationPolicy, ProgressSavingPolicy, SaveAfterDurationPolicy},
};
use iota_sdk::IotaClientBuilder;
use iota_types::base_types::{IotaAddress, TransactionDigest};
use strum_macros::Display;

use crate::{
    config::IndexerConfig,
    eth_bridge_indexer::{EthDataMapper, EthFinalizedSyncDatasource, EthSubscriptionDatasource},
    iota_bridge_indexer::IotaBridgeDataMapper,
    metrics::BridgeIndexerMetrics,
    models::{
        GovernanceAction as DBGovernanceAction, IotaErrorTransactions,
        TokenTransfer as DBTokenTransfer, TokenTransferData as DBTokenTransferData,
    },
    postgres_manager::PgPool,
    storage::PgBridgePersistent,
};

pub mod config;
pub mod iota_transaction_handler;
pub mod iota_transaction_queries;
pub mod metrics;
pub mod models;
pub mod postgres_manager;
pub mod schema;
pub mod storage;
pub mod types;

pub mod eth_bridge_indexer;
pub mod iota_bridge_indexer;

#[derive(Clone)]
pub enum ProcessedTxnData {
    TokenTransfer(TokenTransfer),
    GovernanceAction(GovernanceAction),
    Error(IotaTxnError),
}

#[derive(Clone)]
pub struct IotaTxnError {
    tx_digest: TransactionDigest,
    sender: IotaAddress,
    timestamp_ms: u64,
    failure_status: String,
    cmd_idx: Option<u64>,
}

#[derive(Clone)]
pub struct TokenTransfer {
    chain_id: u8,
    nonce: u64,
    block_height: u64,
    timestamp_ms: u64,
    txn_hash: Vec<u8>,
    txn_sender: Vec<u8>,
    status: TokenTransferStatus,
    gas_usage: i64,
    data_source: BridgeDataSource,
    data: Option<TokenTransferData>,
    is_finalized: bool,
}

#[derive(Clone)]
pub struct GovernanceAction {
    nonce: Option<u64>,
    data_source: BridgeDataSource,
    tx_digest: Vec<u8>,
    sender: Vec<u8>,
    timestamp_ms: u64,
    action: GovernanceActionType,
    data: serde_json::Value,
}

#[derive(Clone)]
pub struct TokenTransferData {
    sender_address: Vec<u8>,
    destination_chain: u8,
    recipient_address: Vec<u8>,
    token_id: u8,
    amount: u64,
    is_finalized: bool,
}

impl TokenTransfer {
    fn to_db(&self) -> DBTokenTransfer {
        DBTokenTransfer {
            chain_id: self.chain_id as i32,
            nonce: self.nonce as i64,
            block_height: self.block_height as i64,
            timestamp_ms: self.timestamp_ms as i64,
            txn_hash: self.txn_hash.clone(),
            txn_sender: self.txn_sender.clone(),
            status: self.status.to_string(),
            gas_usage: self.gas_usage,
            data_source: self.data_source.to_string(),
            is_finalized: self.is_finalized,
        }
    }

    fn to_data_maybe(&self) -> Option<DBTokenTransferData> {
        self.data.as_ref().map(|data| DBTokenTransferData {
            chain_id: self.chain_id as i32,
            nonce: self.nonce as i64,
            block_height: self.block_height as i64,
            timestamp_ms: self.timestamp_ms as i64,
            txn_hash: self.txn_hash.clone(),
            sender_address: data.sender_address.clone(),
            destination_chain: data.destination_chain as i32,
            recipient_address: data.recipient_address.clone(),
            token_id: data.token_id as i32,
            amount: data.amount as i64,
            is_finalized: data.is_finalized,
        })
    }
}

impl IotaTxnError {
    fn to_db(&self) -> IotaErrorTransactions {
        IotaErrorTransactions {
            txn_digest: self.tx_digest.inner().to_vec(),
            sender_address: self.sender.to_vec(),
            timestamp_ms: self.timestamp_ms as i64,
            failure_status: self.failure_status.clone(),
            cmd_idx: self.cmd_idx.map(|idx| idx as i64),
        }
    }
}

impl GovernanceAction {
    fn to_db(&self) -> DBGovernanceAction {
        DBGovernanceAction {
            nonce: self.nonce.map(|nonce| nonce as i64),
            data_source: self.data_source.to_string(),
            txn_digest: self.tx_digest.clone(),
            sender_address: self.sender.to_vec(),
            timestamp_ms: self.timestamp_ms as i64,
            action: self.action.to_string(),
            data: self.data.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) enum TokenTransferStatus {
    Deposited,
    Approved,
    Claimed,
}

impl Display for TokenTransferStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            TokenTransferStatus::Deposited => "Deposited",
            TokenTransferStatus::Approved => "Approved",
            TokenTransferStatus::Claimed => "Claimed",
        };
        write!(f, "{str}")
    }
}

#[derive(Clone, Display)]
pub(crate) enum GovernanceActionType {
    UpdateCommitteeBlocklist,
    EmergencyOperation,
    UpdateBridgeLimit,
    UpdateTokenPrices,
    UpgradeEVMContract,
    AddIotaTokens,
    AddEVMTokens,
}

#[derive(Clone)]
enum BridgeDataSource {
    Iota,
    Eth,
}

impl Display for BridgeDataSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            BridgeDataSource::Eth => "ETH",
            BridgeDataSource::Iota => "IOTA",
        };
        write!(f, "{str}")
    }
}

pub async fn create_iota_indexer(
    pool: PgPool,
    metrics: BridgeIndexerMetrics,
    ingestion_metrics: DataIngestionMetrics,
    config: &IndexerConfig,
) -> anyhow::Result<
    Indexer<PgBridgePersistent, IotaCheckpointDatasource, IotaBridgeDataMapper>,
    anyhow::Error,
> {
    let datastore_with_out_of_order_source = PgBridgePersistent::new(
        pool,
        ProgressSavingPolicy::OutOfOrderSaveAfterDuration(OutOfOrderSaveAfterDurationPolicy::new(
            tokio::time::Duration::from_secs(30),
        )),
    );

    let iota_client = Arc::new(
        IotaClientBuilder::default()
            .build(config.iota_rpc_url.clone())
            .await?,
    );

    let iota_checkpoint_datasource = IotaCheckpointDatasource::new(
        config.remote_store_url.clone(),
        iota_client,
        config.concurrency as usize,
        config
            .checkpoints_path
            .clone()
            .map(|p| p.into())
            .unwrap_or(tempfile::tempdir()?.into_path()),
        config.iota_bridge_genesis_checkpoint,
        ingestion_metrics,
        metrics.clone().boxed(),
    );

    Ok(IndexerBuilder::new(
        "IotaBridgeIndexer",
        iota_checkpoint_datasource,
        IotaBridgeDataMapper { metrics },
        datastore_with_out_of_order_source,
    )
    .build())
}

pub async fn create_eth_sync_indexer(
    pool: PgPool,
    metrics: BridgeIndexerMetrics,
    bridge_metrics: Arc<BridgeMetrics>,
    config: &IndexerConfig,
    eth_client: Arc<EthClient<MeteredEthHttpProvier>>,
) -> Result<Indexer<PgBridgePersistent, EthFinalizedSyncDatasource, EthDataMapper>, anyhow::Error> {
    let bridge_addresses = get_eth_bridge_contract_addresses(config).await?;
    // Start the eth sync data source
    let eth_sync_datasource = EthFinalizedSyncDatasource::new(
        bridge_addresses,
        eth_client.clone(),
        config.eth_rpc_url.clone(),
        metrics.clone().boxed(),
        bridge_metrics.clone(),
        config.eth_bridge_genesis_block,
    )
    .await?;
    Ok(create_eth_indexer_builder(
        pool,
        metrics,
        eth_sync_datasource,
        "EthBridgeFinalizedSyncIndexer",
    )
    .await?
    .with_backfill_strategy(BackfillStrategy::Partitioned { task_size: 1000 })
    .build())
}

pub async fn create_eth_subscription_indexer(
    pool: PgPool,
    metrics: BridgeIndexerMetrics,
    config: &IndexerConfig,
    eth_client: Arc<EthClient<MeteredEthHttpProvier>>,
) -> Result<Indexer<PgBridgePersistent, EthSubscriptionDatasource, EthDataMapper>, anyhow::Error> {
    // Start the eth subscription indexer
    let bridge_addresses = get_eth_bridge_contract_addresses(config).await?;
    // Start the eth subscription indexer
    let eth_subscription_datasource = EthSubscriptionDatasource::new(
        bridge_addresses.clone(),
        eth_client.clone(),
        config.eth_ws_url.clone(),
        metrics.clone().boxed(),
        config.eth_bridge_genesis_block,
    )
    .await?;

    Ok(create_eth_indexer_builder(
        pool,
        metrics,
        eth_subscription_datasource,
        "EthBridgeSubscriptionIndexer",
    )
    .await?
    .with_backfill_strategy(BackfillStrategy::Disabled)
    .build())
}

async fn create_eth_indexer_builder<T: Send, D: Datasource<T>>(
    pool: PgPool,
    metrics: BridgeIndexerMetrics,
    datasource: D,
    indexer_name: &str,
) -> Result<IndexerBuilder<D, EthDataMapper, PgBridgePersistent>, anyhow::Error> {
    let datastore = PgBridgePersistent::new(
        pool,
        ProgressSavingPolicy::SaveAfterDuration(SaveAfterDurationPolicy::new(
            tokio::time::Duration::from_secs(30),
        )),
    );

    // Start the eth subscription indexer
    Ok(IndexerBuilder::new(
        indexer_name,
        datasource,
        EthDataMapper { metrics },
        datastore.clone(),
    ))
}

async fn get_eth_bridge_contract_addresses(
    config: &IndexerConfig,
) -> Result<Vec<EthAddress>, anyhow::Error> {
    let bridge_address = EthAddress::from_str(&config.eth_iota_bridge_contract_address)?;
    let provider = Arc::new(
        Provider::<Http>::try_from(&config.eth_rpc_url)?
            .interval(std::time::Duration::from_millis(2000)),
    );
    let bridge_addresses = get_eth_contract_addresses(bridge_address, &provider).await?;
    Ok(vec![
        bridge_address,
        bridge_addresses.0,
        bridge_addresses.1,
        bridge_addresses.2,
        bridge_addresses.3,
    ])
}
