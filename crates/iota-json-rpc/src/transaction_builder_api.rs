// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use iota_json::IotaJsonValue;
use iota_json_rpc_api::{TransactionBuilderOpenRpc, TransactionBuilderServer, internal_error};
use iota_json_rpc_types::{
    IotaTransactionBlockBuilderMode, IotaTypeTag, RPCTransactionRequestParams,
    TransactionBlockBytes,
};
use iota_open_rpc::Module;
use iota_sdk_types::{Address, ObjectId};
use iota_transaction_builder::{DataReader, TransactionBuilder};
use iota_types::iota_serde::BigInt;
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::IotaRpcModule;

pub struct TransactionBuilderApi(TransactionBuilder);

impl TransactionBuilderApi {
    pub fn new(data_reader: Arc<dyn DataReader + Sync + Send>) -> Self {
        Self(TransactionBuilder::new(data_reader))
    }
}

#[async_trait]
impl TransactionBuilderServer for TransactionBuilderApi {
    async fn transfer_object(
        &self,
        signer: Address,
        object_id: ObjectId,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
        recipient: Address,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .transfer_object(signer, object_id, gas, *gas_budget, recipient)
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn transfer_iota(
        &self,
        signer: Address,
        iota_object_id: ObjectId,
        gas_budget: BigInt<u64>,
        recipient: Address,
        amount: Option<BigInt<u64>>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .transfer_iota(
                signer,
                iota_object_id,
                *gas_budget,
                recipient,
                amount.map(|a| *a),
            )
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn pay(
        &self,
        signer: Address,
        input_coins: Vec<ObjectId>,
        recipients: Vec<Address>,
        amounts: Vec<BigInt<u64>>,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .pay(
                signer,
                input_coins,
                recipients,
                amounts.into_iter().map(|a| *a).collect(),
                gas,
                *gas_budget,
            )
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn pay_iota(
        &self,
        signer: Address,
        input_coins: Vec<ObjectId>,
        recipients: Vec<Address>,
        amounts: Vec<BigInt<u64>>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .pay_iota(
                signer,
                input_coins,
                recipients,
                amounts.into_iter().map(|a| *a).collect(),
                *gas_budget,
            )
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn pay_all_iota(
        &self,
        signer: Address,
        input_coins: Vec<ObjectId>,
        recipient: Address,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .pay_all_iota(signer, input_coins, recipient, *gas_budget)
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn publish(
        &self,
        sender: Address,
        compiled_modules: Vec<Base64>,
        dependencies: Vec<ObjectId>,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let compiled_modules = compiled_modules
            .into_iter()
            .map(|data| data.to_vec().map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(internal_error)?;
        let data = self
            .0
            .publish(sender, compiled_modules, dependencies, gas, *gas_budget)
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn split_coin(
        &self,
        signer: Address,
        coin_object_id: ObjectId,
        split_amounts: Vec<BigInt<u64>>,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let split_amounts = split_amounts.into_iter().map(|a| *a).collect();
        let data = self
            .0
            .split_coin(signer, coin_object_id, split_amounts, gas, *gas_budget)
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn split_coin_equal(
        &self,
        signer: Address,
        coin_object_id: ObjectId,
        split_count: BigInt<u64>,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .split_coin_equal(signer, coin_object_id, *split_count, gas, *gas_budget)
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn merge_coin(
        &self,
        signer: Address,
        primary_coin: ObjectId,
        coin_to_merge: ObjectId,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .merge_coins(signer, primary_coin, coin_to_merge, gas, *gas_budget)
            .await
            .map_err(internal_error)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(internal_error)?)
    }

    async fn move_call(
        &self,
        signer: Address,
        package_object_id: ObjectId,
        module: String,
        function: String,
        type_arguments: Vec<IotaTypeTag>,
        rpc_arguments: Vec<IotaJsonValue>,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
        _txn_builder_mode: Option<IotaTransactionBlockBuilderMode>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .move_call(
                    signer,
                    package_object_id,
                    &module,
                    &function,
                    type_arguments,
                    rpc_arguments,
                    gas,
                    *gas_budget,
                    None,
                )
                .await
                .map_err(internal_error)?,
        )
        .map_err(internal_error)?)
    }

    async fn batch_transaction(
        &self,
        signer: Address,
        params: Vec<RPCTransactionRequestParams>,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
        _txn_builder_mode: Option<IotaTransactionBlockBuilderMode>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .batch_transaction(signer, params, gas, *gas_budget)
                .await
                .map_err(internal_error)?,
        )
        .map_err(internal_error)?)
    }

    async fn request_add_stake(
        &self,
        signer: Address,
        coins: Vec<ObjectId>,
        amount: Option<BigInt<u64>>,
        validator: Address,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let amount = amount.map(|a| *a);
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_add_stake(signer, coins, amount, validator, gas, *gas_budget)
                .await
                .map_err(internal_error)?,
        )
        .map_err(internal_error)?)
    }

    async fn request_withdraw_stake(
        &self,
        signer: Address,
        staked_iota: ObjectId,
        gas: Option<ObjectId>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_withdraw_stake(signer, staked_iota, gas, *gas_budget)
                .await
                .map_err(internal_error)?,
        )
        .map_err(internal_error)?)
    }

    async fn request_add_timelocked_stake(
        &self,
        signer: Address,
        locked_balance: ObjectId,
        validator: Address,
        gas: ObjectId,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_add_timelocked_stake(signer, locked_balance, validator, gas, *gas_budget)
                .await
                .map_err(internal_error)?,
        )
        .map_err(internal_error)?)
    }

    async fn request_withdraw_timelocked_stake(
        &self,
        signer: Address,
        timelocked_staked_iota: ObjectId,
        gas: ObjectId,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_withdraw_timelocked_stake(signer, timelocked_staked_iota, gas, *gas_budget)
                .await
                .map_err(internal_error)?,
        )
        .map_err(internal_error)?)
    }
}

impl IotaRpcModule for TransactionBuilderApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        TransactionBuilderOpenRpc::module_doc()
    }
}
