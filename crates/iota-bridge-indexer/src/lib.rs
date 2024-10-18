// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use iota_types::base_types::{IotaAddress, TransactionDigest};

use crate::models::{
    IotaErrorTransactions, TokenTransfer as DBTokenTransfer,
    TokenTransferData as DBTokenTransferData,
};

pub mod config;
pub mod iota_transaction_handler;
pub mod iota_transaction_queries;
pub mod metrics;
pub mod models;
pub mod postgres_manager;
pub mod schema;
pub mod types;

pub mod eth_bridge_indexer;
pub mod iota_bridge_indexer;

#[derive(Clone)]
pub enum ProcessedTxnData {
    TokenTransfer(TokenTransfer),
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
}

#[derive(Clone)]
pub struct TokenTransferData {
    sender_address: Vec<u8>,
    destination_chain: u8,
    recipient_address: Vec<u8>,
    token_id: u8,
    amount: u64,
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