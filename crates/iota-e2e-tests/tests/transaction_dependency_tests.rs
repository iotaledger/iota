// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A transaction with a transaction dependency input executes only after the
//! transaction it names succeeded in the current epoch, and never executes
//! when that transaction failed. Every transaction here goes through the
//! fullnode's transaction orchestrator, the path a client's request takes.

use std::time::Duration;

use iota_macros::sim_test;
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, Input, ProgrammableTransaction, TransactionDigest,
    TransactionEffects, TransactionKind,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    effects::TransactionEffectsAPI,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TransactionAPI, TransactionEnvelope},
};
use test_cluster::{TestCluster, TestClusterBuilder, override_pcool_flow};

/// How long a submission may take before the test counts it as never
/// executing. A dependent of an unknown transaction is retried by the drivers
/// until they give up, so it only ends this way or with a transient error.
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(60);

/// The balance of each fresh gas coin, in NANOS.
const GAS_COIN_AMOUNT: u64 = 10_000_000_000;

/// A transfer of `amount` from the gas coin to a fresh address. `u64::MAX`
/// fails at execution for an insufficient balance.
fn transfer(amount: u64) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(Address::random(), Some(amount));
    builder.finish()
}

fn current_epoch(cluster: &TestCluster) -> u64 {
    cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch())
}

/// Signs a transfer of `amount` paid with a fresh gas coin, depending on
/// `dependency` when given.
async fn signed(
    cluster: &TestCluster,
    amount: u64,
    dependency: Option<TransactionDigest>,
) -> TransactionEnvelope {
    let sender = cluster.get_address_0();
    let rgp = cluster.get_reference_gas_price().await;
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(GAS_COIN_AMOUNT), sender)
        .await;
    let mut builder = TestTransactionBuilder::new(sender, gas, rgp).programmable(transfer(amount));
    if let Some(dependency) = dependency {
        builder = builder.with_transaction_dependency(dependency, current_epoch(cluster));
    }
    cluster.sign_transaction(&builder.build())
}

/// What became of a submitted transaction.
#[derive(Debug)]
enum Outcome {
    Executed(TransactionEffects),
    Rejected(String),
    TimedOut,
}

async fn submit(cluster: &TestCluster, transaction: TransactionEnvelope) -> Outcome {
    match tokio::time::timeout(
        SUBMISSION_TIMEOUT,
        cluster.execute_transaction_via_orchestrator(transaction),
    )
    .await
    {
        Ok(Ok((effects, _))) => Outcome::Executed(effects),
        Ok(Err(error)) => Outcome::Rejected(error.to_string()),
        Err(_) => Outcome::TimedOut,
    }
}

/// Whether `error` reports a transaction dependency check: the drivers render
/// the error either with its message or with its variant name.
fn is_dependency_error(error: &str) -> bool {
    error.contains("depends on") || error.contains("TransactionDependency")
}

fn assert_executed_successfully(outcome: &Outcome) -> &TransactionEffects {
    let Outcome::Executed(effects) = outcome else {
        panic!("the transaction did not execute: {outcome:?}");
    };
    assert!(effects.status().is_success(), "{effects:?}");
    effects
}

/// The dependent of a failed transaction is rejected before execution or
/// executed as cancelled, depending on how far it got before the failure was
/// known; it never succeeds.
fn assert_cancelled(outcome: &Outcome, dependency: TransactionDigest) {
    match outcome {
        Outcome::Executed(effects) => {
            let ExecutionStatus::Failure { error, command } = effects.status() else {
                panic!("the dependent executed successfully: {effects:?}");
            };
            assert_eq!(
                *error,
                ExecutionError::TransactionDependencyFailed { dependency }
            );
            assert_eq!(*command, None);
            assert!(effects.dependencies().contains(&dependency));
        }
        Outcome::Rejected(error) => {
            assert!(
                is_dependency_error(error),
                "unexpected rejection of the dependent: {error}"
            );
        }
        Outcome::TimedOut => panic!("the dependent of a failed transaction got no answer"),
    }
}

/// The transaction never executed successfully.
fn assert_not_executed(outcome: &Outcome) {
    if let Outcome::Executed(effects) = outcome {
        assert!(
            !effects.status().is_success(),
            "the transaction executed successfully: {effects:?}"
        );
    }
}

async fn dependent_executes_after_its_dependency_succeeds(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let cluster = TestClusterBuilder::new().build().await;

    let dependency = signed(&cluster, 1_000, None).await;
    let dependency_digest = *dependency.digest();
    assert_executed_successfully(&submit(&cluster, dependency).await);

    let dependent = signed(&cluster, 1_000, Some(dependency_digest)).await;
    let outcome = submit(&cluster, dependent).await;
    let effects = assert_executed_successfully(&outcome);
    assert!(effects.dependencies().contains(&dependency_digest));
}

#[sim_test]
async fn dependent_executes_after_its_dependency_succeeds_certificate_flow() {
    dependent_executes_after_its_dependency_succeeds(false).await;
}

#[sim_test]
async fn dependent_executes_after_its_dependency_succeeds_pcool_flow() {
    dependent_executes_after_its_dependency_succeeds(true).await;
}

async fn dependent_of_a_failed_transaction_is_rejected(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let cluster = TestClusterBuilder::new().build().await;

    let failing = signed(&cluster, u64::MAX, None).await;
    let failing_digest = *failing.digest();
    let Outcome::Executed(effects) = submit(&cluster, failing).await else {
        panic!("the failing transaction was rejected instead of executed");
    };
    assert!(!effects.status().is_success());

    // The failure is known before the dependent is submitted, so it is
    // rejected before execution.
    let second = signed(&cluster, 1_000, Some(failing_digest)).await;
    let second_digest = *second.digest();
    let second_outcome = submit(&cluster, second).await;
    let Outcome::Rejected(error) = &second_outcome else {
        panic!("the dependent of a failed transaction was not rejected: {second_outcome:?}");
    };
    assert!(is_dependency_error(error), "{error}");

    // The rejected transaction never executes, so neither does a transaction
    // that depends on it.
    let third = signed(&cluster, 1_000, Some(second_digest)).await;
    assert_not_executed(&submit(&cluster, third).await);
}

#[sim_test]
async fn dependent_of_a_failed_transaction_is_rejected_certificate_flow() {
    dependent_of_a_failed_transaction_is_rejected(false).await;
}

#[sim_test]
async fn dependent_of_a_failed_transaction_is_rejected_pcool_flow() {
    dependent_of_a_failed_transaction_is_rejected(true).await;
}

async fn pipelined_chain_executes_in_order(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let cluster = TestClusterBuilder::new().build().await;

    let first = signed(&cluster, 1_000, None).await;
    let first_digest = *first.digest();
    let second = signed(&cluster, 1_000, Some(first_digest)).await;
    let second_digest = *second.digest();
    let third = signed(&cluster, 1_000, Some(second_digest)).await;

    let (first_outcome, second_outcome, third_outcome) = futures::join!(
        submit(&cluster, first),
        submit(&cluster, second),
        submit(&cluster, third)
    );
    assert_executed_successfully(&first_outcome);
    assert!(
        assert_executed_successfully(&second_outcome)
            .dependencies()
            .contains(&first_digest)
    );
    assert!(
        assert_executed_successfully(&third_outcome)
            .dependencies()
            .contains(&second_digest)
    );
}

#[sim_test]
async fn pipelined_chain_executes_in_order_certificate_flow() {
    pipelined_chain_executes_in_order(false).await;
}

#[sim_test]
async fn pipelined_chain_executes_in_order_pcool_flow() {
    pipelined_chain_executes_in_order(true).await;
}

async fn pipelined_dependent_of_a_failed_transaction_never_succeeds(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let mut cluster = TestClusterBuilder::new().build().await;

    let failing = signed(&cluster, u64::MAX, None).await;
    let failing_digest = *failing.digest();
    let dependent = signed(&cluster, 1_000, Some(failing_digest)).await;
    let dependent_digest = *dependent.digest();
    let dependent_gas = dependent.data().transaction().gas()[0];

    let (failing_outcome, dependent_outcome) =
        futures::join!(submit(&cluster, failing), submit(&cluster, dependent));
    let Outcome::Executed(effects) = &failing_outcome else {
        panic!("the failing transaction was rejected instead of executed");
    };
    assert!(!effects.status().is_success());
    assert_cancelled(&dependent_outcome, failing_digest);

    let mut executed_digests = vec![failing_digest];
    if let Outcome::Executed(effects) = &dependent_outcome {
        // Cancelled with effects: the gas coin moves on, so the sender can
        // reuse it.
        assert!(effects.gas_object().reference().version > dependent_gas.version);
        executed_digests.push(dependent_digest);
    }

    // A fullnode that only replays checkpoints reaches the same effects.
    let fullnode = cluster.spawn_new_fullnode().await.iota_node;
    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &executed_digests)
        .await;
}

#[sim_test]
async fn pipelined_dependent_of_a_failed_transaction_never_succeeds_certificate_flow() {
    pipelined_dependent_of_a_failed_transaction_never_succeeds(false).await;
}

#[sim_test]
async fn pipelined_dependent_of_a_failed_transaction_never_succeeds_pcool_flow() {
    pipelined_dependent_of_a_failed_transaction_never_succeeds(true).await;
}

async fn dependency_on_an_unknown_transaction_never_executes(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let cluster = TestClusterBuilder::new().build().await;

    let dependent = signed(&cluster, 1_000, Some(TransactionDigest::random())).await;
    let outcome = submit(&cluster, dependent).await;
    assert!(
        !matches!(outcome, Outcome::Executed(_)),
        "a dependent of an unknown transaction executed: {outcome:?}"
    );
}

#[sim_test]
async fn dependency_on_an_unknown_transaction_never_executes_certificate_flow() {
    dependency_on_an_unknown_transaction_never_executes(false).await;
}

#[sim_test]
async fn dependency_on_an_unknown_transaction_never_executes_pcool_flow() {
    dependency_on_an_unknown_transaction_never_executes(true).await;
}

async fn dependency_from_an_earlier_epoch_is_rejected(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let cluster = TestClusterBuilder::new().build().await;

    let dependency = signed(&cluster, 1_000, None).await;
    let dependency_digest = *dependency.digest();
    assert_executed_successfully(&submit(&cluster, dependency).await);

    let epoch = current_epoch(&cluster);
    cluster.force_new_epoch().await;
    cluster.wait_for_epoch(Some(epoch + 1)).await;

    let dependent = signed(&cluster, 1_000, Some(dependency_digest)).await;
    let outcome = submit(&cluster, dependent).await;
    let Outcome::Rejected(error) = &outcome else {
        panic!("a dependent of an earlier-epoch transaction was not rejected: {outcome:?}");
    };
    assert!(
        error.contains("current epoch") || error.contains("NotInCurrentEpoch"),
        "{error}"
    );
}

#[sim_test]
async fn dependency_from_an_earlier_epoch_is_rejected_certificate_flow() {
    dependency_from_an_earlier_epoch_is_rejected(false).await;
}

#[sim_test]
async fn dependency_from_an_earlier_epoch_is_rejected_pcool_flow() {
    dependency_from_an_earlier_epoch_is_rejected(true).await;
}

/// A dependency input without an epoch expiration is invalid in either flow.
#[sim_test]
async fn dependent_without_epoch_expiration_is_invalid() {
    let cluster = TestClusterBuilder::new().build().await;
    let sender = cluster.get_address_0();
    let rgp = cluster.get_reference_gas_price().await;
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(GAS_COIN_AMOUNT), sender)
        .await;

    let mut transaction = TestTransactionBuilder::new(sender, gas, rgp)
        .programmable(transfer(1_000))
        .build();
    let TransactionKind::Programmable(pt) = transaction.kind_mut() else {
        unreachable!()
    };
    pt.inputs
        .push(Input::TransactionDependency(TransactionDigest::random()));

    let outcome = submit(&cluster, cluster.sign_transaction(&transaction)).await;
    let Outcome::Rejected(error) = &outcome else {
        panic!("a dependent without an epoch expiration was not rejected: {outcome:?}");
    };
    assert!(
        error.contains("epoch expiration") || error.contains("RequiresExpiration"),
        "{error}"
    );
}
