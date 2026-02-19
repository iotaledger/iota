// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, time::Duration};

use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::LIST_OWNED_OBJECTS_READ_MASK,
    v1::state_service::{
        GetCoinInfoRequest, ListOwnedObjectsRequest, ListOwnedObjectsResponse,
        state_service_client::StateServiceClient,
    },
};
use iota_json_rpc_types::IotaObjectDataOptions;
use iota_macros::sim_test;
use iota_test_transaction_builder::publish_package;
use iota_types::{
    base_types::{IotaAddress, StructTag, TypeTag},
    effects::TransactionEffectsAPI,
    object::Owner,
    parse_iota_struct_tag,
    transaction::{CallArg, ObjectArg},
};
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
    let random_addr = IotaAddress::random();
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
/// This exercises the real owner index, `owner_bounds` cursor seeking,
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
    // because page_size=1 walks in owner-index key order while the full page
    // may use a different natural order, but the *set* must be identical).
    let expected_set: std::collections::HashSet<_> = expected_ids.iter().collect();
    assert_eq!(
        unique, expected_set,
        "paginated IDs do not match single-page IDs"
    );
}

/// Poll `list_owned_objects` until `owner` reports exactly `expected_count`
/// objects, failing the test if the expected state is not observed within
/// the timeout. Used to bridge the lag between transaction execution on the
/// fullnode and owner-index updates that happen during checkpoint commit.
async fn wait_for_owned_count(
    state_client: &mut StateServiceClient<iota_grpc_client::InterceptedChannel>,
    owner: IotaAddress,
    expected_count: usize,
    scenario: &str,
) -> ListOwnedObjectsResponse {
    const TIMEOUT: Duration = Duration::from_secs(30);
    const POLL_INTERVAL: Duration = Duration::from_millis(100);

    tokio::time::timeout(TIMEOUT, async {
        loop {
            let response = state_client
                .list_owned_objects(
                    ListOwnedObjectsRequest::default().with_owner(address_proto(owner)),
                )
                .await
                .unwrap()
                .into_inner();
            if response.objects.len() == expected_count {
                return response;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("timed out waiting for {scenario}: expected owned count {expected_count}")
    })
}

#[sim_test]
async fn list_owned_objects_tto_indexing() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    // Publish the `move_test_code` package (contains the `tto_coin` module).
    let package_ref = publish_package(
        &test_cluster.wallet,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move_test_code"),
    )
    .await;
    let package_id = package_ref.object_id;

    // The sender owns several gas coins by default; pick one that is not the
    // gas-payment coin so we can pass it as the `Coin<IOTA>` argument to
    // `start`. `test_transaction_builder_with_sender` picks the first gas
    // coin returned by the RPC as the payment, so the second one is free
    // for use as an argument.
    let coin_ref = {
        let coins = test_cluster
            .wallet
            .get_all_gas_objects_owned_by_address(sender)
            .await
            .unwrap();
        assert!(
            coins.len() >= 2,
            "sender needs at least 2 gas coins to separate gas payment from the coin argument"
        );
        coins[1]
    };

    // Run `start(coin)` — creates an `A` object owned by the sender and
    // transfers `coin` to `A`'s address via TTO.
    let start_tx = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .move_call(
            package_id,
            "tto_coin",
            "start",
            vec![CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_ref))],
        )
        .build();
    let signed_start = test_cluster.sign_transaction(&start_tx);
    let (start_effects, _) = test_cluster
        .execute_transaction_return_raw_effects(signed_start)
        .await
        .unwrap();
    assert!(start_effects.status().is_ok(), "start tx must succeed");

    let parent_ref = start_effects
        .created()
        .iter()
        .find_map(|(obj_ref, owner)| match owner {
            Owner::Address(addr) if *addr == sender => Some(*obj_ref),
            _ => None,
        })
        .expect("start should create an `A` object owned by the sender");
    let parent_addr = IotaAddress::from(parent_ref.object_id);

    // The coin is now owned by `parent_addr` via TTO; grab its post-start ref.
    let coin_after_start = start_effects
        .mutated_excluding_gas()
        .iter()
        .find_map(|(obj_ref, _)| (obj_ref.object_id == coin_ref.object_id).then_some(*obj_ref))
        .expect("coin must appear in mutated set after start");

    // Parent starts with 1 coin (TTO'd in by `start`).
    wait_for_owned_count(&mut state_client, parent_addr, 1, "parent after start").await;

    // 0x0 starts with 0 coins.
    wait_for_owned_count(
        &mut state_client,
        IotaAddress::ZERO,
        0,
        "0x0 before receive",
    )
    .await;

    // Run `receive(parent, coin)` — receives the coin from `A` and transfers
    // it to `0x0`.
    let receive_tx = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .move_call(
            package_id,
            "tto_coin",
            "receive",
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(parent_ref)),
                CallArg::Object(ObjectArg::Receiving(coin_after_start)),
            ],
        )
        .build();
    let signed_receive = test_cluster.sign_transaction(&receive_tx);
    let (receive_effects, _) = test_cluster
        .execute_transaction_return_raw_effects(signed_receive)
        .await
        .unwrap();
    assert!(receive_effects.status().is_ok(), "receive tx must succeed");

    // Parent ends with 0 coins.
    wait_for_owned_count(&mut state_client, parent_addr, 0, "parent after receive").await;

    // 0x0 ends with 1 coin.
    wait_for_owned_count(&mut state_client, IotaAddress::ZERO, 1, "0x0 after receive").await;
}

/// Collect the set of object IDs from a `ListOwnedObjectsResponse`.
fn object_id_set(response: &ListOwnedObjectsResponse) -> std::collections::HashSet<Vec<u8>> {
    response
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
        .collect()
}

#[sim_test]
async fn list_owned_objects_filter_by_type() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let sender = first_sender(&test_cluster);

    let iota = "0x2::coin::Coin<0x2::iota::IOTA>"
        .parse::<TypeTag>()
        .unwrap()
        .to_string();

    // We start with some IOTA coins — verify by filtering by `Coin<IOTA>` and
    // comparing against the unfiltered list (IOTA's proto `Object` does not
    // carry an `object_type` string, so we check the "all are Coin<IOTA>"
    // invariant by ID-set equality against a type-filtered query).
    let unfiltered = state_client
        .list_owned_objects(ListOwnedObjectsRequest::default().with_owner(address_proto(sender)))
        .await
        .unwrap()
        .into_inner();
    assert!(!unfiltered.objects.is_empty());

    let iota_filtered = state_client
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(address_proto(sender))
                .with_object_type(iota.clone()),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        object_id_set(&unfiltered),
        object_id_set(&iota_filtered),
        "sender should start owning only Coin<IOTA> objects"
    );

    // Publish the `trusted_coin` package
    let package_ref = publish_package(
        &test_cluster.wallet,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/grpc/data/trusted_coin"),
    )
    .await;
    let package_id = package_ref.object_id;

    // Wait for the publish tx to land in a checkpoint so the owner index
    // reflects the newly-created TreasuryCap.
    test_cluster.wait_for_checkpoint(2, None).await;

    let trusted = format!("{package_id}::trusted_coin::TRUSTED_COIN")
        .parse::<TypeTag>()
        .unwrap()
        .to_string();
    let trusted_coin = format!("0x2::coin::Coin<{trusted}>")
        .parse::<TypeTag>()
        .unwrap()
        .to_string();
    let treasury_cap_type = StructTag::new_treasury_cap(parse_iota_struct_tag(&trusted).unwrap())
        .to_canonical_string(true);

    // After publishing we have the treasury cap and the coin metadata has 0
    // supply
    let coin_info = state_client
        .get_coin_info(GetCoinInfoRequest::default().with_coin_type(trusted.clone()))
        .await
        .unwrap()
        .into_inner();
    let metadata = coin_info.metadata.as_ref().expect("should have metadata");
    assert_eq!(coin_info.coin_type.as_deref(), Some(trusted.as_str()));
    assert_eq!(metadata.symbol.as_deref(), Some("TRUSTED"));
    assert_eq!(
        metadata.description.as_deref(),
        Some("Trusted Coin for test")
    );
    assert_eq!(metadata.name.as_deref(), Some("Trusted Coin"));
    assert_eq!(metadata.decimals, Some(2));
    assert_eq!(
        coin_info.treasury.as_ref().expect("treasury").total_supply,
        Some(0)
    );

    let treasury_cap_filtered = state_client
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(address_proto(sender))
                .with_object_type(treasury_cap_type.clone()),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(treasury_cap_filtered.objects.len(), 1);

    // Look up the TreasuryCap's ObjectRef so we can pass it to `mint`.
    let owned = test_cluster
        .get_owned_objects(sender, Some(IotaObjectDataOptions::full_content()))
        .await
        .unwrap();
    let treasury_cap_ref = owned
        .iter()
        .find_map(|resp| {
            let data = resp.data.as_ref()?;
            let struct_tag: StructTag = data.type_.clone()?.try_into().ok()?;
            (struct_tag.to_canonical_string(true) == treasury_cap_type).then(|| data.object_ref())
        })
        .expect("sender should own a TreasuryCap after publish");

    // Mint some coins
    let mint_tx = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .move_call(
            package_id,
            "trusted_coin",
            "mint",
            vec![
                CallArg::Object(ObjectArg::ImmOrOwnedObject(treasury_cap_ref)),
                CallArg::Pure(bcs::to_bytes(&100_000u64).unwrap()),
            ],
        )
        .build();
    let signed_mint = test_cluster.sign_transaction(&mint_tx);
    let (mint_effects, _) = test_cluster
        .execute_transaction_return_raw_effects(signed_mint)
        .await
        .unwrap();
    assert!(mint_effects.status().is_ok(), "mint tx must succeed");

    // Wait for the mint tx to land in a checkpoint so the owner index and
    // the treasury supply reflect the new coin.
    test_cluster.wait_for_checkpoint(3, None).await;

    // After minting we should have some of the new coins and the supply should
    // have updated
    let coin_info = state_client
        .get_coin_info(GetCoinInfoRequest::default().with_coin_type(trusted.clone()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(coin_info.coin_type.as_deref(), Some(trusted.as_str()));
    assert_eq!(
        coin_info.treasury.as_ref().expect("treasury").total_supply,
        Some(100_000)
    );

    let trusted_coin_filtered = state_client
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(address_proto(sender))
                .with_object_type(trusted_coin.clone()),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(trusted_coin_filtered.objects.len(), 1);

    let iota_filtered = state_client
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(address_proto(sender))
                .with_object_type(iota.clone()),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(iota_filtered.objects.len(), 5);

    // Calling `list_owned_objects` with `0x2::coin::Coin` filter (without a
    // type T) should return all coins
    let bare_coin_filtered = state_client
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(address_proto(sender))
                .with_object_type("0x2::coin::Coin".to_owned()),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(bare_coin_filtered.objects.len(), 6);
    // The 6 returned objects must be exactly the union of the 5 `Coin<IOTA>`
    // and the 1 `Coin<TRUSTED_COIN>` — proves the bare filter matches all
    // `Coin<T>` regardless of `T`.
    let expected_union: std::collections::HashSet<Vec<u8>> = object_id_set(&iota_filtered)
        .union(&object_id_set(&trusted_coin_filtered))
        .cloned()
        .collect();
    assert_eq!(object_id_set(&bare_coin_filtered), expected_union);
}
