// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Instant};

use iota_json_rpc_api::WriteApiClient;
use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_types::{quorum_driver_types::ExecuteTransactionRequestType, transaction::Transaction};

use crate::{
    error::{Error, IotaRpcResult},
    RpcClient,
};

const WAIT_FOR_LOCAL_EXECUTION_RETRY_COUNT: u8 = 3;

/// Quorum API that provides functionality to execute a transaction block and
/// submit it to the fullnode(s).
#[derive(Clone)]
pub struct QuorumDriverApi {
    api: Arc<RpcClient>,
}

impl QuorumDriverApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Execute a transaction with a FullNode client. `request_type`
    /// defaults to `ExecuteTransactionRequestType::WaitForLocalExecution`.
    /// When `ExecuteTransactionRequestType::WaitForLocalExecution` is used,
    /// but returned `confirmed_local_execution` is false, the client will
    /// keep retry for WAIT_FOR_LOCAL_EXECUTION_RETRY_COUNT times. If it
    /// still fails, it will return an error.
    pub async fn execute_transaction_block(
        &self,
        tx: Transaction,
        options: IotaTransactionBlockResponseOptions,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> IotaRpcResult<IotaTransactionBlockResponse> {
        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();
        let request_type = request_type.unwrap_or_else(|| options.default_execution_request_type());
        let mut retry_count = 0;
        let start = Instant::now();
        while retry_count < WAIT_FOR_LOCAL_EXECUTION_RETRY_COUNT {
            let response: IotaTransactionBlockResponse = self
                .api
                .http
                .execute_transaction_block(
                    tx_bytes.clone(),
                    signatures.clone(),
                    Some(options.clone()),
                    Some(request_type.clone()),
                )
                .await?;

            match request_type {
                ExecuteTransactionRequestType::WaitForEffectsCert => {
                    return Ok(response);
                }
                ExecuteTransactionRequestType::WaitForLocalExecution => {
                    if let Some(true) = response.confirmed_local_execution {
                        return Ok(response);
                    } else {
                        // If fullnode executed the cert in the network but did not confirm local
                        // execution, it must have timed out and hence we could retry.
                        retry_count += 1;
                    }
                }
            }
        }
        Err(Error::FailToConfirmTransactionStatus(
            *tx.digest(),
            start.elapsed().as_secs(),
        ))
    }
}
