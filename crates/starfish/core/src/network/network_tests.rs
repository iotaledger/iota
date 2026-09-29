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
use crate::{Round, context::Context};

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

#[tokio::test]
async fn a_peer_at_its_subscription_cap_is_rejected_on_another_connection() {
    let (context, keys) = Context::new_for_test(4);
    let mut parameters = context.parameters.clone();
    parameters.tonic.admission.max_subscriptions_per_peer = 1;
    let context = context.with_parameters(parameters);
    let server_index = context.committee.to_authority_index(0).unwrap();

    let server_context = Arc::new(context.clone().with_authority_index(server_index));
    let mut server = TonicManager::new(server_context, keys[0].0.clone());
    server
        .install_service(Arc::new(Mutex::new(TestService {
            endless_subscriptions: true,
            ..TestService::new()
        })))
        .await;

    let client_for = |authority: usize| {
        let client_context = Arc::new(
            context
                .clone()
                .with_authority_index(context.committee.to_authority_index(authority).unwrap()),
        );
        TonicManager::<Mutex<TestService>>::new(client_context, keys[authority].0.clone()).client()
    };
    // Two clients for the same authority, so each opens its own connection.
    let first = client_for(1);
    let second = client_for(1);

    let held = first
        .subscribe_block_bundles(server_index, 0, Duration::from_secs(5))
        .await
        .unwrap();

    let Err(rejected) = second
        .subscribe_block_bundles(server_index, 0, Duration::from_secs(5))
        .await
    else {
        panic!("the peer holds its only subscription slot");
    };
    assert!(
        format!("{rejected:?}").contains("ResourceExhausted"),
        "{rejected:?}"
    );

    // Another authority has its own budget.
    let _other_peer = client_for(2)
        .subscribe_block_bundles(server_index, 0, Duration::from_secs(5))
        .await
        .unwrap();

    // The slot comes back once the subscription is dropped, which the server
    // sees as the response body ending.
    drop(held);
    let mut resubscribed = false;
    for _ in 0..50 {
        if second
            .subscribe_block_bundles(server_index, 0, Duration::from_secs(5))
            .await
            .is_ok()
        {
            resubscribed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(resubscribed, "dropping a subscription should free its slot");
}
