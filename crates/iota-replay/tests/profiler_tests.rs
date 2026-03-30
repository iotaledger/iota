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

#[tokio::test(flavor = "multi_thread")]
async fn test_profiler() {
    use std::fs;

    use iota_replay::ReplayToolCommand;
    use tempfile::tempdir;

    let output_dir = tempdir().unwrap();
    let profile_output = output_dir.path().join("profile.json");

    let testnet_url = "https://api.testnet.iota.cafe".to_string();

    // HINT: if the test is flaky, update this tx_digest to a more recent one.
    // Just pick a random transaction from a recent checkpoint, involving shared
    // objects, or simply run "update_profiler_tx.sh" script.
    let tx_digest = "BKUBrYxQsatsPFj9z9fNFaChYkLX6rCFoSpPxnVh7ESr".to_string();

    let cmd = ReplayToolCommand::ProfileTransaction {
        tx_digest,
        executor_version: None,
        protocol_version: None,
        profile_output: Some(profile_output),
        config_objects: None,
    };

    let command_result =
        iota_replay::execute_replay_command(Some(testnet_url), false, false, None, None, cmd).await;

    command_result.expect("Failed to execute replay command. HINT: if the test is flaky, update the tx_digest to a more recent one by running \"update_profiler_tx.sh\".");

    // check that the profile was written
    let mut found = false;
    for entry in fs::read_dir(output_dir.keep()).unwrap().flatten() {
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
