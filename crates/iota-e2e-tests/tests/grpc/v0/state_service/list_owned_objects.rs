// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::LIST_OWNED_OBJECTS_READ_MASK,
    v0::state_service::{
        ListOwnedObjectsRequest, ListOwnedObjectsResponse, state_service_client::StateServiceClient,
    },
};
use iota_macros::sim_test;
use iota_types::base_types::IotaAddress;
use prost_types::FieldMask;

use crate::{
    collect_streaming_responses,
    utils::{
        NFT_PACKAGE, address_proto, assert_field_presence, assert_tonic_error,
        comma_separated_field_mask_to_paths, publish_example_package, setup_grpc_test,
    },
};

/// Get the first wallet address from a test cluster.
fn first_sender(cluster: &test_cluster::TestCluster) -> IotaAddress {
    cluster.wallet.get_addresses().first().copied().unwrap()
}

/// Collect all streaming responses, validating has_next and field presence.
///
/// Uses [`collect_streaming_responses!`] for stream/has_next validation, then
/// additionally asserts field presence on every returned object.
async fn collect_list_owned_objects(
    state_client: &mut StateServiceClient<iota_grpc_client::InterceptedChannel>,
    request: ListOwnedObjectsRequest,
    expected_field_mask_paths: &[&str],
    scenario: &str,
) -> Vec<ListOwnedObjectsResponse> {
    let responses =
        collect_streaming_responses!(state_client, list_owned_objects, request, scenario);

    // Additional field-presence validation specific to owned objects
    for (resp_idx, response) in responses.iter().enumerate() {
        for (obj_idx, object) in response.objects.iter().enumerate() {
            assert_field_presence(
                object,
                expected_field_mask_paths,
                &[],
                &format!("{scenario} (response {}, object {obj_idx})", resp_idx + 1),
            );
        }
    }

    responses
}

#[sim_test]
async fn list_owned_objects_default_readmask() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    let request = ListOwnedObjectsRequest::default().with_owner(address_proto(sender));

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "default readmask",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert!(
        total_objects > 0,
        "Sender should own at least one object (gas coins)"
    );
}

#[sim_test]
async fn list_owned_objects_with_readmask() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_read_mask(FieldMask::from_paths(["reference.object_id"]));

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &["reference.object_id"],
        "partial readmask",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert!(total_objects > 0, "Should return objects with partial mask");
}

#[sim_test]
async fn list_owned_objects_with_limit() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_limit(2);

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "with limit=2",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(
        total_objects, 2,
        "Should return exactly 2 objects, got {total_objects}"
    );
}

#[sim_test]
async fn list_owned_objects_empty_owner() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;
    let mut state_client = client.state_service_client();

    // Missing owner should return InvalidArgument
    let result = state_client
        .list_owned_objects(ListOwnedObjectsRequest::default())
        .await;

    assert_tonic_error(result, tonic::Code::InvalidArgument, "missing owner");
}

#[sim_test]
async fn list_owned_objects_nonexistent_owner() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    // Random address that owns nothing
    let random_addr = IotaAddress::random_for_testing_only();
    let request = ListOwnedObjectsRequest::default().with_owner(address_proto(random_addr));

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "nonexistent owner",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(total_objects, 0, "Nonexistent owner should have 0 objects");
}

#[sim_test]
async fn list_owned_objects_filter_by_exact_type() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    // Filter by exact Coin<IOTA> type (including type params).
    // The sender owns gas coins of type 0x2::coin::Coin<0x2::iota::IOTA>.
    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_object_type("0x2::coin::Coin<0x2::iota::IOTA>".to_string());

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by exact Coin<IOTA> type",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert!(
        total_objects > 0,
        "Sender should own at least one Coin<IOTA> object"
    );
}

#[sim_test]
async fn list_owned_objects_filter_by_type_without_type_params() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    // Filter by 0x2::coin::Coin without type params — should match all Coin<T>
    // objects regardless of the type parameter T.
    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_object_type("0x2::coin::Coin".to_string());

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by Coin without type params",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert!(
        total_objects > 0,
        "Sender should own at least one Coin object when filtering without type params"
    );
}

#[sim_test]
async fn list_owned_objects_filter_by_nonexistent_type() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    // Publish the NFT package so its type is valid, then filter by the NFT type.
    // The sender has NOT minted any NFTs, so the result should be empty.
    let nft_package_id = publish_example_package(&test_cluster, sender, NFT_PACKAGE).await;

    // Wait for the publish transaction to land in a checkpoint
    test_cluster.wait_for_checkpoint(2, None).await;

    let nft_type = format!("{nft_package_id}::testnet_nft::TestnetNFT");
    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_object_type(nft_type);

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by non-matching NFT type",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(
        total_objects, 0,
        "Sender should have 0 objects of NFT type (none minted)"
    );
}

#[sim_test]
async fn list_owned_objects_filter_by_type_exact_match_with_mint() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    // Publish the NFT package and mint an NFT so the sender owns one.
    let nft_package_id = publish_example_package(&test_cluster, sender, NFT_PACKAGE).await;

    let mint_tx = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .call_nft_create(nft_package_id)
        .build();
    let signed_tx = test_cluster.sign_transaction(&mint_tx);
    test_cluster.execute_transaction(signed_tx).await;

    // Wait for the mint transaction to land in a checkpoint
    test_cluster.wait_for_checkpoint(3, None).await;

    // Filter by exact NFT type — should return exactly 1 NFT
    let nft_type = format!("{nft_package_id}::testnet_nft::TestnetNFT");
    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_object_type(nft_type);

    let responses = collect_list_owned_objects(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by exact NFT type after mint",
    )
    .await;

    let total_nfts: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(
        total_nfts, 1,
        "Sender should own exactly 1 NFT after minting"
    );

    // Filter by Coin type — should still return gas coins but not the NFT
    let coin_request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_object_type("0x2::coin::Coin<0x2::iota::IOTA>".to_string());

    let coin_responses = collect_list_owned_objects(
        &mut state_client,
        coin_request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by Coin type after NFT mint",
    )
    .await;

    let total_coins: usize = coin_responses.iter().map(|r| r.objects.len()).sum();
    assert!(
        total_coins > 0,
        "Sender should still own gas coins after minting NFT"
    );

    // Total filtered objects should be less than unfiltered (which includes
    // both coins and NFT)
    let unfiltered_request = ListOwnedObjectsRequest::default().with_owner(address_proto(sender));

    let unfiltered_responses = collect_list_owned_objects(
        &mut state_client,
        unfiltered_request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "unfiltered after NFT mint",
    )
    .await;

    let total_all: usize = unfiltered_responses.iter().map(|r| r.objects.len()).sum();
    // The sender owns only Coin<IOTA> objects and the minted TestnetNFT in this
    // test configuration, so total_all == total_coins + total_nfts.
    assert!(
        total_all >= total_coins + total_nfts,
        "Unfiltered count ({total_all}) should be at least coins ({total_coins}) + NFTs ({total_nfts})"
    );
}
