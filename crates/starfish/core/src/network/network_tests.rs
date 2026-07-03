// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures::StreamExt as _;
use parking_lot::Mutex;
use rstest::rstest;

use super::{
    NetworkClient, SerializedBlockBundle, test_network::TestService, tonic_network::TonicManager,
};
use crate::{Round, commit::CommitRange, context::Context};

fn serialized_block_bundle_for_round(round: Round) -> SerializedBlockBundle {
    SerializedBlockBundle {
        serialized_block_bundle: Bytes::from(vec![round as u8; 16]),
    }
}

fn service_with_own_block_bundles() -> Arc<Mutex<TestService>> {
    let service = Arc::new(Mutex::new(TestService::new()));
    {
        let mut service = service.lock();
        let own_blocks = (0..=100u8)
            .map(|i| serialized_block_bundle_for_round(i as Round))
            .collect::<Vec<_>>();
        service.add_own_blocks(own_blocks);
    }
    service
}

#[rstest]
#[tokio::test]
async fn subscribe_and_receive_block_bundles() {
    let (context, keys) = Context::new_for_test(4);

    let context_0 = Arc::new(
        context
            .clone()
            .with_authority_index(context.committee.to_authority_index(0).unwrap()),
    );
    let mut manager_0 = TonicManager::new(context_0.clone(), keys[0].0.clone());
    let client_0 = manager_0.client();
    let service_0 = service_with_own_block_bundles();
    manager_0.install_service(service_0.clone()).await;

    let context_1 = Arc::new(
        context
            .clone()
            .with_authority_index(context.committee.to_authority_index(1).unwrap()),
    );
    let mut manager_1 = TonicManager::new(context_1.clone(), keys[1].0.clone());
    let client_1 = manager_1.client();
    let service_1 = service_with_own_block_bundles();
    manager_1.install_service(service_1.clone()).await;

    let client_0_round = 50;
    let receive_stream_0 = client_0
        .subscribe_block_bundles(
            context_0.committee.to_authority_index(1).unwrap(),
            client_0_round,
            Duration::from_secs(5),
        )
        .await
        .unwrap();

    let count = receive_stream_0
        .enumerate()
        .then(|(i, item)| async move {
            assert_eq!(
                item,
                serialized_block_bundle_for_round(client_0_round + i as Round + 1)
            );
            1
        })
        .fold(0, |a, b| async move { a + b })
        .await;
    // Round 51 to 100 blocks should have been received.
    assert_eq!(count, 50);

    let client_1_round = 100;
    let mut receive_stream_1 = client_1
        .subscribe_block_bundles(
            context_1.committee.to_authority_index(0).unwrap(),
            client_1_round,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(receive_stream_1.next().await.is_none());
}

/// The client's configured `fast_commit_sync_max_fetch_bytes` cap reaches the
/// server as the `max_transaction_bytes` argument to
/// `handle_fetch_commits_and_transactions`, over a real client/server round
/// trip, instead of the hardcoded `0` the tonic layer used to pass.
#[rstest]
#[tokio::test]
async fn fetch_commits_and_transactions_forwards_configured_byte_cap() {
    let (context, keys) = Context::new_for_test(4);
    const CONFIGURED_CAP: usize = 12_345;

    let mut parameters = context.parameters.clone();
    parameters.fast_commit_sync_max_fetch_bytes = CONFIGURED_CAP;
    let context_0 = Arc::new(
        context
            .clone()
            .with_parameters(parameters)
            .with_authority_index(context.committee.to_authority_index(0).unwrap()),
    );
    let manager_0 = TonicManager::<Mutex<TestService>>::new(context_0.clone(), keys[0].0.clone());
    let client_0 = manager_0.client();

    let context_1 = Arc::new(
        context
            .clone()
            .with_authority_index(context.committee.to_authority_index(1).unwrap()),
    );
    let mut manager_1 = TonicManager::new(context_1.clone(), keys[1].0.clone());
    let service_1 = Arc::new(Mutex::new(TestService::new()));
    manager_1.install_service(service_1.clone()).await;

    let commit_range: CommitRange = (1..=5).into();
    let peer_1 = context_0.committee.to_authority_index(1).unwrap();
    let _ = client_0
        .fetch_commits_and_transactions(peer_1, commit_range.clone(), Duration::from_secs(5))
        .await
        .unwrap();

    let calls = service_1
        .lock()
        .handle_fetch_commits_and_transactions
        .clone();
    assert_eq!(calls.len(), 1);
    let (received_peer, received_range, received_cap) = &calls[0];
    assert_eq!(
        *received_peer,
        context_0.committee.to_authority_index(0).unwrap()
    );
    assert_eq!(*received_range, commit_range);
    assert_eq!(*received_cap, CONFIGURED_CAP);
}
