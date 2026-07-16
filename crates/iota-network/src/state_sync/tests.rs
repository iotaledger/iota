// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use anemo::{PeerId, Request};
use anyhow::anyhow;
use bytes::Bytes;
use iota_config::{node::CheckpointArchiveConfig, p2p::StateSyncConfig};
use iota_data_ingestion_core::history::{
    CHECKPOINT_FILE_MAGIC,
    manifest::{Manifest, create_file_metadata_from_bytes, finalize_manifest},
};
use iota_storage::{
    FileCompression, StorageFormat,
    blob::{Blob, BlobEncoding},
};
use iota_swarm_config::test_utils::{
    CommitteeFixture, MakeCheckpointResults, empty_contents, random_contents,
};
use iota_types::{
    committee::{Committee, EpochId},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{VerifiedCheckpoint, VerifiedCheckpointContents},
    storage::{ReadStore, SharedInMemoryStore, WriteStore},
};
use tokio::time::{Instant, timeout};

use crate::{
    state_sync::{
        Builder, GetCheckpointSummaryRequest, PeerStateSyncInfo, StateSync, StateSyncMessage,
        UnstartedStateSync,
    },
    utils::build_network,
};

fn make_committee_and_checkpoints<F: Fn() -> VerifiedCheckpointContents>(
    epoch: EpochId,
    committee_size: usize,
    number_of_checkpoints: usize,
    previous_checkpoint: Option<VerifiedCheckpoint>,
    content_generator: F,
) -> (CommitteeFixture, MakeCheckpointResults) {
    let committee = CommitteeFixture::generate(rand::rngs::OsRng, epoch, committee_size);
    let results = committee.make_checkpoints(
        number_of_checkpoints,
        previous_checkpoint,
        content_generator,
    );
    (committee, results)
}

fn store_with_genesis_state(
    genesis_checkpoint: VerifiedCheckpoint,
    genesis_contents: VerifiedCheckpointContents,
    committee: Committee,
) -> SharedInMemoryStore {
    let store = SharedInMemoryStore::default();
    store
        .inner_mut()
        .insert_genesis_state(genesis_checkpoint, genesis_contents, committee);
    store
}

#[tokio::test]
// Test that the server stores the pushed checkpoint summary and triggers the
// sync job.
async fn server_push_checkpoint() {
    let (committee, (ordered_checkpoints, _, _, _)) =
        make_committee_and_checkpoints(0, 4, 2, None, empty_contents);
    let store = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );

    let (
        UnstartedStateSync {
            handle: _handle,
            mut mailbox,
            peer_heights,
            ..
        },
        server,
    ) = Builder::new().store(store).build_internal();
    let peer_id = PeerId([9; 32]); // fake PeerId

    peer_heights.write().unwrap().peers.insert(
        peer_id,
        PeerStateSyncInfo {
            genesis_checkpoint_digest: *ordered_checkpoints[0].digest(),
            on_same_chain_as_us: true,
            height: 0,
            lowest: 0,
        },
    );

    let checkpoint = ordered_checkpoints[1].inner().to_owned();
    let request = Request::new(checkpoint.clone()).with_extension(peer_id);
    server.push_checkpoint_summary(request).await.unwrap();

    assert_eq!(
        peer_heights.read().unwrap().peers.get(&peer_id),
        Some(&PeerStateSyncInfo {
            genesis_checkpoint_digest: *ordered_checkpoints[0].digest(),
            on_same_chain_as_us: true,
            height: 1,
            lowest: 0,
        })
    );
    assert_eq!(
        peer_heights
            .read()
            .unwrap()
            .unprocessed_checkpoints
            .get(checkpoint.digest())
            .unwrap()
            .data(),
        checkpoint.data(),
    );
    assert_eq!(
        peer_heights
            .read()
            .unwrap()
            .highest_known_checkpoint()
            .unwrap()
            .data(),
        checkpoint.data(),
    );
    assert!(matches!(
        mailbox.try_recv().unwrap(),
        StateSyncMessage::StartSyncJob
    ));
}

#[tokio::test]
async fn server_get_checkpoint() {
    let (committee, (ordered_checkpoints, _, _, _)) =
        make_committee_and_checkpoints(0, 4, 3, None, empty_contents);

    let store = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let (builder, server) = Builder::new().store(store).build_internal();

    // Requests for the Latest checkpoint should return the genesis checkpoint
    let response = server
        .get_checkpoint_summary(Request::new(GetCheckpointSummaryRequest::Latest))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        response.unwrap().data(),
        ordered_checkpoints.first().unwrap().data(),
    );

    // Requests for checkpoints that aren't in the server's store
    let requests = [
        GetCheckpointSummaryRequest::BySequenceNumber(9),
        GetCheckpointSummaryRequest::ByDigest(CheckpointDigest::new([10; 32])),
    ];
    for request in requests {
        let response = server
            .get_checkpoint_summary(Request::new(request))
            .await
            .unwrap()
            .into_inner();
        assert!(response.is_none());
    }

    // Populate the node's store with some checkpoints
    for checkpoint in ordered_checkpoints.clone() {
        builder.store.inner_mut().insert_checkpoint(&checkpoint)
    }
    let latest = ordered_checkpoints.last().unwrap().clone();
    builder
        .store
        .inner_mut()
        .update_highest_synced_checkpoint(&latest);

    let request = Request::new(GetCheckpointSummaryRequest::Latest);
    let response = server
        .get_checkpoint_summary(request)
        .await
        .unwrap()
        .into_inner()
        .unwrap();
    assert_eq!(response.data(), latest.data());

    for checkpoint in ordered_checkpoints {
        let request = Request::new(GetCheckpointSummaryRequest::ByDigest(*checkpoint.digest()));
        let response = server
            .get_checkpoint_summary(request)
            .await
            .unwrap()
            .into_inner()
            .unwrap();
        assert_eq!(response.data(), checkpoint.data());

        let request = Request::new(GetCheckpointSummaryRequest::BySequenceNumber(
            checkpoint.sequence_number(),
        ));
        let response = server
            .get_checkpoint_summary(request)
            .await
            .unwrap()
            .into_inner()
            .unwrap();
        assert_eq!(response.data(), checkpoint.data());
    }
}

#[tokio::test]
async fn isolated_sync_job() {
    // Build mock data
    let (committee, (ordered_checkpoints, _, sequence_number_to_digest, checkpoints)) =
        make_committee_and_checkpoints(0, 4, 100, None, empty_contents);

    // Build and connect two nodes — genesis is initialized in each store before
    // it is passed to the builder.
    let store_1 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let store_2 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let (builder, server) = Builder::new().store(store_1).build();
    let network_1 = build_network(|router| router.add_rpc_service(server));
    let (mut event_loop_1, _handle_1) = builder.build(network_1.clone());
    let (builder, server) = Builder::new().store(store_2).build();
    let network_2 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_2, _handle_2) = builder.build(network_2.clone());
    network_1.connect(network_2.local_addr()).await.unwrap();

    // Node 2 will have all the data
    {
        let mut store = event_loop_2.store.inner_mut();
        for checkpoint in ordered_checkpoints.clone() {
            store.insert_checkpoint(&checkpoint);
        }
    }

    // Node 1 will know that Node 2 has the data
    event_loop_1.peer_heights.write().unwrap().peers.insert(
        network_2.peer_id(),
        PeerStateSyncInfo {
            genesis_checkpoint_digest: *ordered_checkpoints[0].digest(),
            on_same_chain_as_us: true,
            height: ordered_checkpoints.last().unwrap().sequence_number(),
            lowest: 0,
        },
    );
    event_loop_1
        .peer_heights
        .write()
        .unwrap()
        .insert_checkpoint(ordered_checkpoints.last().cloned().unwrap().into_inner());

    // Sync the data
    event_loop_1.maybe_start_checkpoint_summary_sync_task();
    event_loop_1.tasks.join_next().await.unwrap().unwrap();
    assert_eq!(
        ordered_checkpoints.last().map(|x| x.data()),
        Some(
            event_loop_1
                .store
                .try_get_highest_verified_checkpoint()
                .unwrap()
                .data()
        )
    );

    {
        let store = event_loop_1.store.inner();
        let expected = checkpoints
            .iter()
            .map(|(key, value)| (key, value.data()))
            .collect::<HashMap<_, _>>();
        let actual = store
            .checkpoints()
            .iter()
            .map(|(key, value)| (key, value.data()))
            .collect::<HashMap<_, _>>();
        assert_eq!(actual, expected);
        assert_eq!(
            store.checkpoint_sequence_number_to_digest(),
            &sequence_number_to_digest
        );
    }
}

#[tokio::test]
async fn test_state_sync_using_checkpoint_archive() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();
    let committee = CommitteeFixture::generate(rand::rngs::OsRng, 0, 4);
    // build mock data
    let (ordered_checkpoints, ordered_contents, sequence_number_to_digest, checkpoints) =
        committee.make_empty_checkpoints(100, None);
    let temp_dir = iota_common::tempdir();
    // We will delete all checkpoints older than this checkpoint on Node 2
    let oldest_checkpoint_to_keep: u64 = 10;

    // Create archive files for the first `oldest_checkpoint_to_keep` checkpoints
    {
        let mut chk_buf: Vec<u8> = Vec::new();
        chk_buf.extend_from_slice(&CHECKPOINT_FILE_MAGIC.to_be_bytes());
        chk_buf.push(StorageFormat::Blob as u8);
        chk_buf.push(FileCompression::None as u8);

        for i in 0..(oldest_checkpoint_to_keep as usize) {
            let checkpoint_data = CheckpointData {
                checkpoint_summary: ordered_checkpoints[i].clone().into_inner(),
                checkpoint_contents: ordered_contents[i].clone().into_checkpoint_contents(),
                transactions: vec![],
            };
            Blob::encode(&checkpoint_data, BlobEncoding::Bcs)?.write(&mut chk_buf)?;
        }

        let chk_bytes = Bytes::from(chk_buf.clone());
        let file_metadata =
            create_file_metadata_from_bytes(chk_bytes, 0..oldest_checkpoint_to_keep)?;
        std::fs::write(temp_dir.join("0.chk"), &chk_buf)?;

        let mut manifest = Manifest::new(0);
        manifest.update(oldest_checkpoint_to_keep, file_metadata);
        let manifest_bytes = finalize_manifest(manifest)?;
        std::fs::write(temp_dir.join("MANIFEST"), &manifest_bytes[..])?;
    }
    let checkpoint_archive_config = CheckpointArchiveConfig {
        download_concurrency: 1,
        url: format!("file://{}", temp_dir.display()),
    };
    // Build and connect two nodes where Node 1 will be given access to an archive
    // store Node 2 will prune older checkpoints, so Node 1 is forced to
    // backfill from the archive
    let store_1 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let (builder, state_sync_router) = Builder::new()
        .store(store_1)
        .config(StateSyncConfig {
            // Shorten the retry delay so pruned-checkpoint tasks quickly discover
            // that the archive has already advanced highest_synced past them.
            wait_interval_when_no_peer_to_sync_content_ms: Some(10),
            ..StateSyncConfig::randomized_for_testing()
        })
        .checkpoint_archive_config(Some(checkpoint_archive_config))
        .build();
    let network_1 = build_network(|router| router.add_rpc_service(state_sync_router));
    let (event_loop_1, _handle_1) = builder.build(network_1.clone());
    let store_2 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let (builder, state_sync_router) = Builder::new()
        .store(store_2)
        .config(StateSyncConfig::randomized_for_testing())
        .build();
    let network_2 = build_network(|router| router.add_rpc_service(state_sync_router));
    let (event_loop_2, _handle_2) = builder.build(network_2.clone());
    network_1.connect(network_2.local_addr()).await.unwrap();

    // Node 2 will have all the data at first
    {
        let mut store = event_loop_2.store.inner_mut();
        for checkpoint in ordered_checkpoints.clone() {
            store.insert_checkpoint(&checkpoint);
            store.insert_checkpoint_contents(&checkpoint, empty_contents());
            store.update_highest_synced_checkpoint(&checkpoint);
        }
    }
    // Prune first 10 checkpoint contents from Node 2
    {
        let mut store = event_loop_2.store.inner_mut();
        for checkpoint in &ordered_checkpoints[0..(oldest_checkpoint_to_keep as usize)] {
            store.delete_checkpoint_content_test_only(checkpoint.sequence_number)?;
        }
        // Now Node 2 has deleted checkpoint contents from range [0, 10) on local store
        assert_eq!(
            store.get_lowest_available_checkpoint(),
            oldest_checkpoint_to_keep
        );
        assert_eq!(
            store
                .get_highest_synced_checkpoint()
                .unwrap()
                .sequence_number,
            ordered_checkpoints.last().unwrap().sequence_number
        );
        assert_eq!(
            store
                .get_highest_verified_checkpoint()
                .unwrap()
                .sequence_number,
            ordered_checkpoints.last().unwrap().sequence_number
        );
    }

    // Node 1 will know that Node 2 has the data starting checkpoint 10
    event_loop_1.peer_heights.write().unwrap().peers.insert(
        network_2.peer_id(),
        PeerStateSyncInfo {
            genesis_checkpoint_digest: *ordered_checkpoints[0].digest(),
            on_same_chain_as_us: true,
            height: ordered_checkpoints.last().unwrap().sequence_number(),
            lowest: oldest_checkpoint_to_keep,
        },
    );

    // Get handle to node 1 store
    let store_1 = event_loop_1.store.clone();

    // Sync the data
    // Start both event loops
    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());

    let total_time = Instant::now();
    loop {
        {
            let store = store_1.inner();
            if let Some(highest_synced_checkpoint) = store.get_highest_synced_checkpoint() {
                if highest_synced_checkpoint.sequence_number
                    == ordered_checkpoints.last().unwrap().sequence_number
                {
                    // Node 1 is fully synced to the latest checkpoint on Node 2
                    let expected = checkpoints
                        .iter()
                        .map(|(key, value)| (key, value.data()))
                        .collect::<HashMap<_, _>>();
                    let actual = store
                        .checkpoints()
                        .iter()
                        .map(|(key, value)| (key, value.data()))
                        .collect::<HashMap<_, _>>();
                    assert_eq!(actual, expected);
                    assert_eq!(
                        store.checkpoint_sequence_number_to_digest(),
                        &sequence_number_to_digest
                    );
                    break;
                }
            }
        }
        if total_time.elapsed() > Duration::from_secs(20) {
            return Err(anyhow!("Test timed out"));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

#[tokio::test]
async fn sync_with_checkpoints_being_inserted() {
    telemetry_subscribers::init_for_testing();
    // Build mock data
    let (committee, (ordered_checkpoints, _contents, sequence_number_to_digest, checkpoints)) =
        make_committee_and_checkpoints(0, 4, 4, None, empty_contents);

    // Build two nodes — genesis must be in the store before passing to the
    // builder so the cached genesis checkpoint is available.
    let store_1 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let store_2 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        empty_contents(),
        committee.committee().to_owned(),
    );
    let (builder, server) = Builder::new().store(store_1).build();
    let network_1 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_1, handle_1) = builder.build(network_1.clone());
    let (builder, server) = Builder::new().store(store_2).build();
    let network_2 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_2, handle_2) = builder.build(network_2.clone());

    // Get handles to each node's stores
    let store_1 = event_loop_1.store.clone();
    let store_2 = event_loop_2.store.clone();
    // Make sure that node_1 knows about node_2
    event_loop_1.peer_heights.write().unwrap().peers.insert(
        network_2.peer_id(),
        PeerStateSyncInfo {
            genesis_checkpoint_digest: *ordered_checkpoints[0].digest(),
            on_same_chain_as_us: true,
            height: 0,
            lowest: 0,
        },
    );
    // Start both event loops
    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());

    network_1.connect(network_2.local_addr()).await.unwrap();

    let mut subscriber_1 = handle_1.subscribe_to_synced_checkpoints();
    let mut subscriber_2 = handle_2.subscribe_to_synced_checkpoints();

    // Inject one checkpoint and verify that it was shared with the other node
    let mut checkpoint_iter = ordered_checkpoints.clone().into_iter().skip(1);
    let checkpoint = checkpoint_iter.next().unwrap();
    store_1
        .try_insert_checkpoint_contents(&checkpoint, empty_contents())
        .unwrap();
    store_1.insert_certified_checkpoint(&checkpoint);
    handle_1.send_checkpoint(checkpoint).await;

    timeout(Duration::from_secs(1), async {
        assert_eq!(
            subscriber_1.recv().await.unwrap().data(),
            ordered_checkpoints[1].data(),
        );
        assert_eq!(
            subscriber_2.recv().await.unwrap().data(),
            ordered_checkpoints[1].data()
        );
    })
    .await
    .unwrap();

    // Inject all the checkpoints
    for checkpoint in checkpoint_iter {
        store_1.insert_certified_checkpoint(&checkpoint);
        handle_1.send_checkpoint(checkpoint).await;
    }

    timeout(Duration::from_secs(1), async {
        for checkpoint in &ordered_checkpoints[2..] {
            assert_eq!(subscriber_1.recv().await.unwrap().data(), checkpoint.data());
            assert_eq!(subscriber_2.recv().await.unwrap().data(), checkpoint.data());
        }
    })
    .await
    .unwrap();

    let store_1 = store_1.inner();
    let store_2 = store_2.inner();
    assert_eq!(
        ordered_checkpoints.last().map(|x| x.digest()),
        store_1
            .get_highest_verified_checkpoint()
            .as_ref()
            .map(|x| x.digest())
    );
    assert_eq!(
        ordered_checkpoints.last().map(|x| x.digest()),
        store_2
            .get_highest_verified_checkpoint()
            .as_ref()
            .map(|x| x.digest())
    );

    let expected = checkpoints
        .iter()
        .map(|(key, value)| (key, value.data()))
        .collect::<HashMap<_, _>>();
    let actual_1 = store_1
        .checkpoints()
        .iter()
        .map(|(key, value)| (key, value.data()))
        .collect::<HashMap<_, _>>();
    assert_eq!(actual_1, expected);
    assert_eq!(
        store_1.checkpoint_sequence_number_to_digest(),
        &sequence_number_to_digest
    );

    let actual_2 = store_2
        .checkpoints()
        .iter()
        .map(|(key, value)| (key, value.data()))
        .collect::<HashMap<_, _>>();
    assert_eq!(actual_2, expected);
    assert_eq!(
        store_2.checkpoint_sequence_number_to_digest(),
        &sequence_number_to_digest
    );
}

#[tokio::test]
async fn sync_with_checkpoints_watermark() {
    telemetry_subscribers::init_for_testing();
    // Build mock data
    let (committee, (ordered_checkpoints, contents, _, _)) =
        make_committee_and_checkpoints(0, 4, 4, None, random_contents);
    let last_checkpoint_seq = ordered_checkpoints
        .last()
        .cloned()
        .unwrap()
        .sequence_number();
    // Build and connect two nodes — genesis is initialized in each store before
    // it is passed to the builder.
    let genesis_checkpoint_content = contents.first().cloned().unwrap();
    let store_1 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        genesis_checkpoint_content.clone(),
        committee.committee().to_owned(),
    );
    let store_2 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        genesis_checkpoint_content.clone(),
        committee.committee().to_owned(),
    );
    let (builder, server) = Builder::new().store(store_1).build();
    let network_1 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_1, handle_1) = builder.build(network_1.clone());
    let (builder, server) = Builder::new().store(store_2).build();
    let network_2 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_2, handle_2) = builder.build(network_2.clone());

    // Get handles to each node's stores
    let store_1 = event_loop_1.store.clone();
    let store_2 = event_loop_2.store.clone();
    let peer_id_1 = network_1.peer_id();

    let peer_heights_1 = event_loop_1.peer_heights.clone();
    let peer_heights_2 = event_loop_2.peer_heights.clone();
    peer_heights_1
        .write()
        .unwrap()
        .set_wait_interval_when_no_peer_to_sync_content(Duration::from_secs(1));
    peer_heights_2
        .write()
        .unwrap()
        .set_wait_interval_when_no_peer_to_sync_content(Duration::from_secs(1));

    // Start both event loops
    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());

    let mut subscriber_1 = handle_1.subscribe_to_synced_checkpoints();
    let mut subscriber_2 = handle_2.subscribe_to_synced_checkpoints();

    network_1.connect(network_2.local_addr()).await.unwrap();

    // Inject one checkpoint and verify that it was shared with the other node
    let mut checkpoint_iter = ordered_checkpoints.clone().into_iter().skip(1);
    let mut contents_iter = contents.clone().into_iter().skip(1);
    let checkpoint_1 = checkpoint_iter.next().unwrap();
    let contents_1 = contents_iter.next().unwrap();
    let checkpoint_seq = checkpoint_1.sequence_number();
    store_1
        .try_insert_checkpoint_contents(&checkpoint_1, contents_1.clone())
        .unwrap();
    store_1.insert_certified_checkpoint(&checkpoint_1);
    handle_1.send_checkpoint(checkpoint_1.clone()).await;

    timeout(Duration::from_secs(3), async {
        assert_eq!(
            subscriber_1.recv().await.unwrap().data(),
            ordered_checkpoints[1].data(),
        );
        assert_eq!(
            subscriber_2.recv().await.unwrap().data(),
            ordered_checkpoints[1].data()
        );
    })
    .await
    .unwrap();

    assert_eq!(
        store_1
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        checkpoint_seq
    );
    assert_eq!(
        store_2
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        checkpoint_seq
    );
    assert_eq!(
        store_1
            .try_get_highest_verified_checkpoint()
            .unwrap()
            .sequence_number(),
        1
    );
    assert_eq!(
        store_2
            .try_get_highest_verified_checkpoint()
            .unwrap()
            .sequence_number(),
        1
    );

    // So far so good.
    // Now we increase Peer 1's low watermark to a high number.
    let a_very_high_checkpoint_seq = 1000;
    store_1
        .inner_mut()
        .set_lowest_available_checkpoint(a_very_high_checkpoint_seq);

    assert!(peer_heights_2.write().unwrap().update_peer_info(
        peer_id_1,
        checkpoint_1.clone().into(),
        Some(a_very_high_checkpoint_seq),
    ));

    // Inject all the checkpoints to Peer 1
    for (checkpoint, contents) in checkpoint_iter.zip(contents_iter) {
        store_1
            .try_insert_checkpoint_contents(&checkpoint, contents)
            .unwrap();
        store_1.insert_certified_checkpoint(&checkpoint);
        handle_1.send_checkpoint(checkpoint).await;
    }

    // Peer 1 has all the checkpoint contents, but not Peer 2
    timeout(Duration::from_secs(1), async {
        for (checkpoint, contents) in ordered_checkpoints[2..]
            .iter()
            .zip(contents.clone().into_iter().skip(2))
        {
            assert_eq!(subscriber_1.recv().await.unwrap().data(), checkpoint.data());
            let content_digest = contents.into_checkpoint_contents_digest();
            store_1
                .get_full_checkpoint_contents(&content_digest)
                .unwrap();
            assert_eq!(store_2.get_full_checkpoint_contents(&content_digest), None);
        }
    })
    .await
    .unwrap();
    subscriber_2.try_recv().unwrap_err();

    assert_eq!(
        store_1
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        ordered_checkpoints.last().unwrap().sequence_number()
    );
    assert_eq!(
        store_2
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        ordered_checkpoints[1].sequence_number()
    );

    assert_eq!(
        store_1
            .try_get_highest_verified_checkpoint()
            .unwrap()
            .sequence_number(),
        last_checkpoint_seq
    );

    // Add Peer 3 — genesis is initialized in the store before it is passed to
    // the builder so the store is ready for handshake.
    let store_3 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        genesis_checkpoint_content.clone(),
        committee.committee().to_owned(),
    );
    let (builder, server) = Builder::new().store(store_3).build();
    let network_3 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_3, handle_3) = builder.build(network_3.clone());

    let mut subscriber_3 = handle_3.subscribe_to_synced_checkpoints();
    let store_3 = event_loop_3.store.clone();
    let peer_heights_3 = event_loop_3.peer_heights.clone();
    peer_heights_3
        .write()
        .unwrap()
        .set_wait_interval_when_no_peer_to_sync_content(Duration::from_secs(1));
    tokio::spawn(event_loop_3.start());
    network_3.connect(network_1.local_addr()).await.unwrap();
    network_3.connect(network_2.local_addr()).await.unwrap();

    // Peer 3 is able to sync checkpoint 1 with the help from Peer 2
    timeout(Duration::from_secs(3), async {
        assert_eq!(
            subscriber_3.recv().await.unwrap().data(),
            ordered_checkpoints[1].data()
        );
        let content_digest = contents[1].clone().into_checkpoint_contents_digest();
        store_3
            .get_full_checkpoint_contents(&content_digest)
            .unwrap();
    })
    .await
    .unwrap();
    subscriber_3.try_recv().unwrap_err();
    subscriber_2.try_recv().unwrap_err();

    assert_eq!(
        store_2
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        ordered_checkpoints[1].sequence_number(),
    );
    assert_eq!(
        store_3
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        ordered_checkpoints[1].sequence_number(),
    );

    // Now set Peer 1's low watermark back to 0
    store_1.inner_mut().set_lowest_available_checkpoint(0);

    // Peer 2 and Peer 3 will know about this change by
    // `get_checkpoint_availability` Soon we expect them to have all
    // checkpoints's content.
    timeout(Duration::from_secs(6), async {
        for (checkpoint, contents) in ordered_checkpoints[2..]
            .iter()
            .zip(contents.clone().into_iter().skip(2))
        {
            assert_eq!(subscriber_2.recv().await.unwrap().data(), checkpoint.data());
            assert_eq!(subscriber_3.recv().await.unwrap().data(), checkpoint.data());
            let content_digest = contents.into_checkpoint_contents_digest();
            store_2
                .get_full_checkpoint_contents(&content_digest)
                .unwrap();
            store_3
                .get_full_checkpoint_contents(&content_digest)
                .unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(
        store_2
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        last_checkpoint_seq
    );
    assert_eq!(
        store_3
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        last_checkpoint_seq
    );
    assert_eq!(
        store_2
            .try_get_highest_verified_checkpoint()
            .unwrap()
            .sequence_number(),
        last_checkpoint_seq
    );
    assert_eq!(
        store_3
            .try_get_highest_verified_checkpoint()
            .unwrap()
            .sequence_number(),
        last_checkpoint_seq
    );

    // Now set Peer 1 and 2's low watermark to a very high number
    store_1
        .inner_mut()
        .set_lowest_available_checkpoint(a_very_high_checkpoint_seq);

    store_2
        .inner_mut()
        .set_lowest_available_checkpoint(a_very_high_checkpoint_seq);

    // Start Peer 4 — genesis is initialized in the store before it is passed
    // to the builder.
    let store_4 = store_with_genesis_state(
        ordered_checkpoints.first().cloned().unwrap(),
        genesis_checkpoint_content,
        committee.committee().to_owned(),
    );
    let (builder, server) = Builder::new().store(store_4).build();
    let network_4 = build_network(|router| router.add_rpc_service(server));
    let (event_loop_4, handle_4) = builder.build(network_4.clone());

    let mut subscriber_4 = handle_4.subscribe_to_synced_checkpoints();
    let store_4 = event_loop_4.store.clone();
    let peer_heights_4 = event_loop_4.peer_heights.clone();
    peer_heights_4
        .write()
        .unwrap()
        .set_wait_interval_when_no_peer_to_sync_content(Duration::from_secs(1));
    tokio::spawn(event_loop_4.start());
    // Need to connect 4 to 1, 2, 3 manually, as it does not have discovery enabled
    network_4.connect(network_1.local_addr()).await.unwrap();
    network_4.connect(network_2.local_addr()).await.unwrap();
    network_4.connect(network_3.local_addr()).await.unwrap();

    // Peer 4 syncs everything with Peer 3
    timeout(Duration::from_secs(3), async {
        for (checkpoint, contents) in ordered_checkpoints[1..]
            .iter()
            .zip(contents.clone().into_iter().skip(1))
        {
            assert_eq!(subscriber_4.recv().await.unwrap().data(), checkpoint.data());
            let content_digest = contents.into_checkpoint_contents_digest();
            store_4
                .get_full_checkpoint_contents(&content_digest)
                .unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(
        store_4
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        last_checkpoint_seq
    );
}

/// Regression test for https://github.com/iotaledger/iota/issues/11496.
///
/// When checkpoint content is unavailable
/// (`ContentSyncError::PrunedOnAllPeers`) during content sync attempt the
/// failing checkpoint must be retried first before trying to sync later
/// checkpoints.
#[tokio::test]
async fn sync_with_checkpoints_gap() -> anyhow::Result<()> {
    telemetry_subscribers::init_for_testing();

    // 6 checkpoints: genesis (seq 0) + sequences 1–5.
    // Checkpoint 1 will be simulated as "pruned" on the peer; 2–5 are available.
    let (committee, (ordered_checkpoints, contents, _, _)) =
        make_committee_and_checkpoints(0, 4, 6, None, random_contents);

    let genesis_content = contents.first().cloned().unwrap();
    let genesis_checkpoint = ordered_checkpoints.first().cloned().unwrap();

    let store_1 = store_with_genesis_state(
        genesis_checkpoint.clone(),
        genesis_content.clone(),
        committee.committee().to_owned(),
    );
    let store_2 = store_with_genesis_state(
        genesis_checkpoint.clone(),
        genesis_content.clone(),
        committee.committee().to_owned(),
    );

    let (builder_1, server_1) = Builder::new().store(store_1.clone()).build();
    let network_1 = build_network(|router| router.add_rpc_service(server_1));
    let (event_loop_1, _handle_1) = builder_1.build(network_1.clone());

    // Shorten the retry back-off so checkpoint 1's failure loop cycles quickly
    // and any watermark regression shows up within the 2 s assertion window.
    let config_2 = StateSyncConfig {
        wait_interval_when_no_peer_to_sync_content_ms: Some(50),
        ..Default::default()
    };
    let (builder_2, server_2) = Builder::new()
        .config(config_2)
        .store(store_2.clone())
        .build();
    let network_2 = build_network(|router| router.add_rpc_service(server_2));
    let (event_loop_2, _handle_2) = builder_2.build(network_2.clone());

    // Node 1: insert all summaries and contents (sequences 0–5), synced
    // watermark at 5.
    {
        let mut store_1 = store_1.inner_mut();
        for (checkpoint, content) in ordered_checkpoints.iter().zip(contents.iter()) {
            store_1.insert_checkpoint(checkpoint);
            store_1.insert_checkpoint_contents(checkpoint, content.clone());
            store_1.update_highest_synced_checkpoint(checkpoint);
        }
    }

    // Simulate checkpoint 1 being pruned: set node 1's lowest-available
    // watermark to 2.  Node 2 will learn this via the handshake and see
    // `is_pruned = true` for checkpoint 1 (seq 1 < lowest 2) while
    // checkpoints 2–5 remain fetchable (seq >= 2).
    store_1.inner_mut().set_lowest_available_checkpoint(2);

    tokio::spawn(event_loop_1.start());
    tokio::spawn(event_loop_2.start());
    network_2.connect(network_1.local_addr()).await.unwrap();

    let genesis_seq = genesis_checkpoint.sequence_number();
    let last_seq = ordered_checkpoints.last().unwrap().sequence_number();

    // Wait for node 2 to verify all summaries (sequences 0–5).
    timeout(Duration::from_secs(10), async {
        loop {
            if store_2
                .try_get_highest_verified_checkpoint()
                .unwrap()
                .sequence_number()
                == last_seq
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("node 2 failed to sync all checkpoint summaries");

    // Give the content-sync loop 2 s to run.  With a 50 ms retry interval
    // ~40 retry cycles fire for checkpoint 1.  If push_back were used
    // (pre-fix), checkpoints 2–5 would advance the watermark to 5 within
    // this window.
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert!(
        store_2
            .get_full_checkpoint_contents_by_sequence_number(2)
            .is_some(),
        "content loop did not even fetch seq 2 within the window — test is not exercising the gap",
    );

    // REGRESSION CHECK: the synced watermark must not have advanced past
    // genesis (sequence 0) while checkpoint 1's contents are unavailable.
    assert_eq!(
        store_2
            .try_get_highest_synced_checkpoint()
            .unwrap()
            .sequence_number(),
        genesis_seq,
        "synced watermark advanced past genesis even though checkpoint 1 \
         contents are unavailable — regression from PR #11485 (push_back bug)"
    );

    // Restore availability: lower the watermark to 0.  Node 2 will refresh
    // node 1's watermark on the next periodic tick (≤5 s) and unblock
    // checkpoint 1's content sync.
    store_1.inner_mut().set_lowest_available_checkpoint(0);

    // Allow up to 12 s for the tick, the watermark refresh, and the full sync.
    timeout(Duration::from_secs(12), async {
        loop {
            if store_2
                .try_get_highest_synced_checkpoint()
                .unwrap()
                .sequence_number()
                == last_seq
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("node 2 failed to fully sync after checkpoint 1 became available");

    // Verify that all checkpoint contents are present in node 2's store.
    for (i, checkpoint) in ordered_checkpoints.iter().enumerate() {
        assert!(
            store_2
                .get_full_checkpoint_contents_by_sequence_number(checkpoint.sequence_number())
                .is_some(),
            "checkpoint {i} contents missing from synced store"
        );
    }

    Ok(())
}
