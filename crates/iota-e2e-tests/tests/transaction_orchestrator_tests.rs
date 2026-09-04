// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Skip-effect-certification futures plus the msim scheduler layers push
// rustc's monomorphization query depth past the default 128 in this test
// binary. See the same attribute in `iota-json-rpc/src/lib.rs` for the
// underlying explanation.
#![recursion_limit = "256"]

use std::{sync::Arc, time::Duration};

use iota_core::{
    authority_client::NetworkAuthorityClient, transaction_orchestrator::TransactionOrchestrator,
};
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectReference, TransactionExpiration};
use iota_storage::{
    key_value_store::TransactionKeyValueStore, key_value_store_metrics::KeyValueStoreMetrics,
};
use iota_test_transaction_builder::{
    TestTransactionBuilder, batch_make_transfer_transactions, make_staking_transaction,
    make_transfer_iota_transaction,
};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaError,
    iota_system_state::IotaSystemStateTrait,
    quorum_driver_types::{
        EffectsFinalityInfo, ExecuteTransactionRequestType, ExecuteTransactionRequestV1,
        ExecuteTransactionResponseV1, FinalizedEffects, IsTransactionExecutedLocally,
        QuorumDriverError,
    },
    supported_protocol_versions::SupportedProtocolVersions,
    transaction::{TransactionAPI, TransactionEnvelope},
};
use test_cluster::{TestClusterBuilder, override_pcool_flow};
use tokio::time::timeout;
use tracing::info;

fn make_socket_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::new([127, 0, 0, 1].into(), 0)
}

#[sim_test]
async fn test_blocking_execution() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn_count = 4;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    // Quorum driver does not execute txn locally
    let txn = txns.swap_remove(0);
    let digest = *txn.digest();
    orchestrator
        .quorum_driver()
        .expect("quorum driver exists on a flag-off boot")
        .submit_transaction_no_ticket(
            ExecuteTransactionRequestV1::new(txn),
            Some(make_socket_addr()),
        )
        .await?;

    // Wait for data sync to catch up
    handle
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[digest])
        .await;

    // Transaction Orchestrator proactivcely executes txn locally
    let txn = txns.swap_remove(0);
    let digest = *txn.digest();

    let (_, executed_locally) = execute_with_orchestrator(
        &orchestrator,
        txn,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .unwrap_or_else(|e| panic!("Failed to execute transaction {digest:?}: {e:?}"));

    assert!(executed_locally);

    let metrics = KeyValueStoreMetrics::new_for_tests();
    let kv_store = Arc::new(TransactionKeyValueStore::new(
        "rocksdb",
        metrics,
        handle.state(),
    ));

    assert!(
        handle
            .state()
            .get_executed_transaction_and_effects(digest, kv_store)
            .await
            .is_ok()
    );

    Ok(())
}

#[sim_test]
async fn test_fullnode_wal_log() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    #[cfg(msim)]
    {
        use iota_core::authority::{CheckpointTimeoutConfig, init_checkpoint_timeout_config};
        init_checkpoint_timeout_config(CheckpointTimeoutConfig {
            warning_timeout: Duration::from_secs(2),
            panic_timeout: None,
        });
    }
    telemetry_subscribers::init_for_testing();
    let mut test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600000)
        .build()
        .await;

    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn_count = 2;
    let context = &mut test_cluster.wallet;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );
    // As a comparison, we first verify a tx can go through
    let txn = txns.swap_remove(0);
    let digest = *txn.digest();
    execute_with_orchestrator(
        &orchestrator,
        txn,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .unwrap_or_else(|e| panic!("Failed to execute transaction {digest:?}: {e:?}"));

    let validator_addresses = test_cluster.get_validator_pubkeys();
    assert_eq!(validator_addresses.len(), 4);

    // Stop 2 validators and we lose quorum
    test_cluster.stop_node(&validator_addresses[0]);
    test_cluster.stop_node(&validator_addresses[1]);

    let txn = txns.swap_remove(0);
    // Expect tx to fail
    execute_with_orchestrator(
        &orchestrator,
        txn.clone(),
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .unwrap_err();

    // Because the tx did not go through, we expect to see it in the WAL log
    let pending_txes: Vec<_> = orchestrator
        .load_all_pending_transactions()?
        .into_iter()
        .map(|t| t.into_inner())
        .collect();
    assert_eq!(pending_txes, vec![txn.clone()]);

    // Bring up 1 validator, we obtain quorum again and tx should succeed
    test_cluster.start_node(&validator_addresses[0]).await;
    tokio::task::yield_now().await;
    execute_with_orchestrator(
        &orchestrator,
        txn,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .unwrap();

    // TODO: wal erasing is done in the loop handling effects, so may have some
    // delay. However, once the refactoring is completed the wal removal will be
    // done before response is returned and we will not need the sleep.
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    // The tx should be erased in wal log.
    let pending_txes = orchestrator.load_all_pending_transactions()?;
    assert!(pending_txes.is_empty());

    Ok(())
}

#[sim_test]
async fn test_transaction_orchestrator_reconfig() {
    let _pcool_guard = override_pcool_flow(false);
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().build().await;
    let epoch = test_cluster.fullnode_handle.iota_node.with(|node| {
        node.transaction_orchestrator()
            .unwrap()
            .quorum_driver()
            .expect("quorum driver exists on a flag-off boot")
            .current_epoch()
    });
    assert_eq!(epoch, 0);

    test_cluster.force_new_epoch().await;

    // After epoch change on a fullnode, there could be a delay before the
    // transaction orchestrator updates its committee (happens asynchronously
    // after receiving a reconfig message). Use a timeout to make the test more
    // reliable.
    timeout(Duration::from_secs(5), async {
        loop {
            let epoch = test_cluster.fullnode_handle.iota_node.with(|node| {
                node.transaction_orchestrator()
                    .unwrap()
                    .quorum_driver()
                    .expect("quorum driver exists on a flag-off boot")
                    .current_epoch()
            });
            if epoch == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    assert_eq!(
        test_cluster.fullnode_handle.iota_node.with(|node| node
            .clone_authority_aggregator()
            .unwrap()
            .committee
            .epoch),
        1
    );
}

#[sim_test]
async fn test_tx_across_epoch_boundaries() {
    telemetry_subscribers::init_for_testing();
    // Halting 2 of 4 validators only withholds certification quorum. Under the
    // P-COOL flow a single accepting validator carries the transaction into
    // consensus, so it can be sequenced before the epoch changes;
    // `test_wait_for_local_execution_across_epoch_boundary` covers that flow.
    let _pcool_guard = override_pcool_flow(false);
    let total_tx_cnt = 1;
    let (result_tx, mut result_rx) = tokio::sync::mpsc::channel::<FinalizedEffects>(total_tx_cnt);

    let test_cluster = TestClusterBuilder::new().build().await;
    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    let authorities = test_cluster.swarm.validator_node_handles();

    // We first let 2 validators stop accepting user cert
    // to make sure QD does not get quorum until reconfig
    for handle in authorities.iter().take(2) {
        handle
            .with_async(|node| async { node.close_epoch_for_testing().await.unwrap() })
            .await;
    }

    // Spawn a task that fire the transaction through TransactionOrchestrator
    // across the epoch boundary.
    let to = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.transaction_orchestrator().unwrap());

    let tx_digest = *tx.digest();
    info!(?tx_digest, "Submitting tx");
    tokio::task::spawn(async move {
        match to
            .execute_transaction_block(
                ExecuteTransactionRequestV1::new(tx.clone()),
                ExecuteTransactionRequestType::WaitForEffectsCert,
                None,
            )
            .await
        {
            Ok((response, _)) => {
                info!(?tx_digest, "tx result: ok");
                result_tx.send(response.effects).await.unwrap();
            }
            Err(QuorumDriverError::TimeoutBeforeFinality) => {
                info!(?tx_digest, "tx result: timeout and will retry")
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    });

    info!("Asking remaining validators to change epoch");
    // Ask the remaining 2 validators to close epoch
    for handle in authorities.iter().skip(2) {
        handle
            .with_async(|node| async { node.close_epoch_for_testing().await.unwrap() })
            .await;
    }

    // Wait for the network to reach the next epoch.
    test_cluster.wait_for_epoch(Some(1)).await;

    // The transaction must finalize in epoch 1
    let start = std::time::Instant::now();
    match tokio::time::timeout(tokio::time::Duration::from_secs(15), result_rx.recv()).await {
        Ok(Some(effects_cert)) if effects_cert.epoch() == 1 => (),
        other => panic!("unexpected error: {other:?}"),
    }
    info!("test completed in {:?}", start.elapsed());
}

/// A `WaitForLocalExecution` request in flight at an epoch boundary must
/// resolve shortly after the transaction is checkpointed in the next epoch,
/// not burn the full 30s finality timeout: its checkpoint-inclusion wait
/// registers on the old epoch's store, while the transaction is checkpointed
/// on the next epoch's store (here because submission is rejected until the
/// epoch changes; in the certificate mode also when an executed-but-not-
/// checkpointed transaction is reverted at the boundary and resubmitted).
#[sim_test]
async fn test_wait_for_local_execution_across_epoch_boundary() {
    telemetry_subscribers::init_for_testing();
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (result_tx, mut result_rx) = tokio::sync::mpsc::channel(1);
    let test_cluster = TestClusterBuilder::new().build().await;
    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    let authorities = test_cluster.swarm.validator_node_handles();

    // Stop every validator from accepting user transactions before
    // submitting. Admission is the only way a user transaction enters
    // consensus, so the transaction deterministically cannot be sequenced in
    // epoch 0 — the driver keeps retrying the rejected submissions
    // (`ValidatorHaltedAtEpochEnd` is retriable) until epoch 1 opens. The
    // validators stay up and keep running consensus; the epoch changes once
    // the 2f+1 `EndOfPublish` quorum is collected.
    info!("Asking all validators to change epoch");
    for handle in authorities.iter() {
        handle
            .with_async(|node| async { node.close_epoch_for_testing().await.unwrap() })
            .await;
    }

    let to = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.transaction_orchestrator().unwrap());
    let tx_digest = *tx.digest();
    info!(?tx_digest, "Submitting WaitForLocalExecution tx");
    tokio::task::spawn(async move {
        let result = to
            .execute_transaction_block(
                ExecuteTransactionRequestV1::new(tx),
                ExecuteTransactionRequestType::WaitForLocalExecution,
                None,
            )
            .await;
        result_tx.send(result).await.unwrap();
    });

    // Tripwire: the request's checkpoint-inclusion wait must register on the
    // epoch-0 store for the test to exercise the boundary crossing.
    // Reconfiguration needs several consensus commits plus checkpoint
    // execution, which cannot complete in the spawn gap above; if this ever
    // trips, the test has gone degenerate (passing without covering the
    // boundary) rather than flaky.
    assert_eq!(
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.state().epoch_store_for_testing().epoch()),
        0,
        "reconfiguration outran the submission; the wait no longer starts in epoch 0"
    );

    test_cluster.wait_for_epoch(Some(1)).await;

    // The transaction is checkpointed early in epoch 1 and the request must
    // resolve shortly after — well under the 30s finality timeout it used to
    // burn before returning `TimeoutBeforeFinality`. The window leaves room
    // for the driver's retry backoff, which is capped at 10s.
    let result = match tokio::time::timeout(Duration::from_secs(20), result_rx.recv()).await {
        Ok(Some(result)) => result,
        Ok(None) => panic!("submission task dropped the result channel"),
        Err(_) => panic!("WaitForLocalExecution did not resolve within 20s of the epoch change"),
    };
    let (response, executed_locally) = result
        .unwrap_or_else(|e| panic!("WaitForLocalExecution failed across the boundary: {e:?}"));
    assert!(executed_locally, "tx should be executed locally");
    match response.effects.finality_info {
        EffectsFinalityInfo::Checkpointed(epoch, _seq) => assert_eq!(epoch, 1),
        other => panic!("expected Checkpointed finality, got {other:?}"),
    }
}

/// The driver must follow `enable_pcool_flow` across the upgrade that flips
/// it, without a fullnode restart: boot at v31 (flag off, QuorumDriver),
/// upgrade to v32 (flag on), and the same orchestrator instance must serve
/// both sides, post-upgrade via the TransactionDriver.
///
/// No `override_pcool_flow`: the env override would pin the flag for every
/// version.
#[sim_test]
async fn test_orchestrator_follows_pcool_flag_across_protocol_upgrade() {
    telemetry_subscribers::init_for_testing();
    const START: u64 = 31;
    const FINISH: u64 = 32;

    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(START.into())
        .with_supported_protocol_versions(SupportedProtocolVersions::new_for_testing(START, FINISH))
        .with_epoch_duration_ms(20_000)
        .build()
        .await;

    let orchestrator = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.transaction_orchestrator().unwrap());
    let pcool_enabled = || {
        test_cluster.fullnode_handle.iota_node.with(|node| {
            node.state()
                .epoch_store_for_testing()
                .protocol_config()
                .enable_pcool_flow()
        })
    };

    // Boot state: flag off, quorum driver built and recovered.
    assert!(!pcool_enabled());
    assert!(orchestrator.quorum_driver().is_some());

    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    let (response, _) = execute_with_orchestrator(
        &orchestrator,
        tx,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .expect("pre-upgrade submission must succeed on the certificate flow");
    assert!(matches!(
        response.effects.finality_info,
        EffectsFinalityInfo::Certified(_)
    ));

    // All validators support FINISH, so the upgrade lands at the first epoch
    // boundary.
    let system_state = test_cluster.wait_for_protocol_version(FINISH.into()).await;
    let flip_epoch = system_state.epoch();
    test_cluster.wait_for_epoch_all_nodes(flip_epoch).await;
    assert!(pcool_enabled());

    // Same orchestrator, no restart: the request must use the
    // TransactionDriver, since validators now reject the certificate flow.
    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    let (response, executed_locally) = execute_with_orchestrator(
        &orchestrator,
        tx,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .expect("post-upgrade submission must succeed without a fullnode restart");
    assert!(executed_locally);
    match response.effects.finality_info {
        EffectsFinalityInfo::Checkpointed(epoch, _) => assert!(epoch >= flip_epoch),
        EffectsFinalityInfo::QuorumExecuted(epoch) => assert!(epoch >= flip_epoch),
        other => panic!("expected TransactionDriver finality, got {other:?}"),
    }

    // The TransactionDriver must keep tracking reconfiguration.
    test_cluster.wait_for_epoch(Some(flip_epoch + 1)).await;
    test_cluster.wait_for_epoch_all_nodes(flip_epoch + 1).await;
    let tx = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
    execute_with_orchestrator(
        &orchestrator,
        tx,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await
    .expect("submission must succeed one epoch after the flip");
}

async fn execute_with_orchestrator(
    orchestrator: &TransactionOrchestrator<NetworkAuthorityClient>,
    tx: TransactionEnvelope,
    request_type: ExecuteTransactionRequestType,
) -> Result<(ExecuteTransactionResponseV1, IsTransactionExecutedLocally), QuorumDriverError> {
    orchestrator
        .execute_transaction_block(ExecuteTransactionRequestV1::new(tx), request_type, None)
        .await
}

/// A resubmission of an already-executed transaction must be answered from
/// the local cache (finality `QuorumExecuted`) instead of being driven
/// through the validators again — on every entry point.
#[sim_test]
async fn test_cached_response_for_executed_transaction() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");
    let digest = *txn.digest();

    let (first, _) = execute_with_orchestrator(
        &orchestrator,
        txn.clone(),
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await?;
    assert!(
        matches!(
            first.effects.finality_info,
            EffectsFinalityInfo::Certified(_)
        ),
        "first execution should be driven to a certificate, got {:?}",
        first.effects.finality_info
    );

    // Make sure the effects have landed in the local cache before
    // resubmitting.
    handle
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[digest])
        .await;

    let (second, executed_locally) = execute_with_orchestrator(
        &orchestrator,
        txn.clone(),
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await?;
    assert!(executed_locally);
    assert!(
        matches!(
            second.effects.finality_info,
            EffectsFinalityInfo::QuorumExecuted(_)
        ),
        "resubmission should be answered from the local cache, got {:?}",
        second.effects.finality_info
    );
    assert_eq!(
        first.effects.effects.digest(),
        second.effects.effects.digest()
    );

    let (third, _) = execute_with_orchestrator(
        &orchestrator,
        txn.clone(),
        ExecuteTransactionRequestType::WaitForEffectsCert,
    )
    .await?;
    assert!(
        matches!(
            third.effects.finality_info,
            EffectsFinalityInfo::QuorumExecuted(_)
        ),
        "resubmission without local-execution wait should also be answered \
         from the local cache, got {:?}",
        third.effects.finality_info
    );

    let response = orchestrator
        .execute_transaction_v1(ExecuteTransactionRequestV1::new(txn), false, None)
        .await?;
    assert!(
        matches!(
            response.effects.finality_info,
            EffectsFinalityInfo::QuorumExecuted(_)
        ),
        "v1 resubmission should be answered from the local cache, got {:?}",
        response.effects.finality_info
    );

    Ok(())
}

#[sim_test]
async fn execute_transaction_v1() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn_count = 1;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    // Quorum driver does not execute txn locally
    let txn = txns.swap_remove(0);

    let request = ExecuteTransactionRequestV1 {
        transaction: txn,
        include_events: true,
        include_input_objects: true,
        include_output_objects: true,
        include_auxiliary_data: false,
    };
    let response = orchestrator
        .execute_transaction_v1(request, false, None)
        .await?;
    let fx = &response.effects.effects;

    let mut expected_input_objects = fx
        .modified_at_versions()
        .into_iter()
        .map(|modified| (modified.object_id, modified.version))
        .collect::<Vec<_>>();
    expected_input_objects.sort_by_key(|&(id, _version)| id);
    let mut expected_output_objects = fx
        .all_changed_objects()
        .into_iter()
        .map(|(owned_object_ref, _)| owned_object_ref.reference)
        .collect::<Vec<_>>();
    expected_output_objects.sort_by_key(|&object_ref| object_ref.object_id);

    let mut actual_input_objects_received = response
        .input_objects
        .unwrap()
        .iter()
        .map(|object| (object.id(), object.version()))
        .collect::<Vec<_>>();
    actual_input_objects_received.sort_by_key(|&(id, _version)| id);
    assert_eq!(expected_input_objects, actual_input_objects_received);

    let mut actual_output_objects_received = response
        .output_objects
        .unwrap()
        .iter()
        .map(|object| ObjectReference::new(object.id(), object.version(), object.digest()))
        .collect::<Vec<_>>();
    actual_output_objects_received.sort_by_key(|&object_ref| object_ref.object_id);
    assert_eq!(expected_output_objects, actual_output_objects_received);

    Ok(())
}

/// With the P-COOL flow enabled, `WaitForLocalExecution` takes the
/// skip-effect-certification path inside the orchestrator. The single-
/// validator response tagged `UncertifiedSingleValidator` must be upgraded
/// to `Checkpointed(epoch, seq)` by the local-cache reconciliation before
/// being returned to the caller — otherwise the safety guard at the end of
/// `execute_transaction_block` would reject the response as
/// `QuorumDriverInternal`.
#[sim_test]
async fn test_skip_effect_cert_reconciles_to_checkpointed() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");
    let digest = *txn.digest();

    let (response, executed_locally) = orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1 {
                transaction: txn,
                include_events: true,
                include_input_objects: true,
                include_output_objects: true,
                include_auxiliary_data: false,
            },
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        )
        .await
        .unwrap_or_else(|e| panic!("skip-cert execution failed for {digest:?}: {e:?}"));

    assert!(executed_locally, "tx should be executed locally");

    // The strong signal that reconcile ran: the TD skip-cert path never
    // produces `Certified` (no 2f+1 broadcast happened) and never produces
    // `QuorumExecuted` (that's the pre-reconcile TD output). Only the
    // reconcile step upgrades to `Checkpointed(epoch, seq)`. If the safety
    // guard had fired instead, `execute_transaction_block` would have
    // returned a `QuorumDriverInternal` error.
    match response.effects.finality_info {
        EffectsFinalityInfo::Checkpointed(_epoch, seq) => {
            assert!(seq > 0, "checkpoint sequence should be populated");
        }
        other => panic!(
            "skip-cert reconciliation should upgrade finality to Checkpointed, got {other:?}"
        ),
    }
    // Request flags were set — the reconcile path must populate the object
    // fields rather than dropping them. (Events are skipped: a transfer
    // tx does not emit any; the negative-case is covered by
    // `test_skip_effect_cert_respects_request_flags`.)
    assert!(response.input_objects.is_some());
    assert!(response.output_objects.is_some());

    Ok(())
}

/// Under the P-COOL flow, a resubmission of an already-executed transaction
/// must be answered from the local cache (finality `QuorumExecuted`) before
/// reaching the skip-effect-certification path, and must not be routed
/// through the cache-rebuild reconciliation (which would tag it
/// `Checkpointed`).
#[sim_test]
async fn test_cached_response_for_executed_transaction_under_pcool() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");

    // The first execution reconciles from the local cache after checkpoint
    // inclusion, so the effects are guaranteed to be cached afterwards.
    let (first, _) = execute_with_orchestrator(
        &orchestrator,
        txn.clone(),
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await?;
    assert!(
        matches!(
            first.effects.finality_info,
            EffectsFinalityInfo::Checkpointed(_, _)
        ),
        "first skip-cert execution should reconcile to Checkpointed, got {:?}",
        first.effects.finality_info
    );

    let (second, executed_locally) = execute_with_orchestrator(
        &orchestrator,
        txn,
        ExecuteTransactionRequestType::WaitForLocalExecution,
    )
    .await?;
    assert!(executed_locally);
    assert!(
        matches!(
            second.effects.finality_info,
            EffectsFinalityInfo::QuorumExecuted(_)
        ),
        "resubmission should be answered from the local cache, got {:?}",
        second.effects.finality_info
    );
    assert_eq!(
        first.effects.effects.digest(),
        second.effects.effects.digest()
    );

    Ok(())
}

/// With the P-COOL flow enabled, a caller that did *not* ask for events
/// or input/output objects must not receive them.
#[sim_test]
async fn test_skip_effect_cert_respects_request_flags() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");

    let (response, _) = orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1 {
                transaction: txn,
                include_events: false,
                include_input_objects: false,
                include_output_objects: false,
                include_auxiliary_data: false,
            },
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        )
        .await?;

    assert!(
        matches!(
            response.effects.finality_info,
            EffectsFinalityInfo::Checkpointed(_, _)
        ),
        "skip-cert response should always be Checkpointed, got {:?}",
        response.effects.finality_info
    );
    assert!(
        response.events.is_none(),
        "events must not leak when include_events=false"
    );
    assert!(
        response.input_objects.is_none(),
        "input_objects must not leak when include_input_objects=false"
    );
    assert!(
        response.output_objects.is_none(),
        "output_objects must not leak when include_output_objects=false"
    );

    Ok(())
}

/// With the P-COOL flow enabled, two concurrent submissions of the same
/// transaction digest must not each drive an independent committee-wide
/// submission: the second observes the first in flight and waits for its
/// effects instead. Both callers must return the same finalized effects, and
/// the pending-transaction log must stay empty: the driver path tracks
/// in-flight submissions in memory only.
#[sim_test]
async fn test_pcool_deduplicates_concurrent_submissions() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");
    let digest = *txn.digest();

    let request = |tx: TransactionEnvelope| ExecuteTransactionRequestV1 {
        transaction: tx,
        include_events: false,
        include_input_objects: false,
        include_output_objects: false,
        include_auxiliary_data: false,
    };

    let (first, second) = tokio::join!(
        orchestrator.execute_transaction_block(
            request(txn.clone()),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        ),
        orchestrator.execute_transaction_block(
            request(txn.clone()),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        ),
    );

    let (first_response, _) =
        first.unwrap_or_else(|e| panic!("first submission failed for {digest:?}: {e:?}"));
    let (second_response, _) =
        second.unwrap_or_else(|e| panic!("second submission failed for {digest:?}: {e:?}"));

    for response in [&first_response, &second_response] {
        assert!(
            matches!(
                response.effects.finality_info,
                EffectsFinalityInfo::Checkpointed(_, _)
            ),
            "concurrent submission should resolve to Checkpointed, got {:?}",
            response.effects.finality_info
        );
    }
    assert_eq!(
        first_response.effects.effects.transaction_digest(),
        second_response.effects.effects.transaction_digest(),
        "both concurrent submissions must report the same finalized effects"
    );

    let pending = orchestrator.load_all_pending_transactions()?;
    assert!(
        pending.is_empty(),
        "driver path must not write to the pending transaction log, found {pending:?}"
    );

    Ok(())
}

/// A duplicate submission must inherit the outcome of the in-flight
/// submission it waited on. With a transaction validators deterministically
/// reject (its gas object version was already consumed), the duplicate must
/// fail with the same error as the driving submission instead of waiting for
/// a checkpoint inclusion that can never happen and timing out.
#[sim_test]
async fn test_pcool_duplicate_submission_inherits_failure() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;
    let context = &test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    // Consume a gas object, then build a second transaction spending the
    // same (now stale) gas object version: validators reject it as invalid,
    // deterministically failing the driving submission.
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let spend = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .transfer_iota(Some(1), sender)
            .build(),
    );
    orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1::new(spend),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        )
        .await
        .expect("spending the gas object must succeed");

    let stale = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .transfer_iota(Some(2), sender)
            .build(),
    );

    let (first, second) = tokio::join!(
        orchestrator.execute_transaction_block(
            ExecuteTransactionRequestV1::new(stale.clone()),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        ),
        orchestrator.execute_transaction_block(
            ExecuteTransactionRequestV1::new(stale.clone()),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        ),
    );

    let first_err = first.expect_err("transaction spending a stale gas object must fail");
    let second_err = second.expect_err("transaction spending a stale gas object must fail");
    assert!(
        matches!(first_err, QuorumDriverError::RejectedByValidators(_)),
        "expected the submission to be rejected by validators, got {first_err:?}"
    );
    assert_eq!(
        first_err, second_err,
        "the duplicate submission must inherit the in-flight submission's error"
    );

    Ok(())
}

/// A duplicate that requires certified effects (v1 without checkpoint
/// waiting) joining an in-flight skip-cert submission must not inherit the
/// uncertified single-validator effects: it certifies the effects itself and
/// returns certified finality.
#[sim_test]
async fn test_pcool_duplicate_requiring_certification_returns_certified_effects()
-> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");

    let request = |tx: TransactionEnvelope| ExecuteTransactionRequestV1 {
        transaction: tx,
        include_events: false,
        include_input_objects: false,
        include_output_objects: false,
        include_auxiliary_data: false,
    };

    // `WaitForLocalExecution` drives a skip-cert submission; the head start
    // lets the v1 call below join it as a duplicate instead of driving its
    // own submission.
    let (driving, duplicate) = tokio::join!(
        orchestrator.execute_transaction_block(
            request(txn.clone()),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        ),
        async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            orchestrator
                .execute_transaction_v1(request(txn.clone()), false, Some(make_socket_addr()))
                .await
        },
    );

    let (driving_response, _) = driving?;
    let duplicate_response = duplicate?;

    assert!(
        !matches!(
            duplicate_response.effects.finality_info,
            EffectsFinalityInfo::UncertifiedSingleValidator(_)
        ),
        "a certification-requiring duplicate must never see uncertified effects, got {:?}",
        duplicate_response.effects.finality_info
    );
    assert_eq!(
        driving_response.effects.effects.transaction_digest(),
        duplicate_response.effects.effects.transaction_digest(),
        "the duplicate must resolve to the same finalized effects"
    );

    Ok(())
}

/// Without consensus quorum, the skip-cert path can never observe checkpoint
/// inclusion. The orchestrator must surface this as `TimeoutBeforeFinality`
/// (a retriable transient), not `QuorumDriverInternal` — the latter would
/// page on-call for a routine availability dip.
#[sim_test]
async fn test_skip_effect_cert_timeout_without_quorum() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    // Sanity-check the happy path first so the failure below is attributable
    // to the deliberate quorum loss, not a misconfigured cluster.
    let mut txns = batch_make_transfer_transactions(context, 2).await;
    let healthy_txn = txns.swap_remove(0);
    orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1 {
                transaction: healthy_txn,
                include_events: false,
                include_input_objects: false,
                include_output_objects: false,
                include_auxiliary_data: false,
            },
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        )
        .await
        .expect("baseline skip-cert tx should succeed before quorum loss");

    // Drop two validators (of four) so consensus cannot form. Checkpoint
    // inclusion will never happen for any new tx submitted after this point.
    let validator_addresses = test_cluster.get_validator_pubkeys();
    assert_eq!(validator_addresses.len(), 4);
    test_cluster.stop_node(&validator_addresses[0]);
    test_cluster.stop_node(&validator_addresses[1]);

    let stuck_txn = txns.swap_remove(0);
    let result = orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1 {
                transaction: stuck_txn,
                include_events: false,
                include_input_objects: false,
                include_output_objects: false,
                include_auxiliary_data: false,
            },
            ExecuteTransactionRequestType::WaitForLocalExecution,
            Some(make_socket_addr()),
        )
        .await;

    match result {
        Err(QuorumDriverError::TimeoutBeforeFinality)
        | Err(QuorumDriverError::FailedWithTransientErrorAfterMaximumAttempts { .. }) => {}
        Err(QuorumDriverError::QuorumDriverInternal(e)) => panic!(
            "skip-cert quorum loss should map to TimeoutBeforeFinality, got \
             QuorumDriverInternal: {e:?}"
        ),
        Err(other) => {
            panic!("unexpected error variant from skip-cert under quorum loss: {other:?}")
        }
        Ok((response, _)) => panic!(
            "skip-cert should not succeed without consensus quorum; got {:?}",
            response.effects.finality_info
        ),
    }

    Ok(())
}

/// Under P-COOL the orchestrator has no quorum driver, so the authority
/// aggregator must come from the transaction driver — and it must track
/// reconfiguration, since test infra polls its committee epoch.
#[sim_test]
async fn test_authority_aggregator_accessor_under_pcool() {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;

    let epoch = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.clone_authority_aggregator().unwrap().committee.epoch);
    assert_eq!(epoch, 0);

    test_cluster.force_new_epoch().await;

    // The aggregator is swapped asynchronously after the reconfig message,
    // so poll with a timeout.
    timeout(Duration::from_secs(5), async {
        loop {
            let epoch = test_cluster
                .fullnode_handle
                .iota_node
                .with(|node| node.clone_authority_aggregator().unwrap().committee.epoch);
            if epoch == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("transaction driver's authority aggregator should reconfigure to epoch 1");
}

#[sim_test]
async fn execute_transaction_v1_staking_transaction() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    // Here we test the staking transaction to a committee member.
    let committee_member_address = context
        .get_client()
        .await?
        .governance_api()
        .get_latest_iota_system_state()
        .await?
        .iter_committee_members()
        .next()
        .unwrap()
        .iota_address;

    let transaction = make_staking_transaction(context, committee_member_address).await;

    let request = ExecuteTransactionRequestV1 {
        transaction,
        include_events: true,
        include_input_objects: true,
        include_output_objects: true,
        include_auxiliary_data: false,
    };
    let response = orchestrator
        .execute_transaction_v1(request, false, None)
        .await?;
    let fx = &response.effects.effects;

    let mut expected_input_objects = fx
        .modified_at_versions()
        .into_iter()
        .map(|modified| (modified.object_id, modified.version))
        .collect::<Vec<_>>();
    expected_input_objects.sort_by_key(|&(id, _version)| id);
    let mut expected_output_objects = fx
        .all_changed_objects()
        .into_iter()
        .map(|(owned_object_ref, _)| owned_object_ref.reference)
        .collect::<Vec<_>>();
    expected_output_objects.sort_by_key(|&object_ref| object_ref.object_id);

    let mut actual_input_objects_received = response
        .input_objects
        .unwrap()
        .iter()
        .map(|object| (object.id(), object.version()))
        .collect::<Vec<_>>();
    actual_input_objects_received.sort_by_key(|&(id, _version)| id);
    assert_eq!(expected_input_objects, actual_input_objects_received);

    let mut actual_output_objects_received = response
        .output_objects
        .unwrap()
        .iter()
        .map(|object| ObjectReference::new(object.id(), object.version(), object.digest()))
        .collect::<Vec<_>>();
    actual_output_objects_received.sort_by_key(|&object_ref| object_ref.object_id);
    assert_eq!(expected_output_objects, actual_output_objects_received);

    Ok(())
}

// Submitting a transaction whose expiration epoch lies in the past must be
// rejected by the orchestrator's `validity_check` before it ever reaches the
// quorum driver. The expected surface error is `InvalidTransaction`, carrying
// the inner `IotaError::TransactionExpired`.
#[sim_test]
async fn test_orchestrator_rejects_expired_transaction() {
    let test_cluster = TestClusterBuilder::new().build().await;

    // Advance to epoch >= 1 so a transaction marked as expiring at epoch 0
    // is past its expiration window.
    test_cluster.force_new_epoch().await;

    let context = &test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let mut data = TestTransactionBuilder::new(sender, gas_object, gas_price)
        .transfer_iota(Some(1), sender)
        .build();
    *data.expiration_mut_for_testing() = TransactionExpiration::Epoch(0);
    let txn = context.sign_transaction(&data);

    let err = orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1::new(txn),
            ExecuteTransactionRequestType::WaitForEffectsCert,
            None,
        )
        .await
        .expect_err("expired transaction must be rejected by the orchestrator");

    assert!(
        matches!(
            err,
            QuorumDriverError::InvalidTransaction(IotaError::TransactionExpired)
        ),
        "expected InvalidTransaction(TransactionExpired), got {err:?}"
    );
}

/// Under the P-COOL flow, `WaitForLocalExecution` races the
/// TransactionDriver submission against local checkpoint inclusion inside
/// `submit_with_checkpoint_race`. That race — including the
/// `drive_transaction` call it wraps — runs in a task detached from the
/// caller, so a client that disconnects mid-call must not stop the
/// transaction from being driven to finality.
///
/// Quorum is broken before submission so the caller can be aborted while the
/// transaction is provably still stuck (no quorum can possibly have been
/// reached yet); quorum is then restored and finality is confirmed
/// independently of the aborted caller.
///
/// Checkpoint inclusion alone cannot isolate the detached task: the
/// submission typically reaches a live validator's consensus adapter before
/// the abort, and the validator carries it to finality once quorum is
/// restored even if the fullnode-side task died. The in-flight map is the
/// fullnode-side signal — a cancelled submission drops its guard and removes
/// the entry — so the test asserts the digest stays in flight across the
/// abort and that a duplicate submitted while quorum is still broken joins
/// the surviving submission instead of driving its own.
#[sim_test]
async fn test_submission_survives_caller_abort() -> Result<(), anyhow::Error> {
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());

    let txn = batch_make_transfer_transactions(context, 1)
        .await
        .pop()
        .expect("gas objects should produce at least one tx");
    let digest = *txn.digest();

    // Break quorum so the submission cannot possibly finish yet, then submit
    // and abort the caller while it is provably still stuck.
    let validator_addresses = test_cluster.get_validator_pubkeys();
    assert_eq!(validator_addresses.len(), 4);
    test_cluster.stop_node(&validator_addresses[0]);
    test_cluster.stop_node(&validator_addresses[1]);

    let caller_task = tokio::spawn({
        let orchestrator = orchestrator.clone();
        let txn = txn.clone();
        async move {
            orchestrator
                .execute_transaction_block(
                    ExecuteTransactionRequestV1::new(txn),
                    ExecuteTransactionRequestType::WaitForLocalExecution,
                    Some(make_socket_addr()),
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_secs(1)).await;
    caller_task.abort();
    assert!(
        caller_task
            .await
            .expect_err("caller task should not have finished before quorum was restored")
            .is_cancelled(),
        "caller task should have been aborted, not have panicked"
    );
    assert_eq!(
        orchestrator.in_flight_duplicates_for_testing(&digest),
        Some(0),
        "the submission must still be in flight after the caller abort"
    );

    // A duplicate submitted while quorum is still broken must join the
    // surviving submission instead of driving a second committee-wide one.
    let duplicate_task = tokio::spawn({
        let orchestrator = orchestrator.clone();
        let txn = txn.clone();
        async move {
            orchestrator
                .execute_transaction_block(
                    ExecuteTransactionRequestV1::new(txn),
                    ExecuteTransactionRequestType::WaitForLocalExecution,
                    Some(make_socket_addr()),
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(
        orchestrator.in_flight_duplicates_for_testing(&digest),
        Some(1),
        "the duplicate must await the in-flight submission's outcome"
    );

    // Restore quorum. The detached task inside the orchestrator — never
    // aborted — should still be retrying submission on its own and drive the
    // transaction to finality.
    tokio::join!(
        test_cluster.start_node(&validator_addresses[0]),
        test_cluster.start_node(&validator_addresses[1]),
    );

    let (duplicate_response, _) = duplicate_task
        .await
        .expect("duplicate task should not panic")?;
    assert!(
        matches!(
            duplicate_response.effects.finality_info,
            EffectsFinalityInfo::Checkpointed(_, _)
        ),
        "the duplicate should resolve to Checkpointed via the surviving submission, got {:?}",
        duplicate_response.effects.finality_info
    );
    assert_eq!(
        duplicate_response.effects.effects.transaction_digest(),
        &digest,
        "the duplicate must return the aborted caller's transaction"
    );

    let inclusion = handle
        .state()
        .wait_for_checkpoint_inclusion(&[digest], Duration::from_secs(30))
        .await
        .expect("wait_for_checkpoint_inclusion should not error");
    assert!(
        inclusion.contains_key(&digest),
        "transaction should reach finality via the detached task even though \
         the caller was aborted"
    );

    Ok(())
}

/// Extracts the suggested gas price from execution-worker congestion
/// cancellation effects, if that is what `effects` carry.
fn cancelled_congestion_suggested_gas_price(
    effects: &iota_sdk_types::TransactionEffects,
) -> Option<u64> {
    match effects.status() {
        iota_sdk_types::ExecutionStatus::Failure {
            error:
                iota_sdk_types::ExecutionError::ExecutionCanceledDueToExecutionWorkerCongestion {
                    suggested_gas_price,
                },
            ..
        } => Some(*suggested_gas_price),
        _ => None,
    }
}

/// End-to-end execution-worker congestion: with a single execution worker and
/// a per-commit limit of one transaction, a burst of owned-object-only
/// transfers overloads the sequencer on every validator. Transactions past
/// the deferral limit are cancelled: they still execute (charging gas) and
/// the client receives certified failure effects carrying a suggested gas
/// price. Resubmitting with the charged gas object at that price must
/// succeed. The four validators independently agreeing on the cancelled set
/// is implicitly verified: the cluster would stall or fork otherwise.
#[sim_test]
async fn test_execution_worker_congestion_end_to_end() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_per_object_congestion_control_mode_for_testing(
            iota_protocol_config::PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_concurrent_execution_workers_for_testing(1);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;
    let context = &test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());
    let rgp = context.get_reference_gas_price().await?;
    let recipient = iota_types::crypto::get_key_pair::<iota_types::crypto::AccountPrivateKey>().0;

    // Submit bursts of concurrent transfers (each using a distinct gas object)
    // until one is cancelled: only one transaction fits per commit, so any
    // commit carrying two or more cancels the rest. Retry with fresh object
    // references in the unlikely case a burst spreads across
    // single-transaction commits.
    let mut congested: Option<(iota_sdk_types::Address, ObjectReference, u64)> = None;
    'bursts: for _ in 0..5 {
        let accounts_and_objs = context.get_all_accounts_and_gas_objects().await?;
        let batch: Vec<_> = accounts_and_objs
            .iter()
            .flat_map(|(address, objs)| objs.iter().map(|obj| (*address, *obj)))
            .take(10)
            .collect();
        let submissions = batch.iter().map(|(address, obj)| {
            let data = iota_sdk_types::Transaction::new_transfer_iota(
                recipient,
                *address,
                Some(2),
                *obj,
                rgp * iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                rgp,
            );
            let txn = context.sign_transaction(&data);
            orchestrator.execute_transaction_block(
                ExecuteTransactionRequestV1 {
                    transaction: txn,
                    include_events: false,
                    include_input_objects: false,
                    include_output_objects: false,
                    include_auxiliary_data: false,
                },
                ExecuteTransactionRequestType::WaitForEffectsCert,
                Some(make_socket_addr()),
            )
        });
        let results = futures::future::join_all(submissions).await;

        for ((address, obj), result) in batch.iter().zip(&results) {
            // Cancelled transactions still finalize with certified effects,
            // so every submission must succeed at the transport level.
            let (response, _) = result
                .as_ref()
                .expect("cancelled transactions still return certified effects");
            let effects = &response.effects.effects;
            if effects.status().is_success() {
                continue;
            }
            let suggested_gas_price = cancelled_congestion_suggested_gas_price(effects)
                .unwrap_or_else(|| {
                    panic!(
                        "expected congestion cancellation, got {:?}",
                        effects.status()
                    )
                });
            // The cancelled transaction lost the single worker to a
            // competitor paying the reference gas price, so the worker
            // clearing price is exactly that, and the suggestion is one
            // above it — always strictly above what the cancelled
            // transaction paid.
            assert_eq!(suggested_gas_price, rgp + 1);
            // The cancelled execution charged gas, so the gas object's
            // version moved: take the current reference from the certified
            // effects (the fullnode's own object view may lag behind them).
            let gas_object = effects
                .mutated()
                .iter()
                .find(|mutated| mutated.reference.object_id == obj.object_id)
                .expect("the cancelled execution charges the gas object")
                .reference;
            congested = Some((*address, gas_object, suggested_gas_price));
            break 'bursts;
        }
    }
    let (address, gas_object, suggested_gas_price) =
        congested.expect("bursts of 10 concurrent transactions should overload a 1-tx commit");
    info!(
        ?gas_object,
        suggested_gas_price, "transaction was cancelled"
    );

    // Resubmit with the charged gas object at the suggested gas price.
    let gas_price = suggested_gas_price;
    let data = iota_sdk_types::Transaction::new_transfer_iota(
        recipient,
        address,
        Some(2),
        gas_object,
        gas_price * iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        gas_price,
    );
    let txn = context.sign_transaction(&data);
    let (response, _) = orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1 {
                transaction: txn,
                include_events: false,
                include_input_objects: false,
                include_output_objects: false,
                include_auxiliary_data: false,
            },
            ExecuteTransactionRequestType::WaitForEffectsCert,
            Some(make_socket_addr()),
        )
        .await
        .expect("resubmission at the suggested gas price should be accepted");
    assert!(response.effects.effects.status().is_success());

    Ok(())
}

/// Restart a validator right after a commit that cancelled a transaction for
/// execution-worker congestion. On recovery the validator must re-derive the
/// cancellation deterministically (replaying consensus, or executing the
/// synced checkpoint against its expected effects digest) — a divergence
/// would fork and panic the node, failing the test. Afterwards the cluster,
/// including the restarted validator, must still finalize a resubmission and
/// keep cancelling under congestion.
#[sim_test]
async fn test_execution_worker_congestion_cancellation_validator_restart()
-> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _env_guard = override_pcool_flow(true);
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_per_object_congestion_control_mode_for_testing(
            iota_protocol_config::PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_concurrent_execution_workers_for_testing(1);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        config
    });

    let test_cluster = TestClusterBuilder::new().build().await;
    let context = &test_cluster.wallet;
    let handle = &test_cluster.fullnode_handle.iota_node;
    let orchestrator = handle.with(|n| n.transaction_orchestrator().as_ref().unwrap().clone());
    let rgp = context.get_reference_gas_price().await?;
    let recipient = iota_types::crypto::get_key_pair::<iota_types::crypto::AccountPrivateKey>().0;

    let submit_transfer = |address, gas_object: ObjectReference, gas_price: u64| {
        let data = iota_sdk_types::Transaction::new_transfer_iota(
            recipient,
            address,
            Some(2),
            gas_object,
            gas_price * iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            gas_price,
        );
        let txn = context.sign_transaction(&data);
        orchestrator.execute_transaction_block(
            ExecuteTransactionRequestV1 {
                transaction: txn,
                include_events: false,
                include_input_objects: false,
                include_output_objects: false,
                include_auxiliary_data: false,
            },
            ExecuteTransactionRequestType::WaitForEffectsCert,
            Some(make_socket_addr()),
        )
    };

    // Burst transfers until one is cancelled (as in
    // `test_execution_worker_congestion_end_to_end`).
    let mut congested: Option<(iota_sdk_types::Address, ObjectReference, u64)> = None;
    'bursts: for _ in 0..5 {
        let accounts_and_objs = context.get_all_accounts_and_gas_objects().await?;
        let batch: Vec<_> = accounts_and_objs
            .iter()
            .flat_map(|(address, objs)| objs.iter().map(|obj| (*address, *obj)))
            .take(10)
            .collect();
        let submissions = batch
            .iter()
            .map(|(address, obj)| submit_transfer(*address, *obj, rgp));
        let results = futures::future::join_all(submissions).await;
        for ((address, obj), result) in batch.iter().zip(&results) {
            let (response, _) = result
                .as_ref()
                .expect("cancelled transactions still return certified effects");
            let effects = &response.effects.effects;
            let Some(suggested_gas_price) = cancelled_congestion_suggested_gas_price(effects)
            else {
                continue;
            };
            let gas_object = effects
                .mutated()
                .iter()
                .find(|mutated| mutated.reference.object_id == obj.object_id)
                .expect("the cancelled execution charges the gas object")
                .reference;
            congested = Some((*address, gas_object, suggested_gas_price));
            break 'bursts;
        }
    }
    let (address, gas_object, suggested_gas_price) =
        congested.expect("bursts of 10 concurrent transactions should overload a 1-tx commit");

    // Restart a validator: its recovery replays the commit that cancelled the
    // transaction (or executes the synced checkpoint), and must reproduce the
    // cancellation exactly. The sleep between stop and start lets the stopped
    // node release its database before the restart reopens it.
    let validator = test_cluster.get_validator_pubkeys()[0];
    test_cluster.stop_node(&validator);
    tokio::time::sleep(Duration::from_secs(1)).await;
    test_cluster.start_node(&validator).await;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // The resubmission at the suggested gas price must finalize.
    let (response, _) = submit_transfer(address, gas_object, suggested_gas_price)
        .await
        .expect("resubmission at the suggested gas price should be accepted");
    assert!(response.effects.effects.status().is_success());

    // Under continued congestion the cluster, including the restarted
    // validator, must still agree on cancellations. Submissions rejected for
    // stale object references are skipped: the wallet's fullnode view may lag
    // the charged gas objects of the first burst.
    let mut cancelled_after_restart = false;
    'bursts: for _ in 0..5 {
        let accounts_and_objs = context.get_all_accounts_and_gas_objects().await?;
        let batch: Vec<_> = accounts_and_objs
            .iter()
            .flat_map(|(address, objs)| objs.iter().map(|obj| (*address, *obj)))
            .take(10)
            .collect();
        let submissions = batch
            .iter()
            .map(|(address, obj)| submit_transfer(*address, *obj, rgp));
        let results = futures::future::join_all(submissions).await;
        for result in &results {
            let Ok((response, _)) = result else {
                continue;
            };
            if cancelled_congestion_suggested_gas_price(&response.effects.effects).is_some() {
                cancelled_after_restart = true;
                break 'bursts;
            }
        }
    }
    assert!(
        cancelled_after_restart,
        "the cluster must keep cancelling under congestion after the restart"
    );

    Ok(())
}
