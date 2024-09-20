// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::time::Duration;
use std::{collections::HashSet, sync::Arc};

use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockEffects, IotaTransactionBlockEffectsV1,
    IotaTransactionBlockResponseOptions, IotaTypeTag,
};
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_move_build::BuildConfig;
use iota_sdk::IotaClient;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    digests::TransactionDigest,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{ProgrammableTransaction, Transaction, TransactionData},
};
use shared_crypto::intent::Intent;
use tokio::sync::RwLock;
use tracing::{instrument, trace};

use crate::{
    client::{
        builders::{
            call_move::MoveCallBuilder, merge_coin::MergeCoinBuilder, pay::PaymentBuilder,
            publish::PublishBuilder, split_coin::SplitCoinBuilder,
            transfer_object::TransferObjectBuilder,
        },
        error::{ClientError, ClientResult},
        publish_type::PublishType,
        response::{Published, TransactionResponse},
    },
    types::GasCoin,
};

/// A client which wraps a [`IotaClient`] and provides a better interface.
#[derive(Clone, Debug)]
pub struct Client {
    data: Arc<RwLock<ClientData>>,
}

/// Client shared data.
pub struct ClientData {
    /// The inner IOTA client.
    pub inner: IotaClient,
    /// The keystore used for signing transactions.
    pub keystore: Option<Keystore>,
    /// The current address upon which all operations will occur.
    pub address: IotaAddress,
}

impl core::fmt::Debug for ClientData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("ClientData");
        s.field("address", &self.address);
        if let Some(keystore) = &self.keystore {
            s.field("keystore", &format!("{}", keystore));
        }
        s.finish()
    }
}

impl Client {
    /// Create a new client from the inner type and a sender address.
    pub fn new(inner: IotaClient, address: IotaAddress) -> ClientResult<Self> {
        Ok(Self {
            data: Arc::new(RwLock::new(ClientData {
                inner,
                keystore: None,
                address,
            })),
        })
    }

    /// Get the inner client data.
    pub async fn data(&self) -> impl core::ops::Deref<Target = ClientData> + '_ {
        self.data.read().await
    }

    /// Get the mutable inner client data.
    pub async fn data_mut(&self) -> impl core::ops::DerefMut<Target = ClientData> + '_ {
        self.data.write().await
    }

    /// Change the current sender address.
    pub async fn switch_address(&self, address: IotaAddress) {
        self.data_mut().await.address = address;
    }

    /// Set the keystore, which will be used to sign transactions.
    pub async fn set_keystore(&self, keystore: impl Into<Option<Keystore>> + Send) {
        self.data_mut().await.keystore = keystore.into();
    }

    /// Publish a Move module at the given path.
    pub fn publish(&self, kind: impl Into<PublishType> + Send) -> PublishBuilder {
        PublishBuilder::new(self, kind)
    }

    /// Build a move call.
    pub fn call_move(&self, package_id: ObjectID, module: &str, function: &str) -> MoveCallBuilder {
        MoveCallBuilder::new(self, package_id, module, function)
    }

    /// Send coins to other addresses.
    pub fn pay(&self, coins: impl IntoIterator<Item = ObjectID> + Send) -> PaymentBuilder {
        PaymentBuilder::new(self, coins)
    }

    /// Transfer an object by its ID to a recipient address.
    pub fn transfer_object(
        &self,
        object_id: ObjectID,
        recipient: IotaAddress,
    ) -> TransferObjectBuilder {
        TransferObjectBuilder::new(self, object_id, recipient)
    }

    /// Split a coin by its ID into the given amounts. A new coin will be
    /// created for each amount if possible, leaving with the remainder in
    /// the original coin.
    pub fn split_coin(
        &self,
        coin: ObjectID,
        split_amounts: impl IntoIterator<Item = u64> + Send,
    ) -> SplitCoinBuilder {
        SplitCoinBuilder::new(self, coin, split_amounts)
    }

    /// Merge two coins together into the primary coin.
    pub fn merge_coins(&self, primary_coin: ObjectID, consumed_coin: ObjectID) -> MergeCoinBuilder {
        MergeCoinBuilder::new(self, primary_coin, consumed_coin)
    }

    /// Execute a programmable transaction.
    pub async fn execute_programmable_txn(
        &self,
        txn: ProgrammableTransaction,
        gas_budget: impl Into<Option<u64>> + Send,
        gas: impl Into<Option<GasCoin>> + Send,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        self.data_mut()
            .await
            .execute_programmable_txn(txn, gas_budget.into(), gas.into(), wait_for_finalization)
            .await
    }
}

impl ClientData {
    /// Execute a programmable transaction.
    #[instrument(skip(self, txn), err, level = "trace")]
    pub async fn execute_programmable_txn(
        &mut self,
        txn: ProgrammableTransaction,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        trace!("{txn}");
        let gas_price = self.read_api().get_reference_gas_price().await?;
        let input_objects = txn
            .input_objects()?
            .into_iter()
            .map(|i| i.object_id())
            .collect::<HashSet<_>>();
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let data = TransactionData::new_programmable(
            self.address,
            gas.into_iter()
                .filter_map(|c| (!input_objects.contains(&c.object_id)).then_some(c.object_ref()))
                .collect(),
            txn,
            gas_budget,
            gas_price,
        );

        self.execute(data, wait_for_finalization).await
    }

    /// Call a move function.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn call_move(
        &mut self,
        package_id: ObjectID,
        module: &str,
        function: &str,
        type_args: Vec<IotaTypeTag>,
        call_args: Vec<IotaJsonValue>,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let data = self
            .transaction_builder()
            .move_call(
                self.address,
                package_id,
                module,
                function,
                type_args,
                call_args,
                gas.map(|g| g.object_id),
                gas_budget,
                None,
            )
            .await?;

        self.execute(data, wait_for_finalization).await
    }

    /// Publish a move package.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn publish(
        &mut self,
        kind: PublishType,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<(Published, TransactionResponse)> {
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let module = match kind {
            PublishType::Path(path) => BuildConfig::default().build(path)?,
            PublishType::Compiled(m) => m,
        };
        let data = self
            .transaction_builder()
            .publish(
                self.address,
                module.get_package_bytes(false),
                module.published_dependency_ids(),
                gas.map(|g| g.object_id),
                gas_budget,
            )
            .await?;

        let res = self.execute(data, wait_for_finalization).await?;

        Ok((res.published[0].clone(), res))
    }

    /// Pay coins to a list of addresses.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn pay(
        &mut self,
        coins: Vec<ObjectID>,
        addresses: Vec<IotaAddress>,
        amounts: Vec<u64>,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let data = self
            .transaction_builder()
            .pay(
                self.address,
                coins,
                addresses,
                amounts,
                gas.map(|g| g.object_id),
                gas_budget,
            )
            .await?;

        self.execute(data, wait_for_finalization).await
    }

    /// Transfer an object.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn transfer_object(
        &mut self,
        object_id: ObjectID,
        recipient: IotaAddress,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let data = self
            .transaction_builder()
            .transfer_object(
                self.address,
                object_id,
                gas.map(|g| g.object_id),
                gas_budget,
                recipient,
            )
            .await?;

        self.execute(data, wait_for_finalization).await
    }

    /// Merge two coins into the first.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn merge_coins(
        &mut self,
        primary_coin: ObjectID,
        consumed_coin: ObjectID,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let data = self
            .transaction_builder()
            .merge_coins(
                self.address,
                primary_coin,
                consumed_coin,
                gas.map(|g| g.object_id),
                gas_budget,
            )
            .await?;

        self.execute(data, wait_for_finalization).await
    }

    /// Split a coin into many.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn split_coin(
        &mut self,
        coin: ObjectID,
        split_amounts: Vec<u64>,
        gas_budget: Option<u64>,
        mut gas: Option<GasCoin>,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        let gas_budget = self.resolve_gas_budget(gas_budget, &mut gas).await?;
        let data = self
            .transaction_builder()
            .split_coin(
                self.address,
                coin,
                split_amounts,
                gas.map(|g| g.object_id),
                gas_budget,
            )
            .await?;

        self.execute(data, wait_for_finalization).await
    }

    async fn resolve_gas_budget(
        &self,
        gas_budget: Option<u64>,
        gas: &mut Option<GasCoin>,
    ) -> ClientResult<u64> {
        if gas.is_none() {
            *gas = self
                .read_gas_coins()
                .await?
                .into_iter()
                .max_by_key(|g| g.coin.value());
        }
        let max_tx_gas =
            self.read_api().get_protocol_config(None).await?.attributes["max_tx_gas"].as_u64();
        let budget = gas_budget
            .unwrap_or_else(|| gas.iter().map(|g| g.coin.value()).sum())
            .min(max_tx_gas);
        trace!("Gas Budget: {budget}");
        Ok(budget)
    }

    /// Get the available gas coins owned by the current address.
    #[instrument(skip(self), err, level = "trace")]
    pub async fn read_gas_coins(&self) -> ClientResult<Vec<GasCoin>> {
        let gas_objs = self
            .read_api()
            .get_owned_objects(
                self.address,
                Some(IotaObjectResponseQuery::new(
                    Some(IotaObjectDataFilter::gas_coin()),
                    Some(IotaObjectDataOptions::bcs_lossless()),
                )),
                None,
                None,
            )
            .await?;

        gas_objs.data.into_iter().map(GasCoin::try_from).collect()
    }

    #[instrument(skip(self), err, level = "trace")]
    async fn wait_for_finalization(&self, digest: TransactionDigest) -> ClientResult<u64> {
        const TIMEOUT: Duration = Duration::from_secs(10);
        let read_txn = || async {
            loop {
                let res = self
                    .read_api()
                    .get_transaction_with_options(
                        digest,
                        IotaTransactionBlockResponseOptions::new(),
                    )
                    .await?;
                if let Some(checkpoint) = res.checkpoint {
                    break Ok(checkpoint);
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        tokio::select! {
            res = read_txn() => res,
            _ = tokio::time::sleep(TIMEOUT) => Err(ClientError::Timeout(format!("waiting for transaction {digest} to finalize")))
        }
    }

    /// Execute a transaction.
    #[instrument(skip_all, err, level = "trace")]
    pub async fn execute(
        &mut self,
        data: TransactionData,
        wait_for_finalization: bool,
    ) -> ClientResult<TransactionResponse> {
        if self.keystore.is_none() {
            self.keystore = Some(crate::default_keystore()?);
        };
        let signature = self.keystore.as_ref().unwrap().sign_secure(
            &self.address,
            &data,
            Intent::iota_transaction(),
        )?;

        let res = self
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(data, vec![signature]),
                IotaTransactionBlockResponseOptions::full_content(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;

        tracing::debug!("{res}");

        if !res.errors.is_empty() {
            return Err(ClientError::Execution(res.errors));
        }

        if let Some(IotaTransactionBlockEffects::V1(IotaTransactionBlockEffectsV1 {
            status: IotaExecutionStatus::Failure { error },
            ..
        })) = res.effects
        {
            return Err(ClientError::TransactionFailure(error));
        }

        let res = TransactionResponse::try_from(res)?;

        if wait_for_finalization {
            self.wait_for_finalization(res.digest).await?;
        }

        Ok(res)
    }
}

impl core::ops::Deref for ClientData {
    type Target = IotaClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
