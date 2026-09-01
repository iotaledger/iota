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
    json_rpc_error,
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
    /// Under `WaitForLocalExecution` the client polls the read API before
    /// returning, whenever the node either does not confirm local execution
    /// or fails with a transient error. If that poll times out, the call
    /// returns whichever the node already gave it: the response, with
    /// `confirmed_local_execution` left as the node reported it, or the
    /// node's error.
    ///
    /// `checkpoint` and `timestamp_ms` are not populated on a response that
    /// came from the execute call; only the read API sets them.
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
        let response = match self
            .api
            .http
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(options.clone()),
                Some(request_type.into()),
            )
            .await
        {
            Ok(response) => {
                if !wait_for_local_execution || response.confirmed_local_execution == Some(true) {
                    return Ok(response);
                }
                Ok(response)
            }
            Err(err) => {
                if !wait_for_local_execution || !is_transient_error(&err) {
                    return Err(err.into());
                }
                // A transient error carries no effects to fall back on, but the
                // transaction may still land; poll for it rather than failing a
                // call the network is going to finalize anyway.
                Err(err)
            }
        };

        // Both remaining cases wait for the transaction to become locally
        // readable. A poll timeout is not itself a failure: the caller gets
        // back whichever answer the node already gave.
        let poll_response = self.wait_until_visible(*tx.digest(), &options, start).await;
        match (response, poll_response) {
            (Ok(mut response), Ok(_)) | (Err(_), Ok(mut response)) => {
                response.confirmed_local_execution = Some(true);
                Ok(response)
            }
            (Ok(response), Err(_)) => Ok(response),
            (Err(e), Err(_)) => Err(e.into()),
        }
    }

    /// Polls the read API until `digest` can be read back on the node that
    /// served the request.
    async fn wait_until_visible(
        &self,
        digest: TransactionDigest,
        options: &IotaTransactionBlockResponseOptions,
        start: Instant,
    ) -> IotaRpcResult<IotaTransactionBlockResponse> {
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

                if let Ok(poll_response) = self
                    .api
                    .http
                    .get_transaction_block(digest, Some(options.clone()))
                    .await
                {
                    return poll_response;
                }
            }
        })
        .await
        .map_err(|_| Error::FailToConfirmTransactionStatus(digest, start.elapsed().as_secs()))
    }
}

/// Whether `err` is a node-side error worth polling past rather than
/// surfacing immediately.
fn is_transient_error(err: &jsonrpsee::core::ClientError) -> bool {
    match err {
        jsonrpsee::core::ClientError::Call(object) => {
            json_rpc_error::Error::from(jsonrpsee::core::ClientError::Call(object.clone()))
                .is_transient_error()
        }
        _ => false,
    }
}
