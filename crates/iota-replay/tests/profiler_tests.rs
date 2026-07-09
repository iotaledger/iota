// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This test verifies that the `tracing` feature on `move-vm-profiler` is
/// enabled (via dev-dependencies). If it fails, check that
/// `iota-replay = { path = ".", features = ["tracing"] }` is present
/// in the dev-dependencies of this crate.
#[test]
fn test_macro_shows_feature_enabled() {
    move_vm_profiler::tracing_feature_disabled! {
        panic!("gas profile feature graph became disconnected");
    }
}

/// Spawns a local network, executes a Move call, then profiles it via the
/// replay tool and checks that a profile was written.
///
/// The transaction is produced on a local cluster rather than fetched from a
/// public network: a live network prunes historical data, and both the pinned
/// transaction and the epoch-change events the replay engine needs to
/// reconstruct the transaction's environment eventually become unfetchable.
#[iota_macros::sim_test]
async fn test_profiler() {
    use std::fs;

    use iota_replay::ReplayToolCommand;
    use iota_test_transaction_builder::publish_basics_package;
    use test_cluster::TestClusterBuilder;

    let tmp_dir = iota_common::tempdir();
    let profile_output = tmp_dir.path().join("profile.json");

    let mut test_cluster = TestClusterBuilder::new().build().await;
    let rpc_url = test_cluster.rpc_url().to_string();

    // The replay engine does not support transactions from epoch 0.
    test_cluster.force_new_epoch().await;

    // Publish a package and call one of its functions so the profiled
    // transaction actually executes Move bytecode.
    let package = publish_basics_package(test_cluster.wallet()).await;
    let tx_data = test_cluster
        .test_transaction_builder()
        .await
        .call_counter_create(package.object_id)
        .build();
    let tx_digest = test_cluster
        .sign_and_execute_transaction(&tx_data)
        .await
        .digest
        .to_string();

    let cmd = ReplayToolCommand::ProfileTransaction {
        tx_digest,
        executor_version: None,
        protocol_version: None,
        profile_output: Some(profile_output),
        config_objects: None,
    };

    let command_result =
        iota_replay::execute_replay_command(Some(rpc_url), false, false, None, None, cmd).await;

    command_result.expect("Failed to execute replay command.");

    // check that the profile was written
    let mut found = false;
    for entry in fs::read_dir(tmp_dir.path()).unwrap().flatten() {
        if entry
            .file_name()
            .into_string()
            .unwrap()
            .starts_with("profile")
        {
            found = true;
        }
    }
    assert!(found);
}
