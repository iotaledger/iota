// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the minimum SDK version header: every response
//! carries it.

mod common;

use std::sync::Arc;

use common::{MockGrpcStateReader, start_test_server};
use iota_grpc_server::{
    GrpcServerHandle,
    constants::{MIN_IOTA_SDK_GRPC_CLIENT_VERSION, X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION},
};
use iota_grpc_types::v1::{
    ledger_service::{GetServiceInfoRequest, ledger_service_client::LedgerServiceClient},
    transaction_execution_service::{
        ExecuteTransactionsRequest,
        transaction_execution_service_client::TransactionExecutionServiceClient,
    },
};
use tonic::{Code, metadata::MetadataMap, transport::Channel};

async fn start_server() -> GrpcServerHandle {
    let mock = Arc::new(MockGrpcStateReader::new_from_iter(0..3));
    let (handle, _) = start_test_server(mock, |_| {}).await;
    handle
}

async fn connect(handle: &GrpcServerHandle) -> Channel {
    Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .unwrap()
}

async fn connect_ledger_client(handle: &GrpcServerHandle) -> LedgerServiceClient<Channel> {
    LedgerServiceClient::new(connect(handle).await)
}

fn min_iota_sdk_grpc_client_version(metadata: &MetadataMap) -> &str {
    metadata
        .get(X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION)
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
    assert_eq!(
        min_iota_sdk_grpc_client_version(response.metadata()),
        MIN_IOTA_SDK_GRPC_CLIENT_VERSION
    );

    handle.shutdown().await.unwrap();
}

/// The test server runs without a transaction executor, so the transaction
/// execution service is not registered and no handler answers this call.
#[tokio::test]
async fn error_response_carries_the_minimum_sdk_version() {
    let handle = start_server().await;
    let mut client = TransactionExecutionServiceClient::new(connect(&handle).await);

    let status = client
        .execute_transactions(ExecuteTransactionsRequest::default())
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::Unimplemented);
    assert_eq!(
        min_iota_sdk_grpc_client_version(status.metadata()),
        MIN_IOTA_SDK_GRPC_CLIENT_VERSION
    );

    handle.shutdown().await.unwrap();
}
