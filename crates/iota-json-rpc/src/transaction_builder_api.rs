// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use fastcrypto::encoding::Base64;
use iota_core::authority::AuthorityState;
use iota_json::IotaJsonValue;
use iota_json_rpc_api::{
    TransactionBuilderOpenRpc, TransactionBuilderServer, cap_page_limit, internal_error,
};
use iota_json_rpc_types::{
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse,
    IotaTransactionBlockBuilderMode, IotaTypeTag, OwnedObjectCursor, RPCTransactionRequestParams,
    TransactionBlockBytes,
};
use iota_metrics::spawn_monitored_blocking;
use iota_open_rpc::Module;
use iota_sdk_types::{Address, ObjectId, ObjectReference, Owner, StructTag, TypeTag};
use iota_transaction_builder::{DataReader, TransactionBuilder};
use iota_types::{gas_coin::GasCoin, iota_serde::BigInt, object::ObjectRead};
use jsonrpsee::{RpcModule, core::RpcResult};

use crate::{IotaRpcModule, authority_state::StateRead};

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

/// How many coins one read of the owner index takes while selecting gas.
const GAS_SELECTION_PAGE: usize = 8;

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
        address: Address,
        object_type: StructTag,
        cursor: Option<OwnedObjectCursor>,
        limit: Option<usize>,
        options: IotaObjectDataOptions,
    ) -> Result<iota_json_rpc_types::ObjectsPage, anyhow::Error> {
        let limit = cap_page_limit(limit);
        // This reader serves the node's own transaction builder, so a cursor
        // that cannot name a position in the node's index is a bug rather than
        // a request to refuse.
        let cursor = cursor
            .map(|cursor| {
                cursor
                    .position()
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("cursor does not name a position in the index"))
            })
            .transpose()?;
        let page = self.0.get_owner_objects_page(
            address,
            cursor,
            limit,
            Some(IotaObjectDataFilter::StructType(object_type)),
        )?;

        let data = page
            .data
            .into_iter()
            .map(|info| {
                let read = self.0.get_object_read(&info.object_id)?;
                IotaObjectResponse::try_from_object_read_and_options(read, &options)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(iota_json_rpc_types::ObjectsPage {
            data,
            next_cursor: page.next_cursor,
            has_next_page: page.has_next_page,
            oldest_available_checkpoint: None,
        })
    }

    async fn get_object_with_options(
        &self,
        object_id: ObjectId,
        options: IotaObjectDataOptions,
    ) -> Result<IotaObjectResponse, anyhow::Error> {
        let result = self.0.get_object_read(&object_id)?;
        IotaObjectResponse::try_from_object_read_and_options(result, &options)
    }

    async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        let epoch_store = self.0.load_epoch_store_one_call_per_task();
        Ok(epoch_store.reference_gas_price())
    }

    async fn select_gas_coin(
        &self,
        signer: Address,
        gas_budget: u64,
        input_objects: &[ObjectId],
    ) -> Result<Option<ObjectReference>, anyhow::Error> {
        let state = self.0.clone();
        let input_objects = input_objects.to_vec();
        spawn_monitored_blocking!(move || {
            select_gas_coin_by_balance(state.as_ref(), signer, gas_budget, &input_objects)
        })
        .await?
    }
}

/// The first of `signer`'s `Coin<IOTA>` objects, largest balance first, that
/// is not one of `input_objects`, if it holds at least `gas_budget`.
///
/// The owner index lists a type's coins by balance, so no coin after the
/// first one that is not an input holds more, and the read stops there. The
/// index follows checkpoints, so a coin's current state decides, not its row.
fn select_gas_coin_by_balance(
    state: &dyn StateRead,
    signer: Address,
    gas_budget: u64,
    input_objects: &[ObjectId],
) -> Result<Option<ObjectReference>, anyhow::Error> {
    let mut cursor = None;
    loop {
        let coins = state.get_owned_coins(
            signer,
            cursor,
            Some(TypeTag::from(StructTag::new_gas())),
            GAS_SELECTION_PAGE,
        )?;
        for (coin, _) in &coins {
            if coin.balance < gas_budget {
                return Ok(None);
            }
            if input_objects.contains(&coin.coin_object_id) {
                continue;
            }
            if let ObjectRead::Exists(object_ref, object, _) =
                state.get_object_read(&coin.coin_object_id)?
            {
                if object.owner == Owner::Address(signer)
                    && GasCoin::try_from(&object).is_ok_and(|gas| gas.value() >= gas_budget)
                {
                    return Ok(Some(object_ref));
                }
            }
        }
        if coins.len() < GAS_SELECTION_PAGE {
            return Ok(None);
        }
        let Some((_, last)) = coins.last() else {
            return Ok(None);
        };
        cursor = Some(
            *last
                .position()
                .ok_or_else(|| anyhow::anyhow!("cursor does not name a position in the index"))?,
        );
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iota_json_rpc_types::Coin;
    use iota_types::{object::Object, storage::OwnedObjectCursor as IndexCursor};

    use super::*;
    use crate::authority_state::MockStateRead;

    /// A `Coin<IOTA>` of `owner` holding `balance`.
    fn gas_coin(owner: Address, balance: u64) -> Object {
        Object::with_id_owner_gas_for_testing(ObjectId::random(), owner, balance)
    }

    /// The owner index's row of `object`, holding `balance` as of its last
    /// checkpoint.
    fn row(object: &Object, balance: u64) -> (Coin, OwnedObjectCursor) {
        let object_ref = object.object_ref();
        let coin = Coin {
            coin_type: StructTag::new_gas().to_string(),
            coin_object_id: object_ref.object_id,
            version: object_ref.version,
            digest: object_ref.digest,
            balance,
            previous_transaction: object.previous_transaction,
        };
        let cursor = OwnedObjectCursor::from_position(IndexCursor {
            object_type_identifier: 0,
            object_type_params: 0,
            inverted_balance: None,
            object_id: object_ref.object_id,
        });
        (coin, cursor)
    }

    /// A node whose owner index lists `rows`, largest balance first, a page at
    /// a time, and whose live objects are `objects`. Expects exactly `pages`
    /// index reads and `object_reads` object reads.
    fn node(
        rows: Vec<(Coin, OwnedObjectCursor)>,
        objects: Vec<Object>,
        pages: usize,
        object_reads: usize,
    ) -> MockStateRead {
        let mut state = MockStateRead::new();
        state
            .expect_get_owned_coins()
            .times(pages)
            .returning(move |_, cursor, _, limit| {
                let start = cursor.map_or(0, |cursor| {
                    rows.iter()
                        .position(|(coin, _)| coin.coin_object_id == cursor.object_id)
                        .unwrap()
                        + 1
                });
                Ok(rows.iter().skip(start).take(limit).cloned().collect())
            });
        let objects: HashMap<ObjectId, Object> = objects
            .into_iter()
            .map(|object| (object.id(), object))
            .collect();
        state
            .expect_get_object_read()
            .times(object_reads)
            .returning(move |id| {
                let object = objects[id].clone();
                Ok(ObjectRead::Exists(object.object_ref(), object, None))
            });
        state
    }

    /// Dust coins after a coin that covers the budget are never read.
    #[test]
    fn the_largest_coin_is_selected_without_reading_the_rest() {
        let signer = Address::random();
        let rich = gas_coin(signer, 1_000);
        let mut rows = vec![row(&rich, 1_000)];
        rows.extend((0..100).map(|_| row(&gas_coin(signer, 1), 1)));
        let state = node(rows, vec![rich.clone()], 1, 1);

        let selected = select_gas_coin_by_balance(&state, signer, 500, &[]).unwrap();
        assert_eq!(selected, Some(rich.object_ref()));
    }

    /// When the largest coin does not cover the budget, no other coin is read.
    #[test]
    fn no_coin_is_selected_when_the_largest_falls_short() {
        let signer = Address::random();
        let rows = (0..100).map(|_| row(&gas_coin(signer, 10), 10)).collect();
        let state = node(rows, vec![], 1, 0);

        assert_eq!(
            select_gas_coin_by_balance(&state, signer, 500, &[]).unwrap(),
            None
        );
    }

    /// The transaction's own inputs are passed over, across index pages.
    #[test]
    fn the_inputs_are_passed_over() {
        let signer = Address::random();
        let coins: Vec<Object> = (0..10).map(|_| gas_coin(signer, 1_000)).collect();
        let rows = coins.iter().map(|coin| row(coin, 1_000)).collect();
        let inputs: Vec<ObjectId> = coins[..9].iter().map(|coin| coin.id()).collect();
        let state = node(rows, coins.clone(), 2, 1);

        let selected = select_gas_coin_by_balance(&state, signer, 500, &inputs).unwrap();
        assert_eq!(selected, Some(coins[9].object_ref()));
    }

    /// A coin whose current balance no longer covers the budget, or that has
    /// changed hands since its row was written, is passed over.
    #[test]
    fn the_current_coin_decides_not_its_row() {
        let signer = Address::random();
        let spent = gas_coin(signer, 10);
        let given_away = gas_coin(Address::random(), 1_000);
        let fitting = gas_coin(signer, 600);
        let rows = vec![
            row(&spent, 1_000),
            row(&given_away, 900),
            row(&fitting, 600),
        ];
        let state = node(rows, vec![spent, given_away, fitting.clone()], 1, 3);

        let selected = select_gas_coin_by_balance(&state, signer, 500, &[]).unwrap();
        assert_eq!(selected, Some(fitting.object_ref()));
    }
}
