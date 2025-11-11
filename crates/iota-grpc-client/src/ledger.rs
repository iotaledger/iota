// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::ledger_service as grpc_ledger_service;
use tonic::transport::Channel;

/// Dedicated client for ledger-related gRPC operations.
///
/// This client handles all ledger service interactions like getting objects,
/// transactions, checkpoints, epochs and a service info.
#[derive(Clone)]
pub struct LedgerClient {
    _client: grpc_ledger_service::ledger_service_client::LedgerServiceClient<Channel>,
}

impl LedgerClient {
    /// Create a new LedgerClient from a shared gRPC channel.
    pub(super) fn new(channel: Channel) -> Self {
        Self {
            _client: grpc_ledger_service::ledger_service_client::LedgerServiceClient::new(channel),
        }
    }
}
