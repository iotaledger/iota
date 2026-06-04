// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end comparison: run a staking transaction through the local
//! [`LocalVm`] (objects pre-fetched over gRPC into a [`GrpcStore`]) and against
//! a live [`test_cluster::TestCluster`]'s own dry-run, then assert both agree.

use iota_json_rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffectsAPI};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::effects::TransactionEffectsAPI;
use iota_vm_sdk::{ExecuteOptions, LocalVm, grpc::GrpcStore};
use test_cluster::TestClusterBuilder;

/// The parts of a simulation result that should agree regardless of which
/// backend produced them. Gas is intentionally excluded: the local run uses
/// dev-inspect (relaxed checks) against objects pre-fetched at a possibly
/// earlier version than the node's dry-run sees, so the two ledgers can differ
/// by a rebate while the object/event changes stay identical.
#[derive(Debug, PartialEq, Eq)]
struct SimulationSummary {
    success: bool,
    created_count: usize,
    mutated_count: usize,
    deleted_count: usize,
    events_count: usize,
}

/// Build a staking transaction, simulate it with both the node's dry-run and
/// the local VM, and assert the two produce the same effects summary.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compare_local_vm_staking_against_test_cluster() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;

    test_cluster.wait_for_checkpoint(1, None).await;

    let validator = test_cluster
        .iota_client()
        .governance_api()
        .get_latest_iota_system_state()
        .await
        .expect("should get system state")
        .active_validators()
        .first()
        .expect("should have at least one validator")
        .iota_address;

    // One coin pays for gas, a second is staked.
    let sender = test_cluster.wallet.get_addresses()[0];
    let rgp = test_cluster.get_reference_gas_price().await;
    let all_coins = test_cluster
        .wallet
        .get_gas_objects_owned_by_address(sender, 5)
        .await
        .unwrap();
    assert!(
        all_coins.len() >= 2,
        "need at least 2 coins, got {}",
        all_coins.len()
    );
    let gas = all_coins[0];
    let stake_coin = all_coins[1];

    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .call_staking(stake_coin, validator)
        .build();

    // Reference result: the node's own dry-run.
    let dry_run = test_cluster
        .iota_client()
        .read_api()
        .dry_run_transaction_block(tx_data.clone())
        .await
        .expect("node dry-run should succeed");
    let node_summary = SimulationSummary {
        success: matches!(dry_run.effects.status(), IotaExecutionStatus::Success),
        created_count: dry_run.effects.created().len(),
        mutated_count: dry_run.effects.mutated().len(),
        deleted_count: dry_run.effects.deleted().len(),
        events_count: dry_run.events.data.len(),
    };
    assert!(node_summary.success, "node dry-run staking should succeed");

    // Local VM: pre-fetch every referenced object over gRPC, then dev-inspect
    // offline against the same Move engine the node uses.
    let mut store = GrpcStore::connect(&test_cluster.grpc_url()).expect("connect gRPC store");
    let ctx = store
        .fetch_chain_context()
        .await
        .expect("fetch chain context");
    store.prefetch(&tx_data).await.expect("prefetch objects");
    let mut vm = LocalVm::new(ctx, store).expect("build LocalVm");
    let result = vm
        .execute(tx_data.clone(), ExecuteOptions::dev_inspect())
        .expect("local dev-inspect should succeed");
    let local_summary = SimulationSummary {
        success: result.effects.status().is_success(),
        created_count: result.effects.created().len(),
        mutated_count: result.effects.mutated().len(),
        deleted_count: result.effects.deleted().len(),
        events_count: result.events.as_ref().map(|e| e.0.len()).unwrap_or(0),
    };
    assert!(
        local_summary.success,
        "local dev-inspect staking should succeed: {:?}",
        result.effects.status()
    );

    assert_eq!(
        node_summary, local_summary,
        "node dry-run and local VM should agree on the staking effects summary"
    );
}
