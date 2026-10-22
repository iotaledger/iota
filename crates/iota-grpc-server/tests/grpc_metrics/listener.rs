// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::ledger_service::{
    GetServiceInfoRequest, ledger_service_client::LedgerServiceClient,
};
use tonic::transport::Channel;

use crate::{
    common::{MockGrpcStateReader, TestCertificate},
    harness::{connect, start, start_with},
};

async fn get_service_info(channel: &Channel) {
    LedgerServiceClient::new(channel.clone())
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn the_listener_counts_the_connections_of_the_api() {
    let (handle, reader) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let channel = connect(&handle).await;
    get_service_info(&channel).await;

    assert_eq!(
        (
            reader.value("inbound_connections_accepted", &[("scope", "loopback")]),
            reader.value("inbound_connections", &[]),
        ),
        (1.0, 1.0),
        "(accepted, open)"
    );
    assert!(
        !reader.has_family("pending_handshakes_peak"),
        "a listener without TLS has no TLS metrics"
    );
}

#[tokio::test]
async fn the_listener_times_the_tls_handshakes_of_the_api() {
    let certificate = TestCertificate::new();
    let (handle, reader) = start_with(MockGrpcStateReader::new_from_iter(0..=10), |config| {
        config.tls = Some(certificate.tls_config.clone())
    })
    .await;
    let channel = certificate.connect(handle.address()).await;
    get_service_info(&channel).await;

    assert_eq!(
        reader
            .histogram_totals("handshake_latency", &[("result", "completed")])
            .count,
        1
    );
    assert_eq!(reader.value("pending_handshakes_peak", &[]), 1.0);
}
