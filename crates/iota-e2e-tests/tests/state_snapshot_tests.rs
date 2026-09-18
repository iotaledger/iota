// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Publishing a formal state snapshot across a real epoch boundary.
//!
//! The unit tests drive the writer directly. Only a running node exercises the
//! path that matters: reconfiguration hands the epoch over while execution is
//! paused, the scan runs against the store the node keeps executing into, and
//! what lands in the remote store has to match the commitment the epoch ended
//! with.

use std::{path::PathBuf, time::Duration};

use iota_config::{node::StateSnapshotConfig, object_storage_config::ObjectStoreConfig};
use iota_macros::sim_test;
use iota_storage::object_store::util::SUCCESS_MARKER;
use test_cluster::TestClusterBuilder;

/// Waits for `path` to appear, so the test fails as a timeout rather than
/// hanging when the snapshot never lands.
async fn wait_for(path: PathBuf) {
    let deadline = Duration::from_secs(120);
    tokio::time::timeout(deadline, async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{} did not appear within {deadline:?}", path.display()));
}

/// A fullnode publishes the snapshot of the epoch it has just left, and the
/// `_SUCCESS` marker is only written once every object and reference file is
/// uploaded and the digest of the scan matched the epoch's commitment.
#[sim_test]
async fn a_fullnode_publishes_the_snapshot_of_an_epoch_it_leaves() {
    let remote_dir = tempfile::tempdir().expect("a temporary remote store");
    let remote_path = remote_dir.path().to_path_buf();

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(20_000)
        .with_fullnode_state_snapshot_config(StateSnapshotConfig {
            object_store_config: Some(ObjectStoreConfig {
                object_store: Some(iota_config::object_storage_config::ObjectStoreType::File),
                directory: Some(remote_path.clone()),
                ..Default::default()
            }),
            concurrency: 1,
        })
        .build()
        .await;

    // Epoch 0 is only published once the node has left it.
    assert!(!remote_path.join("epoch_0").join(SUCCESS_MARKER).exists());

    test_cluster.force_new_epoch().await;

    wait_for(remote_path.join("epoch_0").join(SUCCESS_MARKER)).await;

    // A second boundary publishes its own epoch, which is what shows the
    // writer is released and takes a fresh snapshot rather than being a
    // one-shot.
    test_cluster.force_new_epoch().await;
    wait_for(remote_path.join("epoch_1").join(SUCCESS_MARKER)).await;
}
