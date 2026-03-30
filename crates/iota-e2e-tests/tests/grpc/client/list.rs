// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_sdk_types::{Address, ObjectId};

use super::super::utils::setup_grpc_test;

/// Get the first wallet address from a test cluster.
fn first_sender(cluster: &test_cluster::TestCluster) -> Address {
    let iota_addr = cluster.wallet.get_addresses().first().copied().unwrap();
    Address::new(iota_addr.to_inner())
}

// ==========================================================================
// list_owned_objects
// ==========================================================================

#[sim_test]
async fn list_owned_objects_single_page() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let owner = first_sender(&test_cluster);

    let page = client
        .list_owned_objects(owner, None, None, None, None)
        .await
        .expect("single page should succeed");

    assert!(
        !page.body().items.is_empty(),
        "Sender should own at least one object (gas coins)"
    );
}

#[sim_test]
async fn list_owned_objects_collect_all() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let owner = first_sender(&test_cluster);

    let all = client
        .list_owned_objects(owner, None, None, None, None)
        .collect(None)
        .await
        .expect("collect should succeed");

    assert!(
        !all.body().is_empty(),
        "Sender should own at least one object (gas coins)"
    );
}

#[sim_test]
async fn list_owned_objects_pagination_with_token() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let owner = first_sender(&test_cluster);

    // Fetch first page with page_size=1
    let page1 = client
        .list_owned_objects(owner, None, Some(1), None, None)
        .await
        .expect("first page should succeed");

    assert_eq!(
        page1.body().items.len(),
        1,
        "First page should have exactly 1 item"
    );

    // Collect all for comparison
    let all = client
        .list_owned_objects(owner, None, None, None, None)
        .collect(None)
        .await
        .expect("collect all should succeed");

    // If there are more objects, the token should allow continuation
    if all.body().len() > 1 {
        let token = page1
            .body()
            .next_page_token
            .as_ref()
            .expect("Should have next_page_token when more objects exist");

        let page2 = client
            .list_owned_objects(owner, None, Some(1), Some(token.clone()), None)
            .await
            .expect("second page should succeed");

        assert_eq!(
            page2.body().items.len(),
            1,
            "Second page should have exactly 1 item"
        );

        // The two pages should return different objects
        let id1 = &page1.body().items[0];
        let id2 = &page2.body().items[0];
        assert_ne!(
            format!("{id1:?}"),
            format!("{id2:?}"),
            "Pages should return different objects"
        );
    }
}

#[sim_test]
async fn list_owned_objects_collect_with_limit() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let owner = first_sender(&test_cluster);

    // Collect all first to see how many there are
    let all = client
        .list_owned_objects(owner, None, None, None, None)
        .collect(None)
        .await
        .expect("collect all should succeed");

    if all.body().len() > 1 {
        // Collect with limit=1, should get at most 1
        let limited = client
            .list_owned_objects(owner, None, Some(1), None, None)
            .collect(Some(1))
            .await
            .expect("collect with limit should succeed");

        assert_eq!(
            limited.body().len(),
            1,
            "Collect with limit=1 should return exactly 1 item"
        );
    }
}

#[sim_test]
async fn list_owned_objects_collect_limit_truncates() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let owner = first_sender(&test_cluster);

    // Collect everything to confirm we have more than 2 objects.
    let all = client
        .list_owned_objects(owner, None, None, None, None)
        .collect(None)
        .await
        .expect("collect all should succeed");

    assert!(
        all.body().len() > 2,
        "Test requires > 2 owned objects, got {}",
        all.body().len()
    );

    // Collect with limit=2 but NO page_size — the server will use its
    // default (50), which is larger than the limit. The client must
    // truncate the result to exactly 2 items.
    let limited = client
        .list_owned_objects(owner, None, None, None, None)
        .collect(Some(2))
        .await
        .expect("collect with limit should succeed");

    assert_eq!(
        limited.body().len(),
        2,
        "collect(Some(2)) without page_size must return exactly 2 items, got {}",
        limited.body().len()
    );
}

// ==========================================================================
// list_dynamic_fields
// ==========================================================================

#[sim_test]
async fn list_dynamic_fields_single_page() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    // System state object (0x5) always has dynamic fields
    let parent: ObjectId = "0x5".parse().unwrap();

    let page = client
        .list_dynamic_fields(parent, None, None, None)
        .await
        .expect("single page should succeed");

    assert!(
        !page.body().items.is_empty(),
        "System state object should have at least one dynamic field"
    );
}

#[sim_test]
async fn list_dynamic_fields_collect_all() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let parent: ObjectId = "0x5".parse().unwrap();

    let all = client
        .list_dynamic_fields(parent, None, None, None)
        .collect(None)
        .await
        .expect("collect should succeed");

    assert!(
        !all.body().is_empty(),
        "System state object should have at least one dynamic field"
    );
}

#[sim_test]
async fn list_dynamic_fields_pagination_with_token() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let parent: ObjectId = "0x5".parse().unwrap();

    // Fetch first page with page_size=1
    let page1 = client
        .list_dynamic_fields(parent, Some(1), None, None)
        .await
        .expect("first page should succeed");

    assert_eq!(
        page1.body().items.len(),
        1,
        "First page should have exactly 1 item"
    );

    // Collect all for comparison
    let all = client
        .list_dynamic_fields(parent, None, None, None)
        .collect(None)
        .await
        .expect("collect all should succeed");

    if all.body().len() > 1 {
        let token = page1
            .body()
            .next_page_token
            .as_ref()
            .expect("Should have next_page_token when more fields exist");

        let page2 = client
            .list_dynamic_fields(parent, Some(1), Some(token.clone()), None)
            .await
            .expect("second page should succeed");

        assert_eq!(
            page2.body().items.len(),
            1,
            "Second page should have exactly 1 item"
        );
    }
}

// ==========================================================================
// list_package_versions
// ==========================================================================

#[sim_test]
async fn list_package_versions_single_page() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    // System package 0x2 always exists
    let package_id: ObjectId = "0x2".parse().unwrap();

    let page = client
        .list_package_versions(package_id, None, None)
        .await
        .expect("single page should succeed");

    assert!(
        !page.body().items.is_empty(),
        "System package should have at least one version"
    );
}

#[sim_test]
async fn list_package_versions_collect_all() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let package_id: ObjectId = "0x2".parse().unwrap();

    let all = client
        .list_package_versions(package_id, None, None)
        .collect(None)
        .await
        .expect("collect should succeed");

    assert!(
        !all.body().is_empty(),
        "System package should have at least one version"
    );
}
