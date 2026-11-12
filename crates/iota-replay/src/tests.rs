// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_macros::sim_test;
use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{Address, ExecutionError, ExecutionStatus, TransactionDigest};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    effects::TransactionEffectsAPI,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::TransactionEnvelope,
};
use test_cluster::{TestCluster, TestClusterBuilder};

use crate::{LocalExec, types::ReplayEngineError};

/// The balance of each fresh gas coin, in NANOS.
const GAS_COIN_AMOUNT: u64 = 10_000_000_000;

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
        .transfer_iota(Some(1_000_000_000), Address::ZERO)
        .build();
    let response = test_cluster.sign_and_execute_transaction(&tx_data).await;
    let tx_digest = *response.transaction_digest();

    // Replay with authority certificate execution
    execute_replay(rpc_url, &tx_digest, true)
        .await
        .expect("Replay with authority failed");

    // Replay with execution engine
    execute_replay(rpc_url, &tx_digest, false)
        .await
        .expect("Replay with execution engine failed");
}

/// Turns transaction dependencies on for the nodes of a simulated test
/// cluster and for replay, while the guard is alive.
fn enable_transaction_dependencies() -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_transaction_dependencies_for_testing(true);
        config
    })
}

/// Replays a transaction that depends on a successful transaction. Its
/// effects list the dependency among their dependencies.
#[sim_test]
async fn verify_dependent_replay() {
    let _guard = enable_transaction_dependencies();
    let test_cluster = TestClusterBuilder::new().build().await;
    test_cluster.force_new_epoch().await;

    let dependency = signed_transfer(&test_cluster, 1_000, None).await;
    let dependency_digest = *dependency.digest();
    let (effects, _) = test_cluster
        .execute_transaction_via_orchestrator(dependency)
        .await
        .unwrap();
    assert!(effects.status().is_success(), "{effects:?}");

    let dependent = signed_transfer(&test_cluster, 1_000, Some(dependency_digest)).await;
    let dependent_digest = *dependent.digest();
    let (effects, _) = test_cluster
        .execute_transaction_via_orchestrator(dependent)
        .await
        .unwrap();
    assert!(effects.status().is_success(), "{effects:?}");
    assert!(effects.dependencies().contains(&dependency_digest));

    execute_replay(test_cluster.rpc_url(), &dependent_digest, true)
        .await
        .expect("Replay with authority failed");
    execute_replay(test_cluster.rpc_url(), &dependent_digest, false)
        .await
        .expect("Replay with execution engine failed");
}

/// Replays a transaction cancelled because the transaction it depends on
/// failed.
#[sim_test]
async fn verify_dependent_of_failed_transaction_replay() {
    let _guard = enable_transaction_dependencies();
    let test_cluster = TestClusterBuilder::new().build().await;
    test_cluster.force_new_epoch().await;

    let dependent_digest = execute_dependent_of_failed_transaction(&test_cluster).await;

    execute_replay(test_cluster.rpc_url(), &dependent_digest, true)
        .await
        .expect("Replay with authority failed");
    execute_replay(test_cluster.rpc_url(), &dependent_digest, false)
        .await
        .expect("Replay with execution engine failed");
}

/// Submits a failing transfer together with a transaction that depends on it,
/// until the dependent executes, and returns the dependent's digest. A
/// dependent that reaches a validator after the failure is known is rejected
/// instead, without effects.
async fn execute_dependent_of_failed_transaction(test_cluster: &TestCluster) -> TransactionDigest {
    const ATTEMPTS: usize = 3;
    for _ in 0..ATTEMPTS {
        let failing = signed_transfer(test_cluster, u64::MAX, None).await;
        let failing_digest = *failing.digest();
        let dependent = signed_transfer(test_cluster, 1_000, Some(failing_digest)).await;
        let dependent_digest = *dependent.digest();

        let (failing_result, dependent_result) = futures::join!(
            test_cluster.execute_transaction_via_orchestrator(failing),
            test_cluster.execute_transaction_via_orchestrator(dependent)
        );
        let (effects, _) = failing_result.unwrap();
        assert!(!effects.status().is_success(), "{effects:?}");

        if let Ok((effects, _)) = dependent_result {
            let ExecutionStatus::Failure { error, .. } = effects.status() else {
                panic!("the dependent of a failed transaction succeeded: {effects:?}");
            };
            assert_eq!(
                *error,
                ExecutionError::TransactionDependencyFailed {
                    dependency: failing_digest
                }
            );
            return dependent_digest;
        }
    }
    panic!("the dependent of a failed transaction was rejected in all {ATTEMPTS} attempts");
}

/// Signs a transfer of `amount` from a fresh gas coin to a fresh address,
/// depending on `dependency` when given. `u64::MAX` fails at execution for an
/// insufficient balance.
async fn signed_transfer(
    test_cluster: &TestCluster,
    amount: u64,
    dependency: Option<TransactionDigest>,
) -> TransactionEnvelope {
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(GAS_COIN_AMOUNT), sender)
        .await;
    let mut ptb = ProgrammableTransactionBuilder::new();
    ptb.transfer_iota(Address::random(), Some(amount));
    let mut builder = TestTransactionBuilder::new(sender, gas, rgp).programmable(ptb.finish());
    if let Some(dependency) = dependency {
        let epoch = test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.state().epoch_store_for_testing().epoch());
        builder = builder.with_transaction_dependency(dependency, epoch);
    }
    test_cluster.sign_transaction(&builder.build())
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
