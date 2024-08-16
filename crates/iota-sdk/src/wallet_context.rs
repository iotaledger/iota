// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, path::Path, sync::Arc};

use anyhow::anyhow;
use colored::Colorize;
use iota_config::{Config, PersistedConfig};
use iota_json_rpc_types::{
    IotaObjectData, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
};
use iota_keys::keystore::AccountKeystore;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    gas_coin::GasCoin,
    transaction::{Transaction, TransactionData, TransactionDataAPI},
};
use shared_crypto::intent::Intent;
use tokio::sync::RwLock;
use tracing::warn;

use crate::{iota_client_config::IotaClientConfig, IotaClient};

pub struct WalletContext {
    pub config: PersistedConfig<IotaClientConfig>,
    request_timeout: Option<std::time::Duration>,
    client: Arc<RwLock<Option<IotaClient>>>,
    max_concurrent_requests: Option<u64>,
}

impl WalletContext {
    pub fn new(
        config_path: &Path,
        request_timeout: Option<std::time::Duration>,
        max_concurrent_requests: Option<u64>,
    ) -> Result<Self, anyhow::Error> {
        let config: IotaClientConfig = PersistedConfig::read(config_path).map_err(|err| {
            anyhow!("Cannot open wallet config file at {config_path:?}. Err: {err}",)
        })?;

        let config = config.persisted(config_path);
        let context = Self {
            config,
            request_timeout,
            client: Default::default(),
            max_concurrent_requests,
        };
        Ok(context)
    }

    pub fn get_addresses(&self) -> Vec<IotaAddress> {
        self.config.keystore.addresses()
    }

    pub async fn get_client(&self) -> Result<IotaClient, anyhow::Error> {
        let read = self.client.read().await;

        Ok(if let Some(client) = read.as_ref() {
            client.clone()
        } else {
            drop(read);
            let client = self
                .config
                .get_active_env()?
                .create_rpc_client(self.request_timeout, self.max_concurrent_requests)
                .await?;
            if let Err(e) = client.check_api_version() {
                warn!("{e}");
                eprintln!("{}", format!("[warn] {e}").yellow().bold());
            }
            self.client.write().await.insert(client).clone()
        })
    }

    // TODO: Ger rid of mut
    pub fn active_address(&mut self) -> Result<IotaAddress, anyhow::Error> {
        if self.config.keystore.addresses().is_empty() {
            return Err(anyhow!(
                "No managed addresses. Create new address with `new-address` command."
            ));
        }

        // Ok to unwrap because we checked that config addresses not empty
        // Set it if not exists
        self.config.active_address = Some(
            self.config
                .active_address
                .unwrap_or(*self.config.keystore.addresses().first().unwrap()),
        );

        Ok(self.config.active_address.unwrap())
    }

    /// Get the latest object reference given a object id
    pub async fn get_object_ref(&self, object_id: ObjectID) -> Result<ObjectRef, anyhow::Error> {
        let client = self.get_client().await?;
        Ok(client
            .read_api()
            .get_object_with_options(object_id, IotaObjectDataOptions::new())
            .await?
            .into_object()?
            .object_ref())
    }

    /// Get all the gas objects (and conveniently, gas amounts) for the address
    pub async fn gas_objects(
        &self,
        address: IotaAddress,
    ) -> Result<Vec<(u64, IotaObjectData)>, anyhow::Error> {
        let client = self.get_client().await?;

        let mut objects: Vec<IotaObjectResponse> = Vec::new();
        let mut cursor = None;
        loop {
            let response = client
                .read_api()
                .get_owned_objects(
                    address,
                    Some(IotaObjectResponseQuery::new(
                        Some(IotaObjectDataFilter::StructType(GasCoin::type_())),
                        Some(IotaObjectDataOptions::full_content()),
                    )),
                    cursor,
                    None,
                )
                .await?;

            objects.extend(response.data);

            if response.has_next_page {
                cursor = response.next_cursor;
            } else {
                break;
            }
        }

        // TODO: We should ideally fetch the objects from local cache
        let mut values_objects = Vec::new();

        for object in objects {
            let o = object.data;
            if let Some(o) = o {
                let gas_coin = GasCoin::try_from(&o)?;
                values_objects.push((gas_coin.value(), o.clone()));
            }
        }

        Ok(values_objects)
    }

    pub async fn get_object_owner(&self, id: &ObjectID) -> Result<IotaAddress, anyhow::Error> {
        let client = self.get_client().await?;
        let object = client
            .read_api()
            .get_object_with_options(*id, IotaObjectDataOptions::new().with_owner())
            .await?
            .into_object()?;
        Ok(object
            .owner
            .ok_or_else(|| anyhow!("Owner field is None"))?
            .get_owner_address()?)
    }

    pub async fn try_get_object_owner(
        &self,
        id: &Option<ObjectID>,
    ) -> Result<Option<IotaAddress>, anyhow::Error> {
        if let Some(id) = id {
            Ok(Some(self.get_object_owner(id).await?))
        } else {
            Ok(None)
        }
    }

    /// Find a gas object which fits the budget
    pub async fn gas_for_owner_budget(
        &self,
        address: IotaAddress,
        budget: u64,
        forbidden_gas_objects: BTreeSet<ObjectID>,
    ) -> Result<(u64, IotaObjectData), anyhow::Error> {
        for o in self.gas_objects(address).await.unwrap() {
            if o.0 >= budget && !forbidden_gas_objects.contains(&o.1.object_id) {
                return Ok((o.0, o.1));
            }
        }
        Err(anyhow!(
            "No non-argument gas objects found for this address with value >= budget {budget}. Run iota client gas to check for gas objects."
        ))
    }

    pub async fn get_all_gas_objects_owned_by_address(
        &self,
        address: IotaAddress,
    ) -> anyhow::Result<Vec<ObjectRef>> {
        self.get_gas_objects_owned_by_address(address, None).await
    }

    pub async fn get_gas_objects_owned_by_address(
        &self,
        address: IotaAddress,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<ObjectRef>> {
        let client = self.get_client().await?;
        let results: Vec<_> = client
            .read_api()
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new(
                    Some(IotaObjectDataFilter::StructType(GasCoin::type_())),
                    Some(IotaObjectDataOptions::full_content()),
                )),
                None,
                limit,
            )
            .await?
            .data
            .into_iter()
            .filter_map(|r| r.data.map(|o| o.object_ref()))
            .collect();
        Ok(results)
    }

    /// Given an address, return one gas object owned by this address.
    /// The actual implementation just returns the first one returned by the
    /// read api.
    pub async fn get_one_gas_object_owned_by_address(
        &self,
        address: IotaAddress,
    ) -> anyhow::Result<Option<ObjectRef>> {
        Ok(self
            .get_gas_objects_owned_by_address(address, Some(1))
            .await?
            .pop())
    }

    /// Returns one address and all gas objects owned by that address.
    pub async fn get_one_account(&self) -> anyhow::Result<(IotaAddress, Vec<ObjectRef>)> {
        let address = self.get_addresses().pop().unwrap();
        Ok((
            address,
            self.get_all_gas_objects_owned_by_address(address).await?,
        ))
    }

    /// Return a gas object owned by an arbitrary address managed by the wallet.
    pub async fn get_one_gas_object(&self) -> anyhow::Result<Option<(IotaAddress, ObjectRef)>> {
        for address in self.get_addresses() {
            if let Some(gas_object) = self.get_one_gas_object_owned_by_address(address).await? {
                return Ok(Some((address, gas_object)));
            }
        }
        Ok(None)
    }

    /// Returns all the account addresses managed by the wallet and their owned
    /// gas objects.
    pub async fn get_all_accounts_and_gas_objects(
        &self,
    ) -> anyhow::Result<Vec<(IotaAddress, Vec<ObjectRef>)>> {
        let mut result = vec![];
        for address in self.get_addresses() {
            let objects = self
                .gas_objects(address)
                .await?
                .into_iter()
                .map(|(_, o)| o.object_ref())
                .collect();
            result.push((address, objects));
        }
        Ok(result)
    }

    pub async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        let client = self.get_client().await?;
        let gas_price = client.governance_api().get_reference_gas_price().await?;
        Ok(gas_price)
    }

    /// Sign a transaction with a key currently managed by the WalletContext
    pub fn sign_transaction(&self, data: &TransactionData) -> Transaction {
        let sig = self
            .config
            .keystore
            .sign_secure(&data.sender(), data, Intent::iota_transaction())
            .unwrap();
        // TODO: To support sponsored transaction, we should also look at the gas owner.
        Transaction::from_data(data.clone(), vec![sig])
    }

    /// Execute a transaction and wait for it to be locally executed on the
    /// fullnode. Also expects the effects status to be
    /// ExecutionStatus::Success.
    pub async fn execute_transaction_must_succeed(
        &self,
        tx: Transaction,
    ) -> IotaTransactionBlockResponse {
        tracing::debug!("Executing transaction: {:?}", tx);
        let response = self.execute_transaction_may_fail(tx).await.unwrap();
        assert!(
            response.status_ok().unwrap(),
            "Transaction failed: {:?}",
            response
        );
        response
    }

    /// Execute a transaction and wait for it to be locally executed on the
    /// fullnode. The transaction execution is not guaranteed to succeed and
    /// may fail. This is usually only needed in non-test environment or the
    /// caller is explicitly testing some failure behavior.
    pub async fn execute_transaction_may_fail(
        &self,
        tx: Transaction,
    ) -> anyhow::Result<IotaTransactionBlockResponse> {
        let client = self.get_client().await?;
        Ok(client
            .quorum_driver_api()
            .execute_transaction_block(
                tx,
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_input()
                    .with_events()
                    .with_object_changes()
                    .with_balance_changes(),
                Some(iota_types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?)
    }
}
