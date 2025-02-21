// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_json_rpc_api::QUERY_MAX_RESULT_LIMIT;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_sdk::{IotaClient, IotaClientBuilder};
use iota_types::{base_types::IotaAddress, digests::TransactionDigest};

use crate::{
    LocalExec,
    config::ReplayableNetworkConfigSet,
    types::{MAX_CONCURRENT_REQUESTS, RPC_TIMEOUT_ERR_SLEEP_RETRY_PERIOD, ReplayEngineError},
};

/// Keep searching for non-system TXs in the checkppints for this long
/// Very unlikely to take this long, but we want to be sure we find one
const NUM_CHECKPOINTS_TO_ATTEMPT: usize = 10_000;

/// Checks that replaying the latest tx on each testnet and mainnet does not
/// fail
#[tokio::test]
#[ignore = "https://github.com/iotaledger/iota/issues/5031"]
async fn verify_latest_tx_replay_testnet_mainnet() {
    let _ = verify_latest_tx_replay_impl().await;
}

async fn verify_latest_tx_replay_impl() {
    let default_cfg = ReplayableNetworkConfigSet::default();
    let urls: Vec<_> = default_cfg
        .base_network_configs
        .iter()
        // TODO: enable this when mainnet is launched
        .filter(|q| q.name != "devnet" && q.name != "mainnet") // Devnet is not always stable, mainnet is not ready
        .map(|c| c.public_full_node.clone())
        .collect();

    let mut handles = vec![];
    for url in urls {
        handles.push(tokio::spawn(async move {
            {
                let mut num_checkpoint_trials_left = NUM_CHECKPOINTS_TO_ATTEMPT;
                let rpc_client = IotaClientBuilder::default()
                    .request_timeout(RPC_TIMEOUT_ERR_SLEEP_RETRY_PERIOD)
                    .max_concurrent_requests(MAX_CONCURRENT_REQUESTS)
                    .build(&url)
                    .await
                    .unwrap();

                let chain_id = rpc_client.read_api().get_chain_identifier().await.unwrap();

                let mut subject_checkpoint = rpc_client
                    .read_api()
                    .get_latest_checkpoint_sequence_number()
                    .await
                    .unwrap();
                let txs = rpc_client
                    .read_api()
                    .get_checkpoint(subject_checkpoint.into())
                    .await
                    .unwrap()
                    .transactions;

                let mut non_system_txs = extract_one_no_system_tx(&rpc_client, txs).await;
                num_checkpoint_trials_left -= 1;
                while non_system_txs.is_none() && num_checkpoint_trials_left > 0 {
                    num_checkpoint_trials_left -= 1;
                    subject_checkpoint -= 1;
                    let txs = rpc_client
                        .read_api()
                        .get_checkpoint(subject_checkpoint.into())
                        .await
                        .unwrap()
                        .transactions;
                    non_system_txs = extract_one_no_system_tx(&rpc_client, txs).await;
                }

                if non_system_txs.is_none() {
                    panic!(
                        "No non-system txs found in the last {} checkpoints for network {} using rpc url {}",
                        NUM_CHECKPOINTS_TO_ATTEMPT, chain_id, url
                    );
                }
                let tx: TransactionDigest = non_system_txs.unwrap();
                (url.clone(), execute_replay(&url, &tx)
                    .await
            )
            }
        }));
    }

    let rets = futures::future::join_all(handles)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("Join all failed");

    for (url, ret) in rets {
        if let Err(e) = ret {
            panic!("Replay failed for network {} with error {:?}", url, e);
        }
    }
}

async fn extract_one_no_system_tx(
    rpc_client: &IotaClient,
    mut txs: Vec<TransactionDigest>,
) -> Option<TransactionDigest> {
    let opts = IotaTransactionBlockResponseOptions::full_content();
    txs.retain(|q| *q != TransactionDigest::genesis_marker());

    for ch in txs.chunks(*QUERY_MAX_RESULT_LIMIT) {
        match rpc_client
            .read_api()
            .multi_get_transactions_with_options(ch.to_vec(), opts.clone())
            .await
            .unwrap()
            .into_iter()
            .filter_map(|x| {
                if match x.transaction.unwrap().data {
                    iota_json_rpc_types::IotaTransactionBlockData::V1(tx) => tx.sender,
                } != IotaAddress::ZERO
                {
                    Some(x.digest)
                } else {
                    None
                }
            })
            .next()
        {
            Some(tx) => {
                tokio::task::yield_now().await;
                return Some(tx);
            }
            None => {
                continue;
            }
        }
    }
    None
}

async fn execute_replay(url: &str, tx: &TransactionDigest) -> Result<(), ReplayEngineError> {
    LocalExec::new_from_fn_url(url)
        .await?
        .init_for_execution()
        .await?
        .execute_transaction(
            tx,
            ExpensiveSafetyCheckConfig::default(),
            true,
            None,
            None,
            None,
            None,
        )
        .await?
        .check_effects()?;
    tokio::task::yield_now().await;
    LocalExec::new_from_fn_url(url)
        .await?
        .init_for_execution()
        .await?
        .execute_transaction(
            tx,
            ExpensiveSafetyCheckConfig::default(),
            false,
            None,
            None,
            None,
            None,
        )
        .await?
        .check_effects()?;
    tokio::task::yield_now().await;

    Ok(())
}
