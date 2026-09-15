// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the minimum SDK version header: every response,
//! successful or not, carries it, and the bundled SDK client accepts it.

mod common;

use std::sync::Arc;

use common::{MockGrpcStateReader, start_test_server};
use iota_grpc_client::{Client, ResponseExt};
use iota_grpc_server::{GrpcServerHandle, constants::MIN_SDK_VERSION};
use iota_grpc_types::{
    headers::X_IOTA_MIN_SDK_VERSION,
    v1::ledger_service::{GetServiceInfoRequest, ledger_service_client::LedgerServiceClient},
};
use prost_types::FieldMask;
use tonic::{Code, metadata::MetadataMap, transport::Channel};

async fn start_server() -> GrpcServerHandle {
    let mock = Arc::new(MockGrpcStateReader::new_from_iter(0..3));
    let (handle, _) = start_test_server(mock, |_| {}).await;
    handle
}

async fn connect_ledger_client(handle: &GrpcServerHandle) -> LedgerServiceClient<Channel> {
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .unwrap();
    LedgerServiceClient::new(channel)
}

fn min_sdk_version(metadata: &MetadataMap) -> &str {
    metadata
        .get(X_IOTA_MIN_SDK_VERSION)
        .expect("response carries the minimum SDK version header")
        .to_str()
        .unwrap()
}

#[tokio::test]
async fn successful_response_carries_the_minimum_sdk_version() {
    let handle = start_server().await;
    let mut client = connect_ledger_client(&handle).await;

    let response = client
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .unwrap();
    assert_eq!(min_sdk_version(response.metadata()), MIN_SDK_VERSION);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn error_response_carries_the_minimum_sdk_version() {
    let handle = start_server().await;
    let mut client = connect_ledger_client(&handle).await;

    let invalid_read_mask = FieldMask {
        paths: vec!["no_such_field".to_owned()],
    };
    let status = client
        .get_service_info(GetServiceInfoRequest::default().with_read_mask(invalid_read_mask))
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::InvalidArgument, "{status:?}");
    assert_eq!(min_sdk_version(status.metadata()), MIN_SDK_VERSION);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn bundled_sdk_client_passes_the_version_check() {
    let handle = start_server().await;
    let client = Client::new(format!("http://{}", handle.address())).unwrap();

    let response = client
        .ledger_service_client()
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .unwrap();
    assert_eq!(response.min_sdk_version(), Some(MIN_SDK_VERSION));

    handle.shutdown().await.unwrap();
}
