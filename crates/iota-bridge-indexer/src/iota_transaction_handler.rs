// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use anyhow::Result;
use futures::StreamExt;
use iota_bridge::events::{
    MoveTokenDepositedEvent, MoveTokenTransferApproved, MoveTokenTransferClaimed,
};
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_metrics::metered_channel::{Receiver, ReceiverStream};
use iota_types::{BRIDGE_ADDRESS, digests::TransactionDigest};
use tracing::{error, info};

use crate::{
    BridgeDataSource, ProcessedTxnData, TokenTransfer, TokenTransferData, TokenTransferStatus,
    metrics::BridgeIndexerMetrics,
    postgres_manager::{PgPool, update_iota_progress_store, write},
    types::RetrievedTransaction,
};

pub(crate) const COMMIT_BATCH_SIZE: usize = 10;

pub async fn handle_iota_transactions_loop(
    pg_pool: PgPool,
    rx: Receiver<(Vec<RetrievedTransaction>, Option<TransactionDigest>)>,
    metrics: BridgeIndexerMetrics,
) {
    let checkpoint_commit_batch_size = std::env::var("COMMIT_BATCH_SIZE")
        .unwrap_or(COMMIT_BATCH_SIZE.to_string())
        .parse::<usize>()
        .unwrap();
    let mut stream = ReceiverStream::new(rx).ready_chunks(checkpoint_commit_batch_size);
    while let Some(batch) = stream.next().await {
        // unwrap: batch must not be empty
        let (txns, cursor) = batch.last().cloned().unwrap();
        let data = batch
            .into_iter()
            // TODO: letting it panic so we can capture errors, but we should handle this more
            // gracefully
            .flat_map(|(chunk, _)| process_transactions(chunk, &metrics).unwrap())
            .collect::<Vec<_>>();

        // write batched transaction data to DB
        if !data.is_empty() {
            // unwrap: token_transfers is not empty
            let last_ckp = txns.last().map(|tx| tx.checkpoint).unwrap_or_default();
            while let Err(err) = write(&pg_pool, data.clone()) {
                error!("Failed to write iota transactions to DB: {:?}", err);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            info!("Wrote {} bridge transaction data to DB", data.len());
            metrics.last_committed_iota_checkpoint.set(last_ckp as i64);
        }

        // update iota progress store using the latest cursor
        if let Some(cursor) = cursor {
            while let Err(err) = update_iota_progress_store(&pg_pool, cursor) {
                error!("Failed to update iota progress tore DB: {:?}", err);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            info!("Updated iota transaction cursor to {}", cursor);
        }
    }
    unreachable!("Channel closed unexpectedly");
}

fn process_transactions(
    txns: Vec<RetrievedTransaction>,
    metrics: &BridgeIndexerMetrics,
) -> Result<Vec<ProcessedTxnData>> {
    txns.into_iter().try_fold(vec![], |mut result, tx| {
        result.append(&mut into_token_transfers(tx, metrics)?);
        Ok(result)
    })
}

pub fn into_token_transfers(
    tx: RetrievedTransaction,
    metrics: &BridgeIndexerMetrics,
) -> Result<Vec<ProcessedTxnData>> {
    let mut transfers = Vec::new();
    let tx_digest = tx.tx_digest;
    let timestamp_ms = tx.timestamp_ms;
    let checkpoint_num = tx.checkpoint;
    let effects = tx.effects;
    for ev in tx.events.data {
        if ev.type_.address != BRIDGE_ADDRESS {
            continue;
        }
        match ev.type_.name.as_str() {
            "TokenDepositedEvent" => {
                info!("Observed IOTA Deposit {:?}", ev);
                metrics.total_iota_token_deposited.inc();
                let move_event: MoveTokenDepositedEvent = bcs::from_bytes(ev.bcs.bytes())?;
                transfers.push(ProcessedTxnData::TokenTransfer(TokenTransfer {
                    chain_id: move_event.source_chain,
                    nonce: move_event.seq_num,
                    block_height: checkpoint_num,
                    timestamp_ms,
                    txn_hash: tx_digest.inner().to_vec(),
                    txn_sender: ev.sender.to_vec(),
                    status: TokenTransferStatus::Deposited,
                    gas_usage: effects.gas_cost_summary().net_gas_usage(),
                    data_source: BridgeDataSource::Iota,
                    data: Some(TokenTransferData {
                        destination_chain: move_event.target_chain,
                        sender_address: move_event.sender_address.clone(),
                        recipient_address: move_event.target_address.clone(),
                        token_id: move_event.token_type,
                        amount: move_event.amount_iota_adjusted,
                    }),
                }));
            }
            "TokenTransferApproved" => {
                info!("Observed IOTA Approval {:?}", ev);
                metrics.total_iota_token_transfer_approved.inc();
                let event: MoveTokenTransferApproved = bcs::from_bytes(ev.bcs.bytes())?;
                transfers.push(ProcessedTxnData::TokenTransfer(TokenTransfer {
                    chain_id: event.message_key.source_chain,
                    nonce: event.message_key.bridge_seq_num,
                    block_height: checkpoint_num,
                    timestamp_ms,
                    txn_hash: tx_digest.inner().to_vec(),
                    txn_sender: ev.sender.to_vec(),
                    status: TokenTransferStatus::Approved,
                    gas_usage: effects.gas_cost_summary().net_gas_usage(),
                    data_source: BridgeDataSource::Iota,
                    data: None,
                }));
            }
            "TokenTransferClaimed" => {
                info!("Observed IOTA Claim {:?}", ev);
                metrics.total_iota_token_transfer_claimed.inc();
                let event: MoveTokenTransferClaimed = bcs::from_bytes(ev.bcs.bytes())?;
                transfers.push(ProcessedTxnData::TokenTransfer(TokenTransfer {
                    chain_id: event.message_key.source_chain,
                    nonce: event.message_key.bridge_seq_num,
                    block_height: checkpoint_num,
                    timestamp_ms,
                    txn_hash: tx_digest.inner().to_vec(),
                    txn_sender: ev.sender.to_vec(),
                    status: TokenTransferStatus::Claimed,
                    gas_usage: effects.gas_cost_summary().net_gas_usage(),
                    data_source: BridgeDataSource::Iota,
                    data: None,
                }));
            }
            _ => {
                metrics.total_iota_bridge_txn_other.inc();
            }
        }
    }
    if !transfers.is_empty() {
        info!(
            ?tx_digest,
            "IOTA: Extracted {} bridge token transfer data entries",
            transfers.len(),
        );
    }
    Ok(transfers)
}
