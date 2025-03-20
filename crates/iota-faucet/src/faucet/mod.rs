// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simple_faucet;
mod write_ahead_log;

use std::{net::Ipv4Addr, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use clap::Parser;
use iota_types::base_types::{IotaAddress, ObjectID, TransactionDigest};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::simple_faucet::SimpleFaucet;
use crate::FaucetError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FaucetReceipt {
    pub sent: Vec<CoinInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BatchFaucetReceipt {
    pub task: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoinInfo {
    pub amount: u64,
    pub id: ObjectID,
    pub transfer_tx_digest: TransactionDigest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BatchSendStatus {
    pub status: BatchSendStatusType,
    pub transferred_gas_objects: Option<FaucetReceipt>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum BatchSendStatusType {
    INPROGRESS,
    SUCCEEDED,
    DISCARDED,
}

pub struct AppState<F = Arc<SimpleFaucet>> {
    pub faucet: F,
    pub config: FaucetConfig,
}

impl<F> AppState<F> {
    pub fn new(faucet: F, config: FaucetConfig) -> Self {
        Self { faucet, config }
    }
}

#[async_trait]
pub trait Faucet {
    /// Send `Coin<IOTA>` of the specified amount to the recipient
    async fn send(
        &self,
        id: Uuid,
        recipient: IotaAddress,
        amounts: &[u64],
    ) -> Result<FaucetReceipt, FaucetError>;

    /// Send `Coin<IOTA>` of the specified amount to the recipient in a batch
    /// request
    async fn batch_send(
        &self,
        id: Uuid,
        recipient: IotaAddress,
        amounts: &[u64],
    ) -> Result<BatchFaucetReceipt, FaucetError>;

    /// Get the status of a batch_send request
    async fn get_batch_send_status(&self, task_id: Uuid) -> Result<BatchSendStatus, FaucetError>;
}

pub const DEFAULT_AMOUNT: u64 = 1_000_000_000;
pub const DEFAULT_NUM_OF_COINS: usize = 1;

#[derive(Parser, Clone)]
#[command(
    name = "IOTA Faucet",
    about = "Faucet for requesting test tokens on IOTA"
)]
pub struct FaucetConfig {
    #[arg(long, default_value_t = 5003)]
    pub port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    pub host_ip: Ipv4Addr,

    #[arg(long, default_value_t = DEFAULT_AMOUNT)]
    pub amount: u64,

    #[arg(long, default_value_t = DEFAULT_NUM_OF_COINS)]
    pub num_coins: usize,

    #[arg(long, default_value_t = 10)]
    pub request_buffer_size: usize,

    #[arg(long, default_value_t = 10)]
    pub max_request_per_second: u64,

    #[arg(long, default_value_t = 60)]
    pub wallet_client_timeout_secs: u64,

    #[arg(long)]
    pub write_ahead_log: PathBuf,

    #[arg(long, default_value_t = 300)]
    pub wal_retry_interval: u64,

    #[arg(long, default_value_t = 10000)]
    pub max_request_queue_length: u64,

    #[arg(long, default_value_t = 500)]
    pub batch_request_size: u64,

    #[arg(long, default_value_t = 300)]
    pub ttl_expiration: u64,

    #[arg(long, action = clap::ArgAction::Set, default_value_t = false)]
    pub batch_enabled: bool,
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            port: 5003,
            host_ip: Ipv4Addr::new(127, 0, 0, 1),
            amount: DEFAULT_AMOUNT,
            num_coins: DEFAULT_NUM_OF_COINS,
            request_buffer_size: 10,
            max_request_per_second: 10,
            wallet_client_timeout_secs: 60,
            write_ahead_log: Default::default(),
            wal_retry_interval: 300,
            max_request_queue_length: 10000,
            batch_request_size: 500,
            ttl_expiration: 300,
            batch_enabled: false,
        }
    }
}
