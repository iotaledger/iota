// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::{HeadersInterceptor, ResponseExt};
use iota_macros::sim_test;

use super::super::utils::setup_grpc_test;

/// Assert every standard `x-iota-*` header is present on the given response,
/// via the [`ResponseExt`] accessors.
fn assert_standard_headers(response: &impl ResponseExt, label: &str) {
    assert!(response.chain().is_some(), "{label}: chain header missing");
    assert!(
        response.chain_id().is_some(),
        "{label}: chain_id header missing"
    );
    assert!(response.epoch().is_some(), "{label}: epoch header missing");
    assert!(
        response.checkpoint_height().is_some(),
        "{label}: checkpoint_height header missing"
    );
    assert!(
        response.timestamp_ms().is_some(),
        "{label}: timestamp_ms header missing"
    );
    assert!(
        response.timestamp().is_some(),
        "{label}: timestamp header missing"
    );
    assert!(
        response.lowest_available_checkpoint().is_some(),
        "{label}: lowest_available_checkpoint header missing"
    );
    assert!(
        response.lowest_available_checkpoint_objects().is_some(),
        "{label}: lowest_available_checkpoint_objects header missing"
    );
    assert!(
        response.server_version().is_some(),
        "{label}: server_version header missing"
    );
}

/// Smoke-test that the high-level client surfaces IOTA metadata headers via
/// the [`ResponseExt`] trait.  Per-endpoint header population is exhaustively
/// covered at the low level in
/// `tests/grpc/v1/{ledger_service,transaction_execution_service}/header.rs`;
/// this test's job is to verify only the client-side `ResponseExt` plumbing.
#[sim_test]
async fn metadata_envelope_headers() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let service_info = client
        .get_service_info(None)
        .await
        .expect("get_service_info should succeed");
    assert_standard_headers(&service_info, "get_service_info");

    let health = client
        .get_health(None)
        .await
        .expect("get_health should succeed");
    assert_standard_headers(&health, "get_health");
}

/// Verify that the client successfully attaches HTTP auth credentials and the
/// server still serves the request.  The fixture's gRPC server does not
/// enforce auth, so this tests only the interceptor wiring end-to-end (request
/// succeeds with the auth header attached).  Header *content* is verified at
/// the unit level in `iota_grpc_client::interceptors::tests`.
#[sim_test]
async fn metadata_envelope_with_auth() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    type ConfigureAuth = fn(&mut HeadersInterceptor);
    let scenarios: &[(&str, ConfigureAuth)] = &[
        ("basic auth", |i| i.basic_auth("user", Some("pass"))),
        ("bearer auth", |i| {
            i.bearer_auth("my-token").unwrap();
        }),
    ];

    for (label, configure) in scenarios {
        let mut interceptor = HeadersInterceptor::new();
        configure(&mut interceptor);
        let authed_client = client.clone().with_headers(interceptor);

        let service_info = authed_client
            .get_service_info(None)
            .await
            .unwrap_or_else(|e| panic!("get_service_info should succeed with {label}: {e}"));
        assert_standard_headers(&service_info, &format!("get_service_info ({label})"));
    }
}
