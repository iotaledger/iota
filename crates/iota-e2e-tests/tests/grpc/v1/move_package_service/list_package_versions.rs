// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::move_package_service::ListPackageVersionsRequest;
use iota_macros::sim_test;

use crate::utils::{assert_tonic_error, object_id_from_hex, setup_grpc_test};

#[sim_test]
async fn list_package_versions_framework_package() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut pkg_client = client.move_package_service_client();

    // 0x2 is the iota-framework package, should have at least version 1
    let request = ListPackageVersionsRequest::default().with_package_id(object_id_from_hex("0x2"));

    let response = pkg_client
        .list_package_versions(request)
        .await
        .unwrap()
        .into_inner();

    assert!(
        !response.versions.is_empty(),
        "Framework package should have at least 1 version, got 0"
    );

    // Each version should have original_id, storage_id and version number
    for version in &response.versions {
        assert!(
            version.original_id.is_some(),
            "Each version should have an original_id"
        );
        assert!(
            version.version.is_some(),
            "Each version should have a version number"
        );
        assert!(
            version.storage_id.is_some(),
            "Each version should have a storage_id"
        );
    }
}

#[sim_test]
async fn list_package_versions_with_page_size() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut pkg_client = client.move_package_service_client();

    let request = ListPackageVersionsRequest::default()
        .with_package_id(object_id_from_hex("0x2"))
        .with_page_size(1);

    let response = pkg_client
        .list_package_versions(request)
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        response.versions.len(),
        1,
        "Framework package with page_size=1 should return exactly 1 version, got {}",
        response.versions.len()
    );
}

#[sim_test]
async fn list_package_versions_missing_package_id() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;
    let mut pkg_client = client.move_package_service_client();

    // Missing package_id should return InvalidArgument
    let result = pkg_client
        .list_package_versions(ListPackageVersionsRequest::default())
        .await;

    assert_tonic_error(result, tonic::Code::InvalidArgument, "missing package_id");
}

#[sim_test]
async fn list_package_versions_nonexistent_package() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut pkg_client = client.move_package_service_client();

    // Non-existent package ID
    let request =
        ListPackageVersionsRequest::default().with_package_id(object_id_from_hex("0xdead"));

    let result = pkg_client.list_package_versions(request).await;

    assert_tonic_error(result, tonic::Code::NotFound, "non-existent package");
}

#[sim_test]
async fn list_package_versions_non_package_object() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut pkg_client = client.move_package_service_client();

    // 0x5 is the system state object, not a package
    let request = ListPackageVersionsRequest::default().with_package_id(object_id_from_hex("0x5"));

    let result = pkg_client.list_package_versions(request).await;

    assert_tonic_error(result, tonic::Code::InvalidArgument, "non-package object");
}
