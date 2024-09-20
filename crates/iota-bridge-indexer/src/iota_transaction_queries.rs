// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_bridge::{metrics::BridgeMetrics, retry_with_max_elapsed_time};
use iota_json_rpc_types::{
    IotaTransactionBlockResponseOptions, IotaTransactionBlockResponseQuery, TransactionFilter,
};
use iota_sdk::IotaClient;
use iota_types::{digests::TransactionDigest, IOTA_BRIDGE_OBJECT_ID};
use tracing::{error, info};

use crate::types::RetrievedTransaction;

const QUERY_DURATION: Duration = Duration::from_secs(1);
const SLEEP_DURATION: Duration = Duration::from_secs(5);

pub async fn start_iota_tx_polling_task(
    iota_client: IotaClient,
    mut cursor: Option<TransactionDigest>,
    tx: iota_metrics::metered_channel::Sender<(
        Vec<RetrievedTransaction>,
        Option<TransactionDigest>,
    )>,
    metrics: Arc<BridgeMetrics>,
) {
    info!("Starting IOTA transaction polling task from {:?}", cursor);
    loop {
        let Ok(Ok(results)) = retry_with_max_elapsed_time!(
            iota_client.read_api().query_transaction_blocks(
                IotaTransactionBlockResponseQuery {
                    filter: Some(TransactionFilter::InputObject(IOTA_BRIDGE_OBJECT_ID)),
                    options: Some(IotaTransactionBlockResponseOptions::full_content()),
                },
                cursor,
                None,
                false,
            ),
            Duration::from_secs(600)
        ) else {
            error!("Failed to query bridge transactions after retry");
            continue;
        };
        info!("Retrieved {} bridge transactions", results.data.len());
        let txes = match results
            .data
            .into_iter()
            .map(RetrievedTransaction::try_from)
            .collect::<anyhow::Result<Vec<_>>>()
        {
            Ok(data) => data,
            Err(e) => {
                // TODO: Sometimes fullnode does not return checkpoint strangely. We retry
                // instead of panicking.
                error!(
                    "Failed to convert retrieved transactions to sanitized format: {}",
                    e
                );
                tokio::time::sleep(SLEEP_DURATION).await;
                continue;
            }
        };
        if txes.is_empty() {
            // When there is no more new data, we are caught up, no need to stress the
            // fullnode
            tokio::time::sleep(QUERY_DURATION).await;
            continue;
        }
        // Unwrap: txes is not empty
        let ckp = txes.last().unwrap().checkpoint;
        tx.send((txes, results.next_cursor))
            .await
            .expect("Failed to send transaction block to process");
        metrics.last_synced_iota_checkpoint.set(ckp as i64);
        cursor = results.next_cursor;
    }
}
