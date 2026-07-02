// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! NB: Most tests in this module expect real network connections and
//! interactions, thus they should nearly all be tokio::test rather than
//! simtest.

use core::panic;
use std::{fs::File, num::NonZeroUsize, time::Duration};

use iota_core::{
    authority_client::{
        make_network_authority_clients_with_network_config, validator::ValidatorAPI,
    },
    traffic_controller::{TrafficController, TrafficSim, nodefw_test_server::NodeFwTestServer},
};
use iota_grpc_client::{ReadMask, read_mask_fields::TransactionField};
use iota_macros::sim_test;
use iota_network::default_iota_network_config;
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_test_transaction_builder::batch_make_transfer_transactions;
use iota_types::{
    crypto::Ed25519IotaSignature,
    effects::{TransactionEffects, TransactionEffectsAPI},
    signature::GenericSignature,
    traffic_control::{
        FreqThresholdConfig, PolicyConfig, PolicyType, RemoteFirewallConfig,
        TrafficControlReconfigParams, Weight,
    },
    transaction::Transaction,
};
use test_cluster::{TestCluster, TestClusterBuilder};

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
        .with_fullnode_enable_grpc_api(true)
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
        .with_fullnode_enable_grpc_api(true)
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
        // The test scenario executes the same transaction twice; the validator gRPC
        // API receives some requests that don't count towards the policy (fresh
        // transaction / certificate submissions have zero spam weight) and, from the
        // repeat submission, requests that do
        // (/iota.validator.Validator/CertifiedTransactionV1 for an already executed
        // transaction). The counter is updated only after the response is generated,
        // while the limit is checked before the request is handled, so at-the-limit
        // traffic is still served. Set the limit to the number of counted requests,
        // so that it's not flaky on slower runners.
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
        .with_fullnode_enable_grpc_api(true)
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
        // The following fullnode requests are counted towards this limit:
        // 2 x rpc.discover (JSON-RPC, sent by the wallet client init)
        // 5 x iotax_getOwnedObjects (JSON-RPC, sent by the wallet)
        // 1 x iotax_getReferenceGasPrice (JSON-RPC, sent by the wallet)
        // 2 x ExecuteTransaction (gRPC)
        // 1 x GetReferenceGasPrice (gRPC)
        spam_policy_type: PolicyType::TestNConnIP(11),
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;
    assert_traffic_control_ok(test_cluster).await
}

#[tokio::test]
async fn test_validator_traffic_control_dry_run() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let n = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        spam_policy_type: PolicyType::TestNConnIP(n - 1),
        spam_sample_rate: Weight::one(),
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        dry_run: true,
        ..Default::default()
    };
    let network_config = ConfigBuilder::new_with_temp_dir()
        .committee_size(NonZeroUsize::new(4).unwrap())
        .with_policy_config(Some(policy_config))
        .build();
    let test_cluster = TestClusterBuilder::new()
        .set_network_config(network_config)
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    assert_validator_traffic_control_dry_run(test_cluster, n as usize).await
}

#[tokio::test]
async fn test_fullnode_traffic_control_dry_run() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let n = 15;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 5,
        spam_policy_type: PolicyType::TestNConnIP(n - 1),
        spam_sample_rate: Weight::one(),
        // This should never be invoked when set as an error policy
        // as we are not sending requests that error
        error_policy_type: PolicyType::TestPanicOnInvocation,
        dry_run: true,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let client = test_cluster.grpc_client();

    // In dry-run mode, spamming past the limit must NOT block any request.
    for _ in 0..n {
        client
            .get_reference_gas_price()
            .await
            .expect("request should succeed in dry-run mode");
        // Yield so the background `run_tally_loop` task can run between requests.
        tokio::task::yield_now().await;
    }
    Ok(())
}

#[tokio::test]
async fn test_validator_traffic_control_error_blocked() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let n = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        // Test that any N requests to the gRPC API of the validator will cause an IP to be added to
        // the blocklist. In this test we're directly calling
        // `/iota.validator.Validator/Transaction` gRPC method to go above the limit.
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
    signatures.push(GenericSignature::Signature(
        iota_types::crypto::Signature::Ed25519IotaSignature(Ed25519IotaSignature::default()),
    ));

    // it should take no more than 4 requests to be added to the blocklist
    for _ in 0..n {
        let response = auth_client.handle_transaction(tx.clone(), None).await;
        if let Err(err) = response {
            if err.to_string().contains("Too many requests") {
                return Ok(());
            }
        }
        // Yield to the async executor so that the background `run_tally_loop` task
        // can process the pending tally and update the blocklist before the next
        // request. Without this, the single-threaded tokio test runtime may never
        // schedule the tally loop between iterations, causing the test to be flaky.
        tokio::task::yield_now().await;
    }
    panic!("Expected error policy to trigger within {n} requests");
}

#[tokio::test]
async fn test_validator_traffic_control_error_blocked_with_policy_reconfig()
-> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
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
    signatures.push(GenericSignature::Signature(
        iota_types::crypto::Signature::Ed25519IotaSignature(Ed25519IotaSignature::default()),
    ));

    // Before reconfiguring the policy, we should not block any requests due to dry
    // run mode, even after far exceeding the threshold. However the blocklist
    // should be updated.
    for _ in 0..(2 * n) {
        let response = auth_client.handle_transaction(tx.clone(), None).await;
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
            })
            .await
            .unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    // If Node and TrafficController has not crashed, blocklist and policy freq
    // state should still be intact. A single additional erroneous request from
    // the client should trigger enforcement.
    let response = auth_client.handle_transaction(tx.clone(), None).await;
    if let Err(err) = response {
        if err.to_string().contains("Too many requests") {
            return Ok(());
        }
    }
    panic!("Expected error policy to trigger on next requests after reconfiguration");
}

#[tokio::test]
async fn test_fullnode_traffic_control_spam_blocked() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let n = 15;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 3,
        // Any N spam requests to the fullnode's gRPC API must add the IP to the
        // blocklist. We set the limit to `n - 1` and send up to `n` requests.
        spam_policy_type: PolicyType::TestNConnIP(n - 1),
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let client = test_cluster.grpc_client();

    // Spam the fullnode gRPC endpoint; the spam policy must block the IP within
    // `n` requests.
    for _ in 0..n {
        match client.get_reference_gas_price().await {
            Ok(_) => {}
            Err(err) => {
                assert!(
                    err.to_string().contains("Too many requests"),
                    "Error not due to spam policy: {err}"
                );
                return Ok(());
            }
        }
        // Yield so the background `run_tally_loop` task can process the pending
        // tally and update the blocklist before the next request.
        tokio::task::yield_now().await;
    }
    panic!("Expected spam policy to trigger within {n} requests");
}

// NB: there is no fullnode error-policy test here. That behavior is covered
// by integration tests in `crates/iota-grpc-server/tests/traffic_control.rs`,
// including errors that batch APIs embed in an otherwise successful response
// (e.g. an invalid transaction signature), which the transport-level
// `TrafficControlLayer` cannot see on its own. The validator gRPC path is
// covered by `test_validator_traffic_control_error_blocked` /
// `test_validator_traffic_control_error_delegated`.

#[tokio::test]
async fn test_validator_traffic_control_error_delegated() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let n = 5;
    let port = 65000;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        proxy_blocklist_ttl_sec: 120,
        // Test that any N - 1 requests will cause an IP to be added to the blocklist.
        error_policy_type: PolicyType::TestNConnIP(n - 1),
        dry_run: false,
        ..Default::default()
    };
    // enable remote firewall delegation
    let tmp_dir = iota_common::tempdir();
    let firewall_config = RemoteFirewallConfig {
        remote_fw_url: format!("http://127.0.0.1:{port}"),
        delegate_spam_blocking: true,
        delegate_error_blocking: false,
        destination_port: 8080,
        drain_path: tmp_dir.path().join("drain"),
        drain_timeout_secs: 10,
    };
    let network_config = ConfigBuilder::new_with_temp_dir()
        .committee_size(NonZeroUsize::new(4).unwrap())
        .with_policy_config(Some(policy_config))
        .with_firewall_config(Some(firewall_config))
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
    signatures.push(GenericSignature::Signature(
        iota_types::crypto::Signature::Ed25519IotaSignature(Ed25519IotaSignature::default()),
    ));

    // start test firewall server
    let mut server = NodeFwTestServer::new();
    server.start(port).await;
    // await for the server to start
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // it should take no more than 4 requests to be added to the blocklist
    for _ in 0..n {
        let response = auth_client.handle_transaction(tx.clone(), None).await;
        if let Err(err) = response {
            if err.to_string().contains("Too many requests") {
                return Ok(());
            }
        }
        // Yield to the async executor so that the background `run_tally_loop` task
        // can process the pending tally and update the blocklist before the next
        // request. Without this, the single-threaded tokio test runtime may never
        // schedule the tally loop between iterations, causing the test to be flaky.
        tokio::task::yield_now().await;
    }
    // Allow time for the async HTTP delegation to the firewall server to complete.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let fw_blocklist = server.list_addresses_rpc().await;
    assert!(
        !fw_blocklist.is_empty(),
        "Expected blocklist to be non-empty"
    );
    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn test_fullnode_traffic_control_spam_delegated() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let txn_count = 10;
    let port = 65001;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        proxy_blocklist_ttl_sec: 120,
        // Test that any N - 1 requests will cause an IP to be added to the blocklist.
        spam_policy_type: PolicyType::TestNConnIP(txn_count - 1),
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    // enable remote firewall delegation
    let tmp_dir = iota_common::tempdir();
    let firewall_config = RemoteFirewallConfig {
        remote_fw_url: format!("http://127.0.0.1:{port}"),
        delegate_spam_blocking: true,
        delegate_error_blocking: false,
        destination_port: 9000,
        drain_path: tmp_dir.path().join("drain"),
        drain_timeout_secs: 10,
    };
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_policy_config(Some(policy_config))
        .with_fullnode_fw_config(Some(firewall_config.clone()))
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // start test firewall server
    let mut server = NodeFwTestServer::new();
    server.start(port).await;
    // await for the server to start
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    let client = test_cluster.grpc_client();

    // Spam the gRPC endpoint. Spam blocking is delegated to the remote firewall,
    // so requests keep succeeding locally while the firewall records the block.
    for _ in 0..txn_count {
        let _ = client.get_reference_gas_price().await;
        // Yield to the async executor so that the background `run_tally_loop` task
        // can process the pending tally and delegate to the firewall before the
        // next request.
        tokio::task::yield_now().await;
    }
    // Allow time for the async HTTP delegation to the firewall server to complete.
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let fw_blocklist = server.list_addresses_rpc().await;
    assert!(
        !fw_blocklist.is_empty(),
        "Expected blocklist to be non-empty"
    );
    server.stop().await;
    Ok(())
}

#[tokio::test]
async fn test_traffic_control_dead_mans_switch() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 3,
        spam_policy_type: PolicyType::TestNConnIP(10),
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };

    // sink all traffic to trigger dead mans switch
    let tmp_dir = iota_common::tempdir();
    let drain_path = tmp_dir.path().join("drain");
    assert!(!drain_path.exists(), "Expected drain file to not yet exist",);

    let firewall_config = RemoteFirewallConfig {
        remote_fw_url: String::from("http://127.0.0.1:65000"),
        delegate_spam_blocking: true,
        delegate_error_blocking: false,
        destination_port: 9000,
        drain_path: drain_path.clone(),
        drain_timeout_secs: 6,
    };

    let tc = TrafficController::init_for_test(policy_config.clone(), Some(firewall_config.clone()))
        .await;
    assert!(
        !drain_path.exists(),
        "Expected drain file to not exist after startup unless previously set",
    );

    // after n seconds with no traffic, the dead mans switch should be engaged
    let mut drain_enabled = false;
    for _ in 0..4 {
        if drain_path.exists() {
            drain_enabled = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
    assert!(drain_enabled, "Expected drain file to be enabled");

    // if we drop traffic controller and re-instantiate, drain file should remain
    // set
    drop(tc);
    let _tc = TrafficController::init_for_test(policy_config, Some(firewall_config)).await;
    for _ in 0..3 {
        assert!(
            drain_path.exists(),
            "Expected drain file to be disabled at startup unless previously enabled",
        );
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }

    Ok(())
}

#[tokio::test]
async fn test_traffic_control_manual_set_dead_mans_switch() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let tmp_dir = iota_common::tempdir();
    let drain_path = tmp_dir.path().join("drain");
    assert!(!drain_path.exists(), "Expected drain file to not yet exist",);
    File::create(&drain_path).expect("Failed to touch nodefw drain file");
    assert!(drain_path.exists(), "Expected drain file to exist",);

    Ok(())
}

#[sim_test]
async fn test_traffic_sketch_no_blocks() {
    telemetry_subscribers::init_for_testing();
    let sketch_config = FreqThresholdConfig {
        client_threshold: 5_050,
        proxied_client_threshold: 5_050,
        window_size_secs: 4,
        update_interval_secs: 1,
        ..Default::default()
    };
    let policy = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        spam_policy_type: PolicyType::FreqThreshold(sketch_config),
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        // keeping channel capacity small results in less errors in test metrics,
        // in case of congestion (due to running on slower hardware) requests are dropped
        // and do not influence the rate and do not make the spam rate inconsistent
        channel_capacity: 10,
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy,
        10,    // num_clients
        5_000, // per_client_tps
        Duration::from_secs(20),
        true, // report
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
    assert!(metrics.num_blocklist_adds < 10);
    assert!(metrics.total_time_blocked < Duration::from_secs(10));
}

#[sim_test]
async fn test_traffic_sketch_with_slow_blocks() {
    telemetry_subscribers::init_for_testing();
    let sketch_config = FreqThresholdConfig {
        client_threshold: 9_900,
        proxied_client_threshold: 9_900,
        window_size_secs: 4,
        update_interval_secs: 1,
        ..Default::default()
    };
    let policy = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        spam_policy_type: PolicyType::FreqThreshold(sketch_config),
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        channel_capacity: 10,
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy,
        10,     // num_clients
        10_000, // per_client_tps
        Duration::from_secs(20),
        true, // report
    )
    .await;

    let expected_requests = 10_000 * 10 * 20;
    assert!(metrics.num_requests > expected_requests - 1_000);
    assert!(metrics.num_requests < expected_requests + 200);
    // Due to averaging, we will take 4 seconds to start blocking, then
    // will be in blocklist for 1 second (roughly). The cycle is 4s unblocked
    // + 1s blocked = 5s, giving ~20% of requests blocked.
    assert!(metrics.num_blocked as f64 > (expected_requests as f64 / 5.0) * 0.90);
    // 10 clients, blocked at least every 5 seconds, over 20 seconds
    assert!(metrics.num_blocklist_adds >= 40);
    assert!(metrics.abs_time_to_first_block.unwrap() < Duration::from_secs(5));
    assert!(metrics.total_time_blocked > Duration::from_millis(3500));
}

#[sim_test]
async fn test_traffic_sketch_with_sampled_spam() {
    telemetry_subscribers::init_for_testing();
    let sketch_config = FreqThresholdConfig {
        client_threshold: 450,
        proxied_client_threshold: 450,
        window_size_secs: 4,
        update_interval_secs: 1,
        ..Default::default()
    };
    let policy = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        spam_policy_type: PolicyType::FreqThreshold(sketch_config),
        spam_sample_rate: Weight::new(0.5).unwrap(),
        dry_run: false,
        // keeping channel capacity small results in less errors in test metrics,
        // in case of congestion (due to running on slower hardware) requests are dropped
        // and do not influence the rate and do not make the spam rate inconsistent
        channel_capacity: 10,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy,
        1,    // num_clients
        1000, // per_client_tps
        Duration::from_secs(20),
        true, // report
    )
    .await;

    let expected_requests = 1000 * 20;
    assert!(metrics.num_requests > expected_requests - 100);
    assert!(metrics.num_requests < expected_requests + 20);
    // number of blocked requests should be nearly the same
    // as before, as we have half the single client TPS,
    // but the threshold is also halved. However, divide by
    // 5 instead of 4 as a buffer due in case we're unlucky with
    // the sampling
    assert!(metrics.num_blocked > (expected_requests / 5) - 100);
}

#[sim_test]
async fn test_traffic_sketch_allowlist_mode() {
    telemetry_subscribers::init_for_testing();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 1,
        proxy_blocklist_ttl_sec: 1,
        // first two clients allowlisted, rest blocked
        allow_list: Some(vec![String::from("127.0.0.0"), String::from("127.0.0.1")]),
        dry_run: false,
        ..Default::default()
    };
    let metrics = TrafficSim::run(
        policy_config,
        4,      // num_clients
        10_000, // per_client_tps
        Duration::from_secs(10),
        true, // report
    )
    .await;

    let expected_requests = 10_000 * 10 * 4;
    // ~half of all requests blocked
    assert!(metrics.num_blocked >= expected_requests / 2 - 1000);
    assert!(metrics.num_requests > expected_requests - 1_000);
    assert!(metrics.num_requests < expected_requests + 200);
}

/// Execute a signed transaction over the fullnode gRPC API and return its
/// effects.
async fn execute_transaction_grpc(
    client: &iota_grpc_client::Client,
    transaction: Transaction,
) -> Result<TransactionEffects, anyhow::Error> {
    let signed: iota_sdk_types::SignedTransaction = transaction.try_into()?;
    let response = client
        .execute_transaction(
            signed,
            Some(ReadMask::from(TransactionField::EFFECTS_BCS)),
            None,
        )
        .await?;
    let effects = response
        .body()
        .effects
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("effects should be present"))?
        .bcs
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("effects bcs should be present"))?
        .deserialize()?;
    Ok(effects)
}

async fn assert_traffic_control_ok(test_cluster: TestCluster) -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let client = test_cluster.grpc_client();

    let txn_count = 1;
    let mut txns = batch_make_transfer_transactions(&test_cluster.wallet, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );
    let txn = txns.swap_remove(0);
    let tx_digest = *txn.digest();

    // Execute the transaction twice. Execution flows through the fullnode's
    // transaction orchestrator to the validators, so the validator-side policy
    // sees traffic too: the repeat submission hits the validators'
    // already-executed-certificate path, their only spam-counted request.
    for _ in 0..2 {
        let effects = execute_transaction_grpc(&client, txn.clone())
            .await
            .expect("legitimate execution should succeed under traffic control");
        assert_eq!(effects.transaction_digest(), &tx_digest);
    }

    // And a plain fullnode read must succeed as well.
    client
        .get_reference_gas_price()
        .await
        .expect("legitimate request should succeed under traffic control");

    Ok(())
}

/// Test that in dry-run mode, actions that would otherwise
/// lead to request blocking (in this case, a spammy client)
/// are allowed to proceed.
async fn assert_validator_traffic_control_dry_run(
    test_cluster: TestCluster,
    txn_count: usize,
) -> Result<(), anyhow::Error> {
    let client = test_cluster.grpc_client();

    let mut txns = batch_make_transfer_transactions(&test_cluster.wallet, 1).await;
    let txn = txns.swap_remove(0);
    let tx_digest = *txn.digest();

    // Submit the same transaction repeatedly: every submission after the first
    // hits the validators' already-executed-certificate path — their only
    // spam-counted request — driving the per-validator tally past the
    // configured limit. In dry-run mode none of it may be blocked.
    for _ in 0..=txn_count {
        let effects = execute_transaction_grpc(&client, txn.clone())
            .await
            .expect("request should succeed in dry-run mode");
        assert_eq!(effects.transaction_digest(), &tx_digest);
        // Yield so the background `run_tally_loop` task can run between requests.
        tokio::task::yield_now().await;
    }
    Ok(())
}
