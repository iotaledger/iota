// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_macros::sim_test;
use iota_types::{base_types::IotaAddress, digests::TransactionDigest};
use test_cluster::TestClusterBuilder;

use crate::{LocalExec, types::ReplayEngineError};

/// Spawns a local network, submits a transfer transaction, then replays it
/// using LocalExec and verifies the effects match.
#[sim_test]
async fn verify_tx_replay() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let rpc_url = test_cluster.rpc_url();

    // Advance past epoch 0 since the replay engine does not support it
    test_cluster.force_new_epoch().await;

    // Build and execute a simple transfer transaction
    let tx_data = test_cluster
        .test_transaction_builder()
        .await
        .transfer_iota(Some(1_000_000_000), IotaAddress::ZERO)
        .build();
    let response = test_cluster.sign_and_execute_transaction(&tx_data).await;
    let tx_digest = response.digest;

    // Replay with authority certificate execution
    execute_replay(rpc_url, &tx_digest, true)
        .await
        .expect("Replay with authority failed");

    // Replay with execution engine
    execute_replay(rpc_url, &tx_digest, false)
        .await
        .expect("Replay with execution engine failed");
}

async fn execute_replay(
    url: &str,
    tx: &TransactionDigest,
    use_authority: bool,
) -> Result<(), ReplayEngineError> {
    LocalExec::new_from_fn_url(url)
        .await?
        .init_for_execution()
        .await?
        .execute_transaction(
            tx,
            ExpensiveSafetyCheckConfig::default(),
            use_authority,
            None,
            None,
            None,
            None,
        )
        .await?
        .check_effects()?;
    Ok(())
}
