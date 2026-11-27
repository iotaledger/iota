// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! NB: Most tests in this module expect real network connections and
//! interactions, thus they should nearly all be tokio::test rather than
//! simtest.

use core::panic;
use std::{num::NonZeroUsize, time::Duration};

use fastcrypto::encoding::Base64;
use iota_core::authority_client::{
    NetworkAuthorityClient, make_network_authority_clients_with_network_config,
    validator::ValidatorAPI, validator_v2::ValidatorV2API,
};
use iota_json_rpc_api::TRANSACTION_NOT_FOUND_ERROR_CODE;
use iota_json_rpc_types::{
    DevInspectResults, DryRunTransactionBlockResponse, IotaMoveViewCallResults, IotaObjectResponse,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, ObjectChange,
};
use iota_macros::sim_test;
use iota_network::default_iota_network_config;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Owner, TransactionDigest, UserSignature};
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_test_transaction_builder::{TestTransactionBuilder, batch_make_transfer_transactions};
use iota_traffic_controller::sim::TrafficSim;
use iota_types::{
    base_types::random_object_ref,
    error::IotaError,
    messages_grpc::{GetTxStatusRequest, TxStatusQuery, TxStatusUpdate},
    quorum_driver_types::ExecuteTransactionRequestType,
    traffic_control::{
        AccessRules, DefaultAction, FreqThresholdConfig, PolicyConfig, PolicyType,
        SimulationGasLimitConfig, TrafficControlReconfigParams, Weight,
    },
    transaction::{SenderSignedTransactionAPI, TransactionEnvelope},
};
use jsonrpsee::{
    core::{client::ClientT, params::ArrayParams},
    rpc_params,
};
use test_cluster::{TestCluster, TestClusterBuilder, override_pcool_flow};

#[tokio::test]
async fn test_validator_traffic_control_noop() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        dry_run: false,
        spam_sample_rate: Weight::one(),
        ..Default::default()
    };
    let network_config = ConfigBuilder::new_with_temp_dir()
        .committee_size(NonZeroUsize::new(4).unwrap())
        .with_policy_config(Some(policy_config))
        .build();
    let test_cluster = TestClusterBuilder::new()
        .set_network_config(network_config)
        .build()
        .await;

    assert_traffic_control_ok(test_cluster).await
}

#[tokio::test]
async fn test_fullnode_traffic_control_noop() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;
    assert_traffic_control_ok(test_cluster).await
}

#[tokio::test]
async fn test_validator_traffic_control_ok() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        // In this test, the validator gRPC API is receiving the fullnode's `submit_tx` and
        // `get_tx_status` requests, which don't count towards the policy, and two `get_tx_status`
        // requests for an already executed transaction, which do. However, the counter is updated
        // only after the response is generated, but the limit is checked before we handle the
        // request, so at the end we end up with 2 handled requests even if the limit is set to 1
        // and only the subsequent request would be rejected. Set the limit to the actual number of
        // requests, so that it's not flaky on slower runners.
        spam_policy_type: PolicyType::TestNConnIP(2),
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        dry_run: false,
        spam_sample_rate: Weight::one(),
        ..Default::default()
    };
    let network_config = ConfigBuilder::new_with_temp_dir()
        .committee_size(NonZeroUsize::new(4).unwrap())
        .with_policy_config(Some(policy_config))
        .build();
    let test_cluster = TestClusterBuilder::new()
        .set_network_config(network_config)
        .build()
        .await;

    assert_traffic_control_ok(test_cluster).await
}

#[tokio::test]
async fn test_fullnode_traffic_control_ok() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        // The following JSON API requests are counted towards this limit:
        // 2 x rpc.discover - those are not sent by the test scenario
        // 5 x iotax_getOwnedObjects
        // 1 x iotax_getReferenceGasPrice
        // 3 x iota_executeTransactionBlock
        // 1 x iota_getTransactionBlock
        spam_policy_type: PolicyType::TestNConnIP(12),
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;
    assert_traffic_control_ok(test_cluster).await
}

#[tokio::test]
async fn test_fullnode_traffic_control_dry_run() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let txn_count = 15;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        spam_policy_type: PolicyType::TestNConnIP(txn_count - 1),
        spam_sample_rate: Weight::one(),
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        dry_run: true,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;

    let context = test_cluster.wallet;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;
    let mut txns = batch_make_transfer_transactions(&context, txn_count as usize).await;
    assert!(
        txns.len() >= txn_count as usize,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new(),
        ExecuteTransactionRequestType::WaitForLocalExecution
    ];

    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params.clone())
        .await
        .unwrap();
    let IotaTransactionBlockResponse {
        digest,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(&digest, tx_digest);
    assert!(confirmed_local_execution.unwrap());

    for _ in 0..txn_count {
        let response: Result<IotaTransactionBlockResponse, _> = jsonrpc_client
            .request("iota_getTransactionBlock", rpc_params![*tx_digest])
            .await;
        assert!(
            response.is_ok(),
            "Expected request to succeed in dry-run mode"
        );
    }
    Ok(())
}

/// Sends `tx` to a validator through the endpoint the flow uses:
/// `handle_transaction` without the P-COOL flow, `submit_tx` with it.
async fn send_to_validator(
    client: &NetworkAuthorityClient,
    tx: &TransactionEnvelope,
    pcool: bool,
) -> Result<(), IotaError> {
    if pcool {
        client.submit_tx(vec![tx.clone()], None).await.map(|_| ())
    } else {
        client
            .handle_transaction(tx.clone(), None)
            .await
            .map(|_| ())
    }
}

#[tokio::test]
async fn test_validator_traffic_control_error_blocked_pre_consensus_flow()
-> Result<(), anyhow::Error> {
    test_validator_traffic_control_error_blocked(false).await
}

#[tokio::test]
async fn test_validator_traffic_control_error_blocked_pcool_flow() -> Result<(), anyhow::Error> {
    test_validator_traffic_control_error_blocked(true).await
}

async fn test_validator_traffic_control_error_blocked(pcool: bool) -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _pcool_guard = override_pcool_flow(pcool);
    let n = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        // Test that any N requests to the gRPC API of the validator will cause an IP to be added to
        // the blocklist. In this test we're directly calling the validator's
        // transaction submission gRPC method to go above the limit.
        error_policy_type: PolicyType::TestNConnIP(n - 1),
        dry_run: false,
        ..Default::default()
    };
    let network_config = ConfigBuilder::new_with_temp_dir()
        .committee_size(NonZeroUsize::new(4).unwrap())
        .with_policy_config(Some(policy_config))
        .build();
    let committee = network_config.committee_with_network();
    let test_cluster = TestClusterBuilder::new()
        .set_network_config(network_config)
        .build()
        .await;
    let local_clients = make_network_authority_clients_with_network_config(
        &committee,
        &default_iota_network_config(),
    );
    let (_, auth_client) = local_clients.first_key_value().unwrap();

    let mut txns = batch_make_transfer_transactions(&test_cluster.wallet, n as usize).await;
    let mut tx = txns.swap_remove(0);
    let signatures = tx.tx_signatures_mut_for_testing();
    signatures.pop();
    signatures.push(UserSignature::Simple(
        iota_types::crypto::zero_ed25519_signature(),
    ));

    // The error policy blocks after n - 1 error tallies, so the block must be
    // observed within n requests.
    for _ in 0..n {
        let response = send_to_validator(auth_client, &tx, pcool).await;
        if let Err(err) = response {
            if err.to_string().contains("Too many requests") {
                return Ok(());
            }
        }
    }
    panic!("Expected error policy to trigger within {n} requests");
}

#[tokio::test]
async fn test_validator_traffic_control_error_blocked_with_policy_reconfig_pre_consensus_flow()
-> Result<(), anyhow::Error> {
    test_validator_traffic_control_error_blocked_with_policy_reconfig(false).await
}

#[tokio::test]
async fn test_validator_traffic_control_error_blocked_with_policy_reconfig_pcool_flow()
-> Result<(), anyhow::Error> {
    test_validator_traffic_control_error_blocked_with_policy_reconfig(true).await
}

async fn test_validator_traffic_control_error_blocked_with_policy_reconfig(
    pcool: bool,
) -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let _pcool_guard = override_pcool_flow(pcool);
    let n = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 100,
        error_policy_type: PolicyType::TestNConnIP(n - 1),
        dry_run: true,
        ..Default::default()
    };
    let network_config = ConfigBuilder::new_with_temp_dir()
        .committee_size(NonZeroUsize::new(4).unwrap())
        .with_policy_config(Some(policy_config))
        .build();
    let committee = network_config.committee_with_network();
    let test_cluster = TestClusterBuilder::new()
        .set_network_config(network_config)
        .build()
        .await;
    let local_clients = make_network_authority_clients_with_network_config(
        &committee,
        &default_iota_network_config(),
    );
    let (_, auth_client) = local_clients.first_key_value().unwrap();

    let mut txns = batch_make_transfer_transactions(&test_cluster.wallet, n as usize).await;
    let mut tx = txns.swap_remove(0);
    let signatures = tx.tx_signatures_mut_for_testing();
    signatures.pop();
    signatures.push(UserSignature::Simple(
        iota_types::crypto::zero_ed25519_signature(),
    ));

    // Before reconfiguring the policy, we should not block any requests due to dry
    // run mode, even after far exceeding the threshold. However the blocklist
    // should be updated.
    for _ in 0..(2 * n) {
        let response = send_to_validator(auth_client, &tx, pcool).await;
        if let Err(err) = response {
            assert!(
                !err.to_string().contains("Too many requests"),
                "Expected no blocked requests due to dry run mode"
            );
        }
    }
    // Reconfigure traffic control to disable dry run mode
    for node in test_cluster.all_validator_handles() {
        node.state()
            .reconfigure_traffic_control(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(false),
                connection_blocklist_ttl_sec: None,
                proxy_blocklist_ttl_sec: None,
                ..Default::default()
            })
            .await
            .unwrap();
    }
    // If Node and TrafficController has not crashed, blocklist and policy freq
    // state should still be intact. A single additional erroneous request from
    // the client should trigger enforcement.
    let response = send_to_validator(auth_client, &tx, pcool).await;
    if let Err(err) = response {
        if err.to_string().contains("Too many requests") {
            return Ok(());
        }
    }
    panic!("Expected error policy to trigger on next requests after reconfiguration");
}

/// A fullnode reserves the computation limit of one transaction for each
/// simulation, charges the simulation what it used, and refuses a client's
/// simulation once the client's gas balance is below that limit.
#[tokio::test]
async fn test_fullnode_simulation_gas_limit() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let computation_limit =
        ProtocolConfig::get_for_max_version_UNSAFE().max_gas_computation_bucket();
    let policy_config = PolicyConfig {
        // Room for two reservations of the computation limit. It refills too
        // slowly to matter here.
        simulation_gas_limit: Some(SimulationGasLimitConfig {
            gas_per_sec: 1,
            burst_gas: 2 * computation_limit,
        }),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;
    let client = test_cluster.grpc_client();
    let (sender, gas) = test_cluster.wallet.get_one_gas_object().await?.unwrap();
    let reference_gas_price = test_cluster.get_reference_gas_price().await;
    let transaction = TestTransactionBuilder::new(sender, gas, reference_gas_price)
        .transfer_iota(None, sender)
        .build();

    // Each simulation gets back all of the computation limit but the little
    // a transfer uses, so the balance stays above that limit.
    for _ in 0..2 {
        client
            .simulate_transaction(transaction.clone())
            .await
            .expect("a balance above the computation limit is within the gas limit");
    }
    // The balance is cut to the new burst at its next refill.
    test_cluster
        .fullnode_handle
        .iota_node
        .state()
        .reconfigure_traffic_control(TrafficControlReconfigParams {
            simulation_burst_gas: Some(computation_limit),
            ..Default::default()
        })
        .await?;
    client
        .simulate_transaction(transaction.clone())
        .await
        .expect("a full balance is within the limit");
    let error = client
        .simulate_transaction(transaction)
        .await
        .expect_err("the transfer left the balance below the computation limit");
    assert!(
        error.to_string().contains("simulation gas limit reached"),
        "{error}"
    );
    Ok(())
}

/// A fullnode charges a client's JSON-RPC dry runs, dev inspects and view
/// calls to the client's simulation gas balance, which its gRPC simulations
/// share, at the gas price the simulation ran with. It refuses them with
/// `ServerIsBusy` once the balance is below the computation limit of one
/// transaction, and still serves the client's other requests.
#[tokio::test]
async fn test_fullnode_json_rpc_simulation_gas_limit() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let computation_limit =
        ProtocolConfig::get_for_max_version_UNSAFE().max_gas_computation_bucket();
    let policy_config = PolicyConfig {
        // Room for two reservations of the computation limit. It refills too
        // slowly to matter here.
        simulation_gas_limit: Some(SimulationGasLimitConfig {
            gas_per_sec: 1,
            burst_gas: 2 * computation_limit,
        }),
        // A refusal must not feed the error policy.
        error_policy_type: PolicyType::TestNConnIP(1),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;
    let view_call = publish_view_functions_example(&test_cluster).await;
    let (sender, gas) = test_cluster.wallet.get_one_gas_object().await?.unwrap();
    let reference_gas_price = test_cluster.get_reference_gas_price().await;
    let transaction = TestTransactionBuilder::new(sender, gas, reference_gas_price)
        .transfer_iota(None, sender)
        .build();
    let (transaction_bytes, kind_bytes) = transaction_and_kind_bytes(&transaction);

    // A view call and a dev inspect leave the gas price to the simulation.
    // Each is charged the little it used at that price, so the balance stays
    // above the computation limit for the dry run. A charge of the whole
    // computation limit would leave too little for it.
    let results: IotaMoveViewCallResults = jsonrpc_client
        .request("iota_view", view_call.clone())
        .await
        .expect("a view call is within the limit");
    assert!(results.error().is_none(), "{results:?}");
    let results: DevInspectResults = jsonrpc_client
        .request(
            "iota_devInspectTransactionBlock",
            rpc_params![sender, kind_bytes.clone()],
        )
        .await
        .expect("a dev inspect is within the limit");
    assert!(results.error.is_none(), "{results:?}");
    let _: DryRunTransactionBlockResponse = jsonrpc_client
        .request(
            "iota_dryRunTransactionBlock",
            rpc_params![transaction_bytes.clone()],
        )
        .await
        .expect("a dry run is within the limit");
    // The balance is cut to the new burst at its next refill.
    test_cluster
        .fullnode_handle
        .iota_node
        .state()
        .reconfigure_traffic_control(TrafficControlReconfigParams {
            simulation_burst_gas: Some(computation_limit),
            ..Default::default()
        })
        .await?;
    let _: DryRunTransactionBlockResponse = jsonrpc_client
        .request(
            "iota_dryRunTransactionBlock",
            rpc_params![transaction_bytes.clone()],
        )
        .await
        .expect("a full balance is within the limit");

    let error = jsonrpc_client
        .request::<DryRunTransactionBlockResponse, _>(
            "iota_dryRunTransactionBlock",
            rpc_params![transaction_bytes],
        )
        .await
        .expect_err("the dry run left the balance below the computation limit");
    assert_simulation_gas_refusal(error);
    let error = jsonrpc_client
        .request::<DevInspectResults, _>(
            "iota_devInspectTransactionBlock",
            rpc_params![sender, kind_bytes],
        )
        .await
        .expect_err("a dev inspect over the limit is refused");
    assert_simulation_gas_refusal(error);
    let error = jsonrpc_client
        .request::<IotaMoveViewCallResults, _>("iota_view", view_call)
        .await
        .expect_err("a view call over the limit is refused");
    assert_simulation_gas_refusal(error);
    // gRPC simulations take from the same balance.
    let error = test_cluster
        .grpc_client()
        .simulate_transaction(transaction)
        .await
        .expect_err("a gRPC simulation over the limit is refused");
    assert!(
        error
            .to_string()
            .contains("simulation gas limit reached, retry in "),
        "{error}"
    );
    let _: IotaObjectResponse = jsonrpc_client
        .request("iota_getObject", rpc_params![gas.object_id])
        .await
        .expect("the client's other requests are served");
    Ok(())
}

/// A JSON-RPC simulation gives back all the gas it reserved when it fails
/// before it runs, and when the node already runs as many simulations as it
/// allows. A view call, which runs as a dev inspect, is charged what it ran.
#[tokio::test]
async fn test_fullnode_json_rpc_simulation_gives_back_unused_gas() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    let computation_limit = protocol_config.max_gas_computation_bucket();
    let policy_config = PolicyConfig {
        // One reservation of the computation limit, and less spare gas than the
        // gas rounding step that a simulation is charged at least. It refills
        // too slowly to matter here.
        simulation_gas_limit: Some(SimulationGasLimitConfig {
            gas_per_sec: 1,
            burst_gas: computation_limit + protocol_config.gas_rounding_step() / 2,
        }),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;
    let view_call = publish_view_functions_example(&test_cluster).await;
    let (sender, gas) = test_cluster.wallet.get_one_gas_object().await?.unwrap();
    let reference_gas_price = test_cluster.get_reference_gas_price().await;
    let (transaction_bytes, kind_bytes) = transaction_and_kind_bytes(
        &TestTransactionBuilder::new(sender, gas, reference_gas_price)
            .transfer_iota(None, sender)
            .build(),
    );
    // A transfer of an object that does not exist fails before it runs.
    let (failing_transaction_bytes, failing_kind_bytes) = transaction_and_kind_bytes(
        &TestTransactionBuilder::new(sender, gas, reference_gas_price)
            .transfer(random_object_ref(), sender)
            .build(),
    );
    let server_is_busy = jsonrpsee::types::error::ErrorCode::ServerIsBusy.code();
    let error_code = |error: jsonrpsee::core::ClientError| match error {
        jsonrpsee::core::ClientError::Call(error) => error.code(),
        other => panic!("expected an error from the method, got {other}"),
    };

    // Without a refund the second attempt would be refused for gas.
    for _ in 0..2 {
        let error = jsonrpc_client
            .request::<DryRunTransactionBlockResponse, _>(
                "iota_dryRunTransactionBlock",
                rpc_params![failing_transaction_bytes.clone()],
            )
            .await
            .expect_err("a dry run of a missing object fails");
        assert_ne!(error_code(error), server_is_busy);
        let error = jsonrpc_client
            .request::<DevInspectResults, _>(
                "iota_devInspectTransactionBlock",
                rpc_params![sender, failing_kind_bytes.clone()],
            )
            .await
            .expect_err("a dev inspect of a missing object fails");
        assert_ne!(error_code(error), server_is_busy);
    }

    // Each simulation reserves the computation limit before the node refuses
    // it for want of a permit. Without a refund the second attempt would be
    // refused for gas.
    let state = test_cluster.fullnode_handle.iota_node.state();
    let permits = std::iter::from_fn(|| state.try_acquire_simulation_permit()).collect::<Vec<_>>();
    assert!(!permits.is_empty());
    for _ in 0..2 {
        let error = jsonrpc_client
            .request::<DryRunTransactionBlockResponse, _>(
                "iota_dryRunTransactionBlock",
                rpc_params![transaction_bytes.clone()],
            )
            .await
            .expect_err("a dry run without a permit is refused");
        assert_simulation_permit_refusal(error);
        let error = jsonrpc_client
            .request::<DevInspectResults, _>(
                "iota_devInspectTransactionBlock",
                rpc_params![sender, kind_bytes.clone()],
            )
            .await
            .expect_err("a dev inspect without a permit is refused");
        assert_simulation_permit_refusal(error);
        let error = jsonrpc_client
            .request::<IotaMoveViewCallResults, _>("iota_view", view_call.clone())
            .await
            .expect_err("a view call without a permit is refused");
        assert_simulation_permit_refusal(error);
    }
    drop(permits);

    // The balance is still full. The view call is charged at least the gas
    // rounding step, which leaves less than the computation limit.
    let results: IotaMoveViewCallResults = jsonrpc_client
        .request("iota_view", view_call)
        .await
        .expect("a full balance is within the limit");
    assert!(results.error().is_none(), "{results:?}");
    let error = jsonrpc_client
        .request::<DevInspectResults, _>(
            "iota_devInspectTransactionBlock",
            rpc_params![sender, kind_bytes],
        )
        .await
        .expect_err("the view call left the balance below the computation limit");
    assert_simulation_gas_refusal(error);
    Ok(())
}

/// Publishes the `view_functions` example package, and returns the parameters
/// of an `iota_view` call of its `counter::value` on the shared counter that
/// the package creates.
async fn publish_view_functions_example(test_cluster: &TestCluster) -> ArrayParams {
    let publish = test_cluster
        .test_transaction_builder_with_sender(test_cluster.get_address_0())
        .await
        .publish_examples("view_functions")
        .build();
    let object_changes = test_cluster
        .wallet
        .execute_transaction_must_succeed(test_cluster.wallet.sign_transaction(&publish))
        .await
        .object_changes
        .expect("the response has the object changes");
    let package_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .expect("the package is published");
    let counter_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Created {
                object_id,
                owner: Owner::Shared(_),
                ..
            } => Some(*object_id),
            _ => None,
        })
        .expect("the package creates a shared counter");
    rpc_params![
        format!("{package_id}::counter::value"),
        Option::<Vec<String>>::None,
        vec![counter_id.to_string()]
    ]
}

/// The BCS bytes of `transaction` and of its kind, as JSON-RPC dry runs and
/// dev inspects take them.
fn transaction_and_kind_bytes(transaction: &iota_sdk_types::Transaction) -> (Base64, Base64) {
    let iota_sdk_types::Transaction::V1(transaction_v1) = transaction else {
        unreachable!("the test transactions are V1")
    };
    (
        Base64::from_bytes(&bcs::to_bytes(transaction).unwrap()),
        Base64::from_bytes(&bcs::to_bytes(&transaction_v1.kind).unwrap()),
    )
}

/// Asserts that `error` is the refusal of a simulation over its client's gas
/// limit: `ServerIsBusy` with `data.reason` `simulation_gas_limit_reached`,
/// and the time to retry in the message and in `data.retry_after_secs`.
fn assert_simulation_gas_refusal(error: jsonrpsee::core::ClientError) {
    let error = match error {
        jsonrpsee::core::ClientError::Call(error) => error,
        other => panic!("expected an error from the method, got {other}"),
    };
    assert_eq!(
        error.code(),
        jsonrpsee::types::error::ErrorCode::ServerIsBusy.code(),
        "{error}"
    );
    let retry_secs: u64 = error
        .message()
        .strip_prefix("simulation gas limit reached, retry in ")
        .and_then(|rest| rest.strip_suffix('s'))
        .and_then(|secs| secs.parse().ok())
        .unwrap_or_else(|| panic!("no retry time in the message: {error}"));
    // The client can read the same retry time without parsing the message.
    let data: serde_json::Value =
        serde_json::from_str(error.data().expect("the refusal has data").get())
            .expect("the data is JSON");
    assert_eq!(data["retry_after_secs"], retry_secs, "{error}");
    assert_eq!(data["reason"], "simulation_gas_limit_reached", "{error}");
}

/// Asserts that `error` is the refusal of a simulation while the node runs as
/// many simulations as it allows: `ServerIsBusy`, and not a refusal for gas.
fn assert_simulation_permit_refusal(error: jsonrpsee::core::ClientError) {
    let error = match error {
        jsonrpsee::core::ClientError::Call(error) => error,
        other => panic!("expected an error from the method, got {other}"),
    };
    assert_eq!(
        error.code(),
        jsonrpsee::types::error::ErrorCode::ServerIsBusy.code(),
        "{error}"
    );
    assert!(
        error
            .message()
            .contains("running as many simulations as it allows"),
        "{error}"
    );
}

#[tokio::test]
async fn test_fullnode_traffic_control_spam_blocked() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let txn_count = 15;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        // Test that any N requests will cause an IP to be added to the blocklist. In the test we
        // are performing `txn_count` `iota_getTransactionBlock` calls to the fullnode's JSON API,
        // but additionally before that, we're performing other requests, but using `txn_count - 1`
        // as the limit is enough to test the blocking functionality.
        spam_policy_type: PolicyType::TestNConnIP(txn_count - 1),
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;

    let context = test_cluster.wallet;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let mut txns = batch_make_transfer_transactions(&context, txn_count as usize).await;
    assert!(
        txns.len() >= txn_count as usize,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new(),
        ExecuteTransactionRequestType::WaitForLocalExecution
    ];

    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params.clone())
        .await
        .unwrap();
    let IotaTransactionBlockResponse {
        digest,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(&digest, tx_digest);
    assert!(confirmed_local_execution.unwrap());

    for _ in 0..txn_count {
        let response: Result<IotaTransactionBlockResponse, _> = jsonrpc_client
            .request("iota_getTransactionBlock", rpc_params![*tx_digest])
            .await;
        if let Err(err) = response {
            // TODO: fix validator blocking error handling such that the error message
            // is not misleading. The full error message currently is the following:
            //  Transaction execution failed due to issues with transaction inputs, please
            //  review the errors and try again: Too many requests.
            assert!(
                err.to_string().contains("Too many requests"),
                "Error not due to spam policy"
            );
            return Ok(());
        }
    }
    panic!("Expected spam policy to trigger within {txn_count} requests");
}

#[tokio::test]
async fn test_fullnode_traffic_control_error_blocked() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let txn_count = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        error_policy_type: PolicyType::TestNConnIP(txn_count - 1),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;

    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;
    let context = test_cluster.wallet;

    let mut txns = batch_make_transfer_transactions(&context, txn_count as usize).await;
    assert!(
        txns.len() >= txn_count as usize,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    for _ in 0..txn_count {
        let txn = txns.swap_remove(0);
        let tx_digest = txn.digest();
        let (tx_bytes, _signatures) = txn.to_tx_bytes_and_signatures();
        // create invalid (empty) client signature
        let signatures: Vec<Base64> = vec![];
        let params = rpc_params![
            tx_bytes,
            signatures,
            IotaTransactionBlockResponseOptions::new(),
            ExecuteTransactionRequestType::WaitForLocalExecution
        ];
        let response: Result<IotaTransactionBlockResponse, _> = jsonrpc_client
            .request("iota_executeTransactionBlock", params.clone())
            .await;
        if let Err(err) = response {
            if err.to_string().contains("Too many requests") {
                return Ok(());
            }
        } else {
            let IotaTransactionBlockResponse {
                digest,
                confirmed_local_execution,
                ..
            } = response.unwrap();
            assert_eq!(&digest, tx_digest);
            assert!(confirmed_local_execution.unwrap());
        }
    }
    panic!("Expected error policy to trigger within {txn_count} requests");
}

#[tokio::test]
async fn unknown_transaction_does_not_trigger_fullnode_error_policy() {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 3,
        error_policy_type: PolicyType::TestNConnIP(1),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .build()
        .await;

    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;
    let digest = TransactionDigest::from([42; 32]);

    for request_number in 1..=2 {
        let response: Result<IotaTransactionBlockResponse, _> = jsonrpc_client
            .request("iota_getTransactionBlock", rpc_params![digest])
            .await;
        let error = response.expect_err("an unknown transaction should return an error");
        let jsonrpsee::core::ClientError::Call(error) = error else {
            panic!("request {request_number} returned a non-JSON-RPC error: {error}");
        };
        assert_eq!(error.code(), TRANSACTION_NOT_FOUND_ERROR_CODE);

        if request_number == 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

#[sim_test]
async fn test_traffic_sim_no_blocks() {
    telemetry_subscribers::init_for_testing();
    let policy = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        spam_policy_type: PolicyType::FreqThreshold(FreqThresholdConfig {
            client_threshold: 5_050,
            proxied_client_threshold: 5_050,
            burst_secs: 4,
        }),
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy,
        10,    // num_clients
        5_000, // per_client_tps
        Duration::from_secs(20),
    )
    .await;

    let expected_requests = 5_000 * 10 * 20;
    assert!(metrics.num_blocked < 5_005);
    assert!(metrics.num_requests > expected_requests - 1_000);
    assert!(metrics.num_requests < expected_requests + 200);
    assert!(metrics.num_blocklist_adds <= 1);
    if let Some(first_block) = metrics.abs_time_to_first_block {
        assert!(first_block > Duration::from_secs(2));
    }
}

#[sim_test]
async fn test_traffic_sim_with_blocks() {
    telemetry_subscribers::init_for_testing();
    let policy = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        spam_policy_type: PolicyType::FreqThreshold(FreqThresholdConfig {
            client_threshold: 5_000,
            proxied_client_threshold: 5_000,
            burst_secs: 1,
        }),
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy,
        10,     // num_clients
        10_000, // per_client_tps
        Duration::from_secs(20),
    )
    .await;

    let expected_requests = 10_000 * 10 * 20;
    assert!(metrics.num_requests > expected_requests - 1_000);
    assert!(metrics.num_requests < expected_requests + 200);
    // At 10_000/s against a 5_000/s quota, the 5_000 cell burst drains in ~1s,
    // and fully replenishes over the 1s the client spends blocked. The cycle is
    // ~1s unblocked + ~1s blocked, giving roughly half of all requests blocked.
    assert!(metrics.num_blocked > expected_requests / 3);
    assert!(metrics.num_blocked < (expected_requests * 2) / 3);
    // 10 clients, blocked roughly every 2 seconds over 20 seconds
    assert!(metrics.num_blocklist_adds >= 40);
    assert!(metrics.abs_time_to_first_block.unwrap() < Duration::from_secs(3));
}

#[sim_test]
async fn test_traffic_sim_with_sampled_spam() {
    telemetry_subscribers::init_for_testing();
    let policy = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        spam_policy_type: PolicyType::FreqThreshold(FreqThresholdConfig {
            client_threshold: 250,
            proxied_client_threshold: 250,
            burst_secs: 1,
        }),
        spam_sample_rate: Weight::new(0.5).unwrap(),
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy,
        1,    // num_clients
        1000, // per_client_tps
        Duration::from_secs(20),
    )
    .await;

    let expected_requests = 1000 * 20;
    assert!(metrics.num_requests > expected_requests - 100);
    assert!(metrics.num_requests < expected_requests + 20);
    // 1000 requests/s sampled at 0.5 charges ~500/s against a 250/s quota, the
    // same 2x overage as the unsampled case, so the client spends roughly half
    // its time blocked.
    assert!(metrics.num_blocked > expected_requests / 3);
}

#[sim_test]
async fn test_traffic_sim_default_deny() {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        // first two clients allowed, rest refused
        access_rules: AccessRules {
            allow: vec![String::from("127.0.0.0"), String::from("127.0.0.1")],
            default: DefaultAction::Deny,
            ..Default::default()
        },
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy_config,
        4,   // num_clients
        100, // per_client_tps
        Duration::from_secs(2),
    )
    .await;

    let expected_requests = 100 * 2 * 4;
    // ~half of all requests blocked
    assert!(metrics.num_blocked >= expected_requests / 2 - 10);
    assert!(metrics.num_requests > expected_requests - 10);
    assert!(metrics.num_requests < expected_requests + 20);
}

async fn assert_traffic_control_ok(mut test_cluster: TestCluster) -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let context = &mut test_cluster.wallet;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let txn_count = 4;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();
    let first_digest = *tx_digest;

    // Test request with ExecuteTransactionRequestType::WaitForLocalExecution
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new(),
        ExecuteTransactionRequestType::WaitForLocalExecution
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        digest,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(&digest, tx_digest);
    assert!(confirmed_local_execution.unwrap());

    let _response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_getTransactionBlock", rpc_params![*tx_digest])
        .await
        .unwrap();

    // Test request with ExecuteTransactionRequestType::WaitForEffectsCert
    // Use the same txn which should return local finalized effects
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new().with_effects(),
        ExecuteTransactionRequestType::WaitForEffectsCert
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        effects,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(effects.unwrap().transaction_digest(), tx_digest);
    assert!(confirmed_local_execution.unwrap());

    // Test request with ExecuteTransactionRequestType::WaitForEffectsCert
    // Use a different txn to avoid the case where the txn effects are already
    // cached locally
    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();

    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new().with_effects(),
        ExecuteTransactionRequestType::WaitForEffectsCert
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        effects,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(effects.unwrap().transaction_digest(), tx_digest);
    assert!(!confirmed_local_execution.unwrap());

    // Only a `get_tx_status` of an already executed transaction counts towards
    // the spam policy of a validator. Send two to one validator.
    let client = test_cluster
        .authority_aggregator()
        .authority_clients
        .values()
        .next()
        .unwrap()
        .clone();
    for _ in 0..2 {
        let request = GetTxStatusRequest {
            queries: vec![TxStatusQuery {
                transaction_digest: first_digest,
                include_details: false,
            }],
        };
        let statuses = client
            .authority_client()
            .get_tx_status(request, None)
            .await?;
        assert!(
            matches!(statuses[..], [(_, TxStatusUpdate::Executed { .. })]),
            "expected the executed transaction's status, got {statuses:?}"
        );
    }

    Ok(())
}
