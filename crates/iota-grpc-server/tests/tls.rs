// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration test for serving the gRPC API over TLS.

mod common;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use common::{MockGrpcStateReader, owner_proto, start_test_server};
use iota_config::node::TlsConfig;
use iota_grpc_types::v1::state_service::{
    ListOwnedObjectsRequest, state_service_client::StateServiceClient,
};
use iota_sdk_types::Address;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};

#[tokio::test]
async fn serves_requests_over_tls() {
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::write(&cert_path, cert.pem()).unwrap();
    std::fs::write(&key_path, key_pair.serialize_pem()).unwrap();
    let tls_config: TlsConfig = serde_json::from_value(serde_json::json!({
        "cert": cert_path,
        "key": key_path,
    }))
    .unwrap();

    let (handle, _reader) = start_test_server(Arc::new(MockGrpcStateReader::default()), |config| {
        config.tls = Some(tls_config);
    })
    .await;

    let client_tls_config = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(cert.pem()))
        .domain_name("localhost");
    let endpoint = Channel::from_shared(format!("https://{}", handle.address()))
        .unwrap()
        .tls_config(client_tls_config)
        .unwrap();
    // With TLS, the server binds its port in a background task after
    // `start_grpc_server` returns.
    let deadline = Instant::now() + Duration::from_secs(10);
    let channel = loop {
        match endpoint.connect().await {
            Ok(channel) => break channel,
            Err(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(e) => panic!("failed to connect to the TLS gRPC server: {e}"),
        }
    };

    StateServiceClient::new(channel)
        .list_owned_objects(
            ListOwnedObjectsRequest::default().with_owner(owner_proto(Address::random())),
        )
        .await
        .unwrap();
}
