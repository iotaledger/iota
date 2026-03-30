// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil, read_masks::LIST_OWNED_OBJECTS_READ_MASK,
    v1::state_service::ListOwnedObjectsRequest,
};
use iota_macros::sim_test;
use iota_types::base_types::IotaAddress;
use prost_types::FieldMask;

use crate::utils::{
    NFT_PACKAGE, address_proto, assert_field_presence, assert_tonic_error,
    comma_separated_field_mask_to_paths, publish_example_package, setup_grpc_test,
};

/// Get the first wallet address from a test cluster.
fn first_sender(cluster: &test_cluster::TestCluster) -> IotaAddress {
    cluster.wallet.get_addresses().first().copied().unwrap()
}

/// Make a unary call and validate field presence on every returned object.
async fn list_and_validate(
    state_client: &mut iota_grpc_types::v1::state_service::state_service_client::StateServiceClient<
        iota_grpc_client::InterceptedChannel,
    >,
    request: ListOwnedObjectsRequest,
    expected_field_mask_paths: &[&str],
    scenario: &str,
) -> iota_grpc_types::v1::state_service::ListOwnedObjectsResponse {
    let response = state_client
        .list_owned_objects(request)
        .await
        .unwrap()
        .into_inner();

    for (idx, object) in response.objects.iter().enumerate() {
        assert_field_presence(
            object,
            expected_field_mask_paths,
            &[],
            &format!("{scenario} (object {idx})"),
        );
    }

    response
}

#[sim_test]
async fn list_owned_objects_default_readmask() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    let request = ListOwnedObjectsRequest::default().with_owner(address_proto(sender));

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "default readmask",
    )
    .await;

    assert!(
        !response.objects.is_empty(),
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

    let response = list_and_validate(
        &mut state_client,
        request,
        &["reference.object_id"],
        "partial readmask",
    )
    .await;

    assert!(
        !response.objects.is_empty(),
        "Should return objects with partial mask"
    );
}

#[sim_test]
async fn list_owned_objects_with_page_size() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    let request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_page_size(2);

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "with page_size=2",
    )
    .await;

    assert_eq!(
        response.objects.len(),
        2,
        "Should return exactly 2 objects, got {}",
        response.objects.len()
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

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "nonexistent owner",
    )
    .await;

    assert_eq!(
        response.objects.len(),
        0,
        "Nonexistent owner should have 0 objects"
    );
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

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by exact Coin<IOTA> type",
    )
    .await;

    assert!(
        !response.objects.is_empty(),
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

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by Coin without type params",
    )
    .await;

    assert!(
        !response.objects.is_empty(),
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

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by non-matching NFT type",
    )
    .await;

    assert_eq!(
        response.objects.len(),
        0,
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

    let response = list_and_validate(
        &mut state_client,
        request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by exact NFT type after mint",
    )
    .await;

    let total_nfts = response.objects.len();
    assert_eq!(
        total_nfts, 1,
        "Sender should own exactly 1 NFT after minting"
    );

    // Filter by Coin type — should still return gas coins but not the NFT
    let coin_request = ListOwnedObjectsRequest::default()
        .with_owner(address_proto(sender))
        .with_object_type("0x2::coin::Coin<0x2::iota::IOTA>".to_string());

    let coin_response = list_and_validate(
        &mut state_client,
        coin_request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "filter by Coin type after NFT mint",
    )
    .await;

    let total_coins = coin_response.objects.len();
    assert!(
        total_coins > 0,
        "Sender should still own gas coins after minting NFT"
    );

    // Total filtered objects should be less than unfiltered (which includes
    // both coins and NFT)
    let unfiltered_request = ListOwnedObjectsRequest::default().with_owner(address_proto(sender));

    let unfiltered_response = list_and_validate(
        &mut state_client,
        unfiltered_request,
        &comma_separated_field_mask_to_paths(LIST_OWNED_OBJECTS_READ_MASK),
        "unfiltered after NFT mint",
    )
    .await;

    let total_all = unfiltered_response.objects.len();
    // The sender owns only Coin<IOTA> objects and the minted TestnetNFT in this
    // test configuration, so total_all == total_coins + total_nfts.
    assert!(
        total_all >= total_coins + total_nfts,
        "Unfiltered count ({total_all}) should be at least coins ({total_coins}) + NFTs ({total_nfts})"
    );
}

/// Walk through all owned objects one at a time using cursor-based pagination.
///
/// This exercises the real v2 owner index, `owner_v2_bounds` cursor seeking,
/// and page-token round-tripping through the full gRPC stack — something
/// the unit tests with `MockGrpcStateReader` cannot cover.
#[sim_test]
async fn list_owned_objects_cursor_pagination_e2e() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    // First, get the total count in a single large page.
    let all_response = list_and_validate(
        &mut state_client,
        ListOwnedObjectsRequest::default()
            .with_owner(address_proto(sender))
            .with_read_mask(FieldMask::from_str("reference.object_id")),
        &["reference.object_id"],
        "full page",
    )
    .await;

    let total_count = all_response.objects.len();
    assert!(
        total_count >= 2,
        "need at least 2 objects to test pagination, got {total_count}"
    );

    // Collect all object IDs from the single-page response.
    let expected_ids: Vec<_> = all_response
        .objects
        .iter()
        .map(|o| {
            o.reference
                .as_ref()
                .unwrap()
                .object_id
                .as_ref()
                .unwrap()
                .object_id
                .to_vec()
        })
        .collect();

    // Now paginate one-by-one using page_size=1.
    let mut paginated_ids = Vec::new();
    let mut next_page_token = None;
    let mut pages = 0u32;

    loop {
        let mut request = ListOwnedObjectsRequest::default()
            .with_owner(address_proto(sender))
            .with_page_size(1)
            .with_read_mask(FieldMask::from_str("reference.object_id"));

        if let Some(token) = next_page_token.take() {
            request = request.with_page_token(token);
        }

        let resp = state_client
            .list_owned_objects(request)
            .await
            .unwrap()
            .into_inner();

        for obj in &resp.objects {
            paginated_ids.push(
                obj.reference
                    .as_ref()
                    .unwrap()
                    .object_id
                    .as_ref()
                    .unwrap()
                    .object_id
                    .to_vec(),
            );
        }

        pages += 1;
        assert!(
            pages <= total_count as u32 + 1,
            "pagination loop exceeded expected page count — possible infinite loop"
        );

        match resp.next_page_token {
            Some(token) => next_page_token = Some(token),
            None => break,
        }
    }

    // Every object must appear exactly once.
    assert_eq!(
        paginated_ids.len(),
        total_count,
        "paginated walk returned {} objects, expected {total_count}",
        paginated_ids.len()
    );

    let unique: std::collections::HashSet<_> = paginated_ids.iter().collect();
    assert_eq!(
        unique.len(),
        paginated_ids.len(),
        "duplicate object IDs found across pages"
    );

    // The set of IDs must match the single-page response (order may differ
    // because page_size=1 walks in v2-key order while the full page may use
    // a different natural order, but the *set* must be identical).
    let expected_set: std::collections::HashSet<_> = expected_ids.iter().collect();
    assert_eq!(
        unique, expected_set,
        "paginated IDs do not match single-page IDs"
    );
}
