// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pagination edge-case tests for `ListPackageVersions`.
//!
//! Uses `MockGrpcStateReader` populated with controlled data so we can
//! exercise cursor-based seeking without a full validator cluster.

mod common;

use std::sync::Arc;

use common::{
    MockGrpcStateReader, connect_move_package_service_client, object_id_proto, start_test_server,
};
use iota_grpc_types::v1::move_package_service::{
    ListPackageVersionsRequest, ListPackageVersionsResponse,
};
use iota_sdk_types::ObjectId;
use iota_types::storage::{PackageVersionInfo, PackageVersionKey};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the returned version numbers from a response.
fn returned_versions(resp: &ListPackageVersionsResponse) -> Vec<u64> {
    resp.versions.iter().map(|v| v.version.unwrap()).collect()
}

/// Set up a mock with sequential versions `1..=count` of a single package.
fn make_package_version_mock(original_package_id: ObjectId, count: u64) -> MockGrpcStateReader {
    let package_versions = (1..=count)
        .map(|version| {
            (
                PackageVersionKey {
                    original_package_id,
                    version,
                },
                PackageVersionInfo {
                    storage_id: ObjectId::random(),
                },
            )
        })
        .collect();
    MockGrpcStateReader {
        package_versions,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Walk through all versions with page_size=1 and verify every version is
/// returned exactly once, in order, with no duplicates or gaps: a regression
/// that skips an extra item on top of the exclusive-cursor store (e.g. the
/// old inclusive-cursor-plus-skip pattern applied twice) would drop every
/// other version.
#[tokio::test]
async fn paginate_one_at_a_time() {
    let original_package_id = ObjectId::random();
    let version_count = 5u64;
    let mock = make_package_version_mock(original_package_id, version_count);

    let (handle, _reader) = start_test_server(Arc::new(mock), |_| {}).await;
    let mut client = connect_move_package_service_client(&handle).await;

    let base = ListPackageVersionsRequest::default()
        .with_package_id(object_id_proto(original_package_id))
        .with_page_size(1);

    let mut returned: Vec<u64> = Vec::new();
    let mut page_token = None;
    // Walk at most `version_count + 2` pages, so a cursor that stops
    // advancing fails the test instead of hanging it.
    let mut pages_left = version_count + 2;
    loop {
        assert!(
            pages_left > 0,
            "cursor did not advance: page budget exhausted"
        );
        pages_left -= 1;
        let mut request = base.clone();
        if let Some(token) = page_token.take() {
            request = request.with_page_token(token);
        }
        let resp = client
            .list_package_versions(request)
            .await
            .unwrap()
            .into_inner();

        assert!(
            resp.versions.len() <= 1,
            "page_size=1 but got {} versions",
            resp.versions.len()
        );
        returned.extend(returned_versions(&resp));

        match resp.next_page_token {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }

    let expected: Vec<u64> = (1..=version_count).collect();
    assert_eq!(
        returned, expected,
        "every version must be returned exactly once, in order",
    );
}
