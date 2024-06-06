// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use iota_json_rpc::IotaRpcModule;
use iota_json_rpc_api::{WriteApiClient, WriteApiServer};
use iota_json_rpc_types::{
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
};
use iota_open_rpc::Module;
use iota_types::{
    base_types::IotaAddress, iota_serde::BigInt, quorum_driver_types::ExecuteTransactionRequestType,
};
use jsonrpsee::{core::RpcResult, http_client::HttpClient, RpcModule};

use crate::types::IotaTransactionBlockResponseWithOptions;

pub(crate) struct WriteApi {
    fullnode: HttpClient,
}

impl WriteApi {
    pub fn new(fullnode_client: HttpClient) -> Self {
        Self {
            fullnode: fullnode_client,
        }
    }
}

#[async_trait]
impl WriteApiServer for WriteApi {
    async fn execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        options: Option<IotaTransactionBlockResponseOptions>,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> RpcResult<IotaTransactionBlockResponse> {
        let iota_transaction_response = self
            .fullnode
            .execute_transaction_block(tx_bytes, signatures, options.clone(), request_type)
            .await?;
        Ok(IotaTransactionBlockResponseWithOptions {
            response: iota_transaction_response,
            options: options.unwrap_or_default(),
        }
        .into())
    }

    async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx_bytes: Base64,
        gas_price: Option<BigInt<u64>>,
        epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> RpcResult<DevInspectResults> {
        self.fullnode
            .dev_inspect_transaction_block(
                sender_address,
                tx_bytes,
                gas_price,
                epoch,
                additional_args,
            )
            .await
    }

    async fn dry_run_transaction_block(
        &self,
        tx_bytes: Base64,
    ) -> RpcResult<DryRunTransactionBlockResponse> {
        self.fullnode.dry_run_transaction_block(tx_bytes).await
    }
}

impl IotaRpcModule for WriteApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::WriteApiOpenRpc::module_doc()
    }
}
