// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use iota_json_rpc_api::{ReadApiClient, WriteApiClient};
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_sdk_types::TransactionDigest;
use iota_types::{
    quorum_driver_types::ExecuteTransactionRequestType, transaction::TransactionEnvelope,
};

use crate::{
    RpcClient,
    error::{Error, IotaRpcResult},
};

const WAIT_FOR_LOCAL_EXECUTION_MIN_INTERVAL: Duration = Duration::from_millis(100);
const WAIT_FOR_LOCAL_EXECUTION_MAX_INTERVAL: Duration = Duration::from_secs(2);

/// Defines methods to execute transaction blocks and submit them to fullnodes.
#[derive(Clone)]
pub struct QuorumDriverApi {
    api: Arc<RpcClient>,
}

impl QuorumDriverApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Execute a transaction with a FullNode client.
    ///
    /// The request type defaults to
    /// [`ExecuteTransactionRequestType::WaitForLocalExecution`] when
    /// `options` require effects (see
    /// [`IotaTransactionBlockResponseOptions::require_effects`]), and to
    /// [`ExecuteTransactionRequestType::WaitForEffectsCert`] otherwise.
    ///
    /// Under `WaitForLocalExecution` the returned response is guaranteed to be
    /// queryable on the node that served the request. Whenever the node does
    /// not confirm local execution — whether because it does not honor the
    /// request type, or because it honors it but cannot confirm in time — the
    /// client polls until the transaction becomes visible; if that does not
    /// happen within the timeout, the call fails with
    /// [`Error::FailToConfirmTransactionStatus`].
    ///
    /// `checkpoint` and `timestamp_ms` are never populated on the returned
    /// response, on either path; only the read API populates them. Errors
    /// from the node are propagated to the caller as-is, including a
    /// finality timeout the node itself may raise while waiting for local
    /// execution.
    pub async fn execute_transaction_block(
        &self,
        tx: TransactionEnvelope,
        options: IotaTransactionBlockResponseOptions,
        request_type: impl Into<Option<ExecuteTransactionRequestType>>,
    ) -> IotaRpcResult<IotaTransactionBlockResponse> {
        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();
        let request_type = request_type
            .into()
            .unwrap_or_else(|| options.default_execution_request_type());
        let wait_for_local_execution = matches!(
            request_type,
            ExecuteTransactionRequestType::WaitForLocalExecution
        );

        let start = Instant::now();
        let mut response = self
            .api
            .http
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(options.clone()),
                Some(request_type.into()),
            )
            .await?;

        if !wait_for_local_execution || response.confirmed_local_execution == Some(true) {
            return Ok(response);
        }

        self.wait_until_visible(*tx.digest(), &options, start)
            .await?;
        response.confirmed_local_execution = Some(true);
        Ok(response)
    }

    /// Waits for `digest` to become queryable on the node, for nodes that
    /// answer `WaitForLocalExecution` without confirming local execution.
    /// The response is discarded; this is a barrier, not a data source.
    async fn wait_until_visible(
        &self,
        digest: TransactionDigest,
        options: &IotaTransactionBlockResponseOptions,
        start: Instant,
    ) -> IotaRpcResult<()> {
        // In simtests, fullnodes can stop receiving checkpoints for > 30s.
        let wait_for_local_execution_timeout: Duration = if cfg!(msim) {
            Duration::from_secs(120)
        } else {
            Duration::from_secs(60)
        };
        tokio::time::timeout(wait_for_local_execution_timeout, async {
            let mut backoff = iota_common::backoff::ExponentialBackoff::new(
                WAIT_FOR_LOCAL_EXECUTION_MIN_INTERVAL,
                WAIT_FOR_LOCAL_EXECUTION_MAX_INTERVAL,
            );
            loop {
                // Wait before the first request too, to leave time for the
                // checkpoint containing the transaction to be certified,
                // propagate to the full node, and get executed.
                tokio::time::sleep(backoff.next().unwrap()).await;

                if self
                    .api
                    .http
                    .get_transaction_block(digest, Some(options.clone()))
                    .await
                    .is_ok()
                {
                    return;
                }
            }
        })
        .await
        .map_err(|_| Error::FailToConfirmTransactionStatus(digest, start.elapsed().as_secs()))
    }
}
