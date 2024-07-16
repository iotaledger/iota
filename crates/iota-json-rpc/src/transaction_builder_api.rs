// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use iota_core::authority::AuthorityState;
use iota_json::IotaJsonValue;
use iota_json_rpc_api::{TransactionBuilderOpenRpc, TransactionBuilderServer};
use iota_json_rpc_types::{
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse,
    IotaTransactionBlockBuilderMode, IotaTypeTag, RPCTransactionRequestParams,
    TransactionBlockBytes,
};
use iota_open_rpc::Module;
use iota_transaction_builder::{DataReader, TransactionBuilder};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectInfo},
    iota_serde::BigInt,
};
use jsonrpsee::{
    core::RpcResult,
    types::{error::INTERNAL_ERROR_CODE, ErrorObjectOwned},
    RpcModule,
};
use move_core_types::language_storage::StructTag;

use crate::{authority_state::StateRead, IotaRpcModule};

pub struct TransactionBuilderApi(TransactionBuilder);

impl TransactionBuilderApi {
    pub fn new(state: Arc<AuthorityState>) -> Self {
        let reader = Arc::new(AuthorityStateDataReader::new(state));
        Self(TransactionBuilder::new(reader))
    }

    pub fn new_with_data_reader(data_reader: Arc<dyn DataReader + Sync + Send>) -> Self {
        Self(TransactionBuilder::new(data_reader))
    }
}

pub struct AuthorityStateDataReader(Arc<dyn StateRead>);

impl AuthorityStateDataReader {
    pub fn new(state: Arc<AuthorityState>) -> Self {
        Self(state)
    }
}

#[async_trait]
impl DataReader for AuthorityStateDataReader {
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        object_type: StructTag,
    ) -> Result<Vec<ObjectInfo>, anyhow::Error> {
        Ok(self
            .0
            // DataReader is used internally, don't need a limit
            .get_owner_objects(
                address,
                None,
                Some(IotaObjectDataFilter::StructType(object_type)),
            )?)
    }

    async fn get_object_with_options(
        &self,
        object_id: ObjectID,
        options: IotaObjectDataOptions,
    ) -> Result<IotaObjectResponse, anyhow::Error> {
        let result = self.0.get_object_read(&object_id)?;
        Ok((result, options).try_into()?)
    }

    async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        let epoch_store = self.0.load_epoch_store_one_call_per_task();
        Ok(epoch_store.reference_gas_price())
    }
}

#[async_trait]
impl TransactionBuilderServer for TransactionBuilderApi {
    async fn transfer_object(
        &self,
        signer: IotaAddress,
        object_id: ObjectID,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
        recipient: IotaAddress,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .transfer_object(signer, object_id, gas, *gas_budget, recipient)
            .await
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn transfer_iota(
        &self,
        signer: IotaAddress,
        iota_object_id: ObjectID,
        gas_budget: BigInt<u64>,
        recipient: IotaAddress,
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
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn pay(
        &self,
        signer: IotaAddress,
        input_coins: Vec<ObjectID>,
        recipients: Vec<IotaAddress>,
        amounts: Vec<BigInt<u64>>,
        gas: Option<ObjectID>,
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
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn pay_iota(
        &self,
        signer: IotaAddress,
        input_coins: Vec<ObjectID>,
        recipients: Vec<IotaAddress>,
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
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn pay_all_iota(
        &self,
        signer: IotaAddress,
        input_coins: Vec<ObjectID>,
        recipient: IotaAddress,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .pay_all_iota(signer, input_coins, recipient, *gas_budget)
            .await
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn publish(
        &self,
        sender: IotaAddress,
        compiled_modules: Vec<Base64>,
        dependencies: Vec<ObjectID>,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let compiled_modules = compiled_modules
            .into_iter()
            .map(|data| data.to_vec().map_err(|e| anyhow::anyhow!(e)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(rpc_error_from_internal)?;
        let data = self
            .0
            .publish(sender, compiled_modules, dependencies, gas, *gas_budget)
            .await
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn split_coin(
        &self,
        signer: IotaAddress,
        coin_object_id: ObjectID,
        split_amounts: Vec<BigInt<u64>>,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let split_amounts = split_amounts.into_iter().map(|a| *a).collect();
        let data = self
            .0
            .split_coin(signer, coin_object_id, split_amounts, gas, *gas_budget)
            .await
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn split_coin_equal(
        &self,
        signer: IotaAddress,
        coin_object_id: ObjectID,
        split_count: BigInt<u64>,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .split_coin_equal(signer, coin_object_id, *split_count, gas, *gas_budget)
            .await
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn merge_coin(
        &self,
        signer: IotaAddress,
        primary_coin: ObjectID,
        coin_to_merge: ObjectID,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let data = self
            .0
            .merge_coins(signer, primary_coin, coin_to_merge, gas, *gas_budget)
            .await
            .map_err(rpc_error_from_internal)?;
        Ok(TransactionBlockBytes::from_data(data).map_err(rpc_error_from_internal)?)
    }

    async fn move_call(
        &self,
        signer: IotaAddress,
        package_object_id: ObjectID,
        module: String,
        function: String,
        type_arguments: Vec<IotaTypeTag>,
        rpc_arguments: Vec<IotaJsonValue>,
        gas: Option<ObjectID>,
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
                .map_err(|e| {
                    ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                })?,
        )
        .map_err(rpc_error_from_internal)?)
    }

    async fn batch_transaction(
        &self,
        signer: IotaAddress,
        params: Vec<RPCTransactionRequestParams>,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
        _txn_builder_mode: Option<IotaTransactionBlockBuilderMode>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .batch_transaction(signer, params, gas, *gas_budget)
                .await
                .map_err(|e| {
                    ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                })?,
        )
        .map_err(rpc_error_from_internal)?)
    }

    async fn request_add_stake(
        &self,
        signer: IotaAddress,
        coins: Vec<ObjectID>,
        amount: Option<BigInt<u64>>,
        validator: IotaAddress,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        let amount = amount.map(|a| *a);
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_add_stake(signer, coins, amount, validator, gas, *gas_budget)
                .await
                .map_err(|e| {
                    ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                })?,
        )
        .map_err(rpc_error_from_internal)?)
    }

    async fn request_withdraw_stake(
        &self,
        signer: IotaAddress,
        staked_iota: ObjectID,
        gas: Option<ObjectID>,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_withdraw_stake(signer, staked_iota, gas, *gas_budget)
                .await
                .map_err(|e| {
                    ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                })?,
        )
        .map_err(rpc_error_from_internal)?)
    }

    async fn request_add_timelocked_stake(
        &self,
        signer: IotaAddress,
        locked_balance: ObjectID,
        validator: IotaAddress,
        gas: ObjectID,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_add_timelocked_stake(signer, locked_balance, validator, gas, *gas_budget)
                .await
                .map_err(|e| {
                    ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, e.to_string(), None)
                })?,
        )
        .map_err(rpc_error_from_internal)?)
    }

    async fn request_withdraw_timelocked_stake(
        &self,
        signer: IotaAddress,
        timelocked_staked_iota: ObjectID,
        gas: ObjectID,
        gas_budget: BigInt<u64>,
    ) -> RpcResult<TransactionBlockBytes> {
        Ok(TransactionBlockBytes::from_data(
            self.0
                .request_withdraw_timelocked_stake(signer, timelocked_staked_iota, gas, *gas_budget)
                .await
                .map_err(rpc_error_from_internal)?,
        )
        .map_err(rpc_error_from_internal)?)
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

fn rpc_error_from_internal(err: anyhow::Error) -> ErrorObjectOwned {
    ErrorObjectOwned::owned::<()>(INTERNAL_ERROR_CODE, err.to_string(), None)
}
