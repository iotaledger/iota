// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use tonic::Code;

use crate::{
    common::{
        EXECUTE_TRANSACTIONS_PATH, MockGrpcStateReader, PendingExecutor,
        call_execute_transactions_with_grpc_timeout,
    },
    harness::start_with_executor,
};

/// A call that the server ends at its `grpc-timeout` is counted with the status
/// of its response, not as canceled.
#[tokio::test]
async fn a_call_past_its_grpc_timeout_is_counted_as_deadline_exceeded() {
    let (handle, reader) = start_with_executor(
        MockGrpcStateReader::default(),
        Some(Arc::new(PendingExecutor::default())),
        |_| {},
    )
    .await;

    assert_eq!(
        call_execute_transactions_with_grpc_timeout(handle.address(), Duration::from_millis(200))
            .await,
        Code::DeadlineExceeded
    );

    let requests = |status| {
        reader.value(
            "requests",
            &[("method", EXECUTE_TRANSACTIONS_PATH), ("status", status)],
        )
    };
    assert_eq!(
        (requests("DeadlineExceeded"), requests("canceled")),
        (1.0, 0.0),
        "(DeadlineExceeded, canceled)"
    );
}
