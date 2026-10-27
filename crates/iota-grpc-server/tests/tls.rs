// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration test for serving the gRPC API over TLS.

mod common;

use std::sync::Arc;

use common::{MockGrpcStateReader, TestCertificate, owner_proto, start_test_server};
use iota_grpc_types::v1::state_service::{
    ListOwnedObjectsRequest, state_service_client::StateServiceClient,
};
use iota_sdk_types::Address;

#[tokio::test]
async fn serves_requests_over_tls() {
    let certificate = TestCertificate::new();
    let (handle, _reader) = start_test_server(Arc::new(MockGrpcStateReader::default()), |config| {
        config.tls = Some(certificate.tls_config.clone());
    })
    .await;

    StateServiceClient::new(certificate.connect(handle.address()).await)
        .list_owned_objects(
            ListOwnedObjectsRequest::default().with_owner(owner_proto(Address::random())),
        )
        .await
        .unwrap();
}
