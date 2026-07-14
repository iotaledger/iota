// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, path::Path, sync::Arc};

use anyhow::{anyhow, bail, ensure};
use colored::Colorize;
use futures::{StreamExt, TryStreamExt, future};
use getset::{Getters, MutGetters};
use iota_config::{Config, PersistedConfig};
use iota_grpc_client::{ReadMask, read_mask_fields::ObjectField};
use iota_grpc_types::v1::transaction::ExecutedTransaction;
use iota_json_rpc_types::{
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockResponseOptions,
};
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_sdk_types::{Address, Coin, ObjectId, ObjectReference, StructTag, crypto::Intent};
use iota_types::{
    crypto::IotaKeyPair,
    effects::TransactionEffectsAPI,
    transaction::{Transaction, TransactionData, TransactionDataAPI},
};
use tokio::sync::RwLock;
use tracing::warn;

use crate::{
    IotaClient, PagedFn,
    iota_client_config::{IotaClientConfig, IotaEnv},
};

/// Read mask for `execute_transaction`: everything `WalletContext`'s current
/// consumers read off an executed transaction.
const EXECUTE_TRANSACTION_READ_MASK: &str = iota_grpc_types::field_mask!(
    "transaction.digest",
    "effects",
    "events",
    "object_changes",
    "balance_changes",
    "checkpoint",
    "timestamp",
);

/// How long the server waits for a submitted transaction to land in a
/// checkpoint before `execute_transaction` returns.
const CHECKPOINT_INCLUSION_TIMEOUT_MS: u64 = 60_000;

/// Which transport `WalletContext` uses for chain-touching operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WalletBackend {
    /// Use the node's gRPC API. The default; requires the active environment
    /// to have a `grpc` URL configured (chain-touching calls error otherwise).
    #[default]
    Grpc,
    /// Use the node's JSON-RPC API.
    JsonRpc,
}

/// Wallet for managing accounts, objects, and interact with client APIs.
// Mainly used in the CLI and tests.
#[derive(Getters, MutGetters)]
#[getset(get = "pub", get_mut = "pub")]
pub struct WalletContext {
    config: PersistedConfig<IotaClientConfig>,
    request_timeout: Option<std::time::Duration>,
    client: Arc<RwLock<Option<IotaClient>>>,
    grpc_client: Arc<RwLock<Option<iota_grpc_client::Client>>>,
    max_concurrent_requests: Option<u64>,
    env_override: Option<String>,
    backend: WalletBackend,
}

impl WalletContext {
    /// Create a new [`WalletContext`] with the config path to an existing
    /// [`IotaClientConfig`] and optional parameters for the client.
    pub fn new(config_path: &Path) -> Result<Self, anyhow::Error> {
        let config: IotaClientConfig = PersistedConfig::read(config_path).map_err(|err| {
            anyhow!("Cannot open wallet config file at {config_path:?}. Err: {err}")
        })?;

        if let Some(active_address) = &config.active_address {
            let addresses = match &config.keystore {
                Keystore::File(file) => file.addresses(),
                Keystore::InMem(mem) => mem.addresses(),
            };
            ensure!(
                addresses.contains(active_address),
                "error in '{}': active address not found in the keystore",
                config_path.display()
            );
        }

        if let Some(active_env) = &config.active_env {
            ensure!(
                config.get_env(active_env).is_some(),
                "error in '{}': active environment not found in the envs list",
                config_path.display()
            );
        }

        let config = config.persisted(config_path);
        let context = Self {
            config,
            request_timeout: None,
            client: Default::default(),
            grpc_client: Default::default(),
            max_concurrent_requests: None,
            env_override: None,
            backend: WalletBackend::default(),
        };
        Ok(context)
    }

    /// Set the request timeout for chain-touching calls.
    ///
    /// This currently only affects the JSON-RPC backend. The gRPC backend's
    /// `iota_grpc_client::Client` does not yet expose a request-timeout setter,
    /// so this value is ignored there until that lands upstream.
    pub fn with_request_timeout(mut self, request_timeout: std::time::Duration) -> Self {
        self.request_timeout = Some(request_timeout);
        self
    }

    pub fn with_max_concurrent_requests(mut self, max_concurrent_requests: u64) -> Self {
        self.max_concurrent_requests = Some(max_concurrent_requests);
        self
    }

    pub fn with_env_override(mut self, env_override: String) -> Self {
        self.env_override = Some(env_override);
        self
    }

    /// Force `WalletContext` to use the JSON-RPC backend instead of the
    /// default gRPC one.
    pub fn with_jsonrpc_backend(mut self) -> Self {
        self.backend = WalletBackend::JsonRpc;
        self
    }

    /// Get all addresses from the keystore.
    pub fn get_addresses(&self) -> Vec<Address> {
        self.config.keystore.addresses()
    }

    pub fn get_env_override(&self) -> Option<String> {
        self.env_override.clone()
    }

    /// Get the configured [`IotaClient`].
    pub async fn get_client(&self) -> Result<IotaClient, anyhow::Error> {
        let read = self.client.read().await;

        Ok(if let Some(client) = read.as_ref() {
            client.clone()
        } else {
            drop(read);
            let client = self
                .active_env()?
                .create_rpc_client(self.request_timeout, self.max_concurrent_requests)
                .await?;
            if let Err(e) = client.check_api_version() {
                warn!("{e}");
                eprintln!("{}", format!("[warn] {e}").yellow().bold());
            }
            self.client.write().await.insert(client).clone()
        })
    }

    /// Get the configured gRPC client, creating and caching it on first use.
    /// Errors if the active env has no `grpc` URL configured.
    pub async fn get_grpc_client(&self) -> Result<iota_grpc_client::Client, anyhow::Error> {
        let read = self.grpc_client.read().await;

        Ok(if let Some(client) = read.as_ref() {
            client.clone()
        } else {
            drop(read);
            let client = self.active_env()?.create_grpc_client()?;
            self.grpc_client.write().await.insert(client).clone()
        })
    }

    /// Get the active [`Address`].
    /// If not set, defaults to the first address in the keystore.
    pub fn active_address(&self) -> Result<Address, anyhow::Error> {
        if self.config.keystore.addresses().is_empty() {
            bail!("No managed addresses. Create new address with the `new-address` command.");
        }

        Ok(if let Some(addr) = self.config.active_address() {
            *addr
        } else {
            self.config.keystore().addresses()[0]
        })
    }

    /// Get the active [`IotaEnv`].
    /// If not set, defaults to the first environment in the config.
    pub fn active_env(&self) -> Result<&IotaEnv, anyhow::Error> {
        if self.config.envs.is_empty() {
            bail!("No managed environments. Create new environment with the `new-env` command.");
        }

        if let Some(env_override) = &self.env_override {
            self.config.get_env(env_override).ok_or_else(|| {
                anyhow!("Environment configuration not found for env [{env_override}]")
            })
        } else {
            Ok(if self.config.active_env().is_some() {
                self.config.get_active_env()?
            } else {
                &self.config.envs()[0]
            })
        }
    }

    /// Get the latest object reference given a object id.
    pub async fn get_object_ref(
        &self,
        object_id: ObjectId,
    ) -> Result<ObjectReference, anyhow::Error> {
        match self.backend {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                let objects = client
                    .get_objects(
                        &[(object_id, None)],
                        Some(ReadMask::from(ObjectField::REFERENCE)),
                    )
                    .await?
                    .into_inner();
                let object = objects
                    .first()
                    .ok_or_else(|| anyhow!("object {object_id} not found"))?;
                Ok(object.object_reference()?)
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;
                Ok(client
                    .read_api()
                    .get_object_with_options(object_id, IotaObjectDataOptions::new())
                    .await?
                    .into_object()?
                    .object_ref())
            }
        }
    }

    /// Get all the gas objects (and conveniently, gas amounts) for the address.
    pub async fn gas_objects(
        &self,
        address: Address,
    ) -> Result<Vec<(u64, iota_sdk_types::Object)>, anyhow::Error> {
        match self.backend {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                let objects = client
                    .list_owned_objects(address, Some(StructTag::new_gas_coin()), None, None, None)
                    .collect(None)
                    .await?
                    .into_inner();
                objects
                    .iter()
                    .map(|o| {
                        let object = o.object()?;
                        let coin = Coin::try_from_object(&object).map_err(|e| anyhow!("{e}"))?;
                        Ok((coin.balance(), object))
                    })
                    .collect()
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;

                let values_objects = PagedFn::stream(async |cursor| {
                    client
                        .read_api()
                        .get_owned_objects(
                            address,
                            IotaObjectResponseQuery::new(
                                Some(IotaObjectDataFilter::StructType(StructTag::new_gas_coin())),
                                Some(IotaObjectDataOptions::full_content().with_bcs()),
                            ),
                            cursor,
                            None,
                        )
                        .await
                })
                .filter_map(|res| async {
                    match res {
                        Ok(res) => res.data.map(|o| {
                            let object =
                                iota_sdk_types::Object::try_from(&o).map_err(|e| anyhow!("{e}"))?;
                            let coin =
                                Coin::try_from_object(&object).map_err(|e| anyhow!("{e}"))?;
                            Ok((coin.balance(), object))
                        }),
                        Err(e) => Some(Err(anyhow!("{e}"))),
                    }
                })
                .try_collect::<Vec<_>>()
                .await?;

                Ok(values_objects)
            }
        }
    }

    /// Get the address that owns the object of the provided [`ObjectId`].
    pub async fn get_object_owner(&self, id: &ObjectId) -> Result<Address, anyhow::Error> {
        match self.backend {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                let objects = client
                    .get_objects(&[(*id, None)], Some(ReadMask::from(ObjectField::BCS)))
                    .await?
                    .into_inner();
                let object = objects
                    .first()
                    .ok_or_else(|| anyhow!("object {id} not found"))?
                    .object()?;
                Ok(*object
                    .owner()
                    .address_or_object()
                    .ok_or_else(|| anyhow!("not an address or object owner"))?)
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;
                let object = client
                    .read_api()
                    .get_object_with_options(*id, IotaObjectDataOptions::new().with_owner())
                    .await?
                    .into_object()?;
                Ok(*object
                    .owner
                    .ok_or_else(|| anyhow!("Owner field is None"))?
                    .address_or_object()
                    .ok_or_else(|| anyhow::anyhow!("not an address or object owner"))?)
            }
        }
    }

    /// Get the address that owns the object, if an [`ObjectId`] is provided.
    pub async fn try_get_object_owner(
        &self,
        id: &Option<ObjectId>,
    ) -> Result<Option<Address>, anyhow::Error> {
        if let Some(id) = id {
            Ok(Some(self.get_object_owner(id).await?))
        } else {
            Ok(None)
        }
    }

    /// Infer the sender of a transaction based on the gas objects provided. If
    /// no gas objects are provided, assume the active address is the
    /// sender.
    pub async fn infer_sender(&mut self, gas: &[ObjectId]) -> Result<Address, anyhow::Error> {
        if gas.is_empty() {
            return self.active_address();
        }

        // Find the owners of all supplied object IDs
        let owners = future::try_join_all(gas.iter().map(|id| self.get_object_owner(id))).await?;

        // SAFETY `gas` is non-empty.
        let owner = owners[0];

        ensure!(
            owners.iter().all(|o| o == &owner),
            "Cannot infer sender, not all gas objects have the same owner."
        );

        Ok(owner)
    }

    /// Find a gas object which fits the budget.
    pub async fn gas_for_owner_budget(
        &self,
        address: Address,
        budget: u64,
        forbidden_gas_objects: BTreeSet<ObjectId>,
    ) -> Result<(u64, iota_sdk_types::Object), anyhow::Error> {
        for o in self.gas_objects(address).await? {
            if o.0 >= budget && !forbidden_gas_objects.contains(&o.1.id()) {
                return Ok((o.0, o.1));
            }
        }
        bail!(
            "No non-argument gas objects found for this address with value >= budget {budget}. Run iota client gas to check for gas objects."
        )
    }

    /// Get the [`ObjectReference`] for gas objects owned by the provided
    /// address. Maximum is RPC_QUERY_MAX_RESULT_LIMIT (50 by default).
    pub async fn get_all_gas_objects_owned_by_address(
        &self,
        address: Address,
    ) -> anyhow::Result<Vec<ObjectReference>> {
        self.get_gas_objects_owned_by_address(address, None).await
    }

    /// Get a limited amount of [`ObjectReference`]s for gas objects owned by
    /// the provided address. Max limit is RPC_QUERY_MAX_RESULT_LIMIT (50 by
    /// default).
    pub async fn get_gas_objects_owned_by_address(
        &self,
        address: Address,
        limit: impl Into<Option<usize>>,
    ) -> anyhow::Result<Vec<ObjectReference>> {
        let limit = limit.into();
        match self.backend {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                let objects = client
                    .list_owned_objects(address, Some(StructTag::new_gas_coin()), None, None, None)
                    .collect(limit.map(|l| l as u32))
                    .await?
                    .into_inner();
                objects
                    .iter()
                    .map(|o| o.object_reference().map_err(|e| anyhow!("{e}")))
                    .collect()
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;
                let results: Vec<_> = client
                    .read_api()
                    .get_owned_objects(
                        address,
                        IotaObjectResponseQuery::new(
                            Some(IotaObjectDataFilter::StructType(StructTag::new_gas_coin())),
                            Some(IotaObjectDataOptions::full_content()),
                        ),
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
        }
    }

    /// Given an address, return one gas object owned by this address.
    /// The actual implementation just returns the first one returned by the
    /// read api.
    pub async fn get_one_gas_object_owned_by_address(
        &self,
        address: Address,
    ) -> anyhow::Result<Option<ObjectReference>> {
        Ok(self
            .get_gas_objects_owned_by_address(address, 1)
            .await?
            .pop())
    }

    /// Return one address and all gas objects owned by that address.
    pub async fn get_one_account(&self) -> anyhow::Result<(Address, Vec<ObjectReference>)> {
        let address = self.get_addresses().pop().unwrap();
        Ok((
            address,
            self.get_all_gas_objects_owned_by_address(address).await?,
        ))
    }

    /// Return a gas object owned by an arbitrary address managed by the wallet.
    pub async fn get_one_gas_object(&self) -> anyhow::Result<Option<(Address, ObjectReference)>> {
        for address in self.get_addresses() {
            if let Some(gas_object) = self.get_one_gas_object_owned_by_address(address).await? {
                return Ok(Some((address, gas_object)));
            }
        }
        Ok(None)
    }

    /// Return all the account addresses managed by the wallet and their owned
    /// gas objects.
    pub async fn get_all_accounts_and_gas_objects(
        &self,
    ) -> anyhow::Result<Vec<(Address, Vec<ObjectReference>)>> {
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
        match self.backend {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                Ok(client.get_reference_gas_price().await?.into_inner())
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;
                Ok(client.governance_api().get_reference_gas_price().await?)
            }
        }
    }

    /// Add an account.
    pub fn add_account(&mut self, alias: impl Into<Option<String>>, keypair: IotaKeyPair) {
        self.config.keystore.add_key(alias.into(), keypair).unwrap();
    }

    /// Sign a transaction with a key currently managed by the WalletContext.
    pub fn sign_transaction(&self, data: &TransactionData) -> Transaction {
        let sig = self
            .config
            .keystore
            .sign_secure(&data.sender(), data, Intent::iota_transaction())
            .unwrap();
        // TODO: To support sponsored transaction, we should also look at the gas owner.
        Transaction::from_data(data.clone(), vec![sig])
    }

    /// Execute a transaction, wait for the fullnode to observe it, and assert
    /// the effects status is `ExecutionStatus::Success`. The gRPC backend waits
    /// for the transaction to be included in a checkpoint; the JSON-RPC backend
    /// waits for local execution.
    pub async fn execute_transaction_must_succeed(&self, tx: Transaction) -> ExecutedTransaction {
        tracing::debug!("Executing transaction: {:?}", tx);
        let response = self.execute_transaction_may_fail(tx).await.unwrap();
        let status_ok = response
            .effects()
            .expect("effects missing from execute_transaction response")
            .effects()
            .expect("effects failed to deserialize")
            .status()
            .is_success();
        assert!(status_ok, "Transaction failed: {response:?}");
        response
    }

    /// Execute a transaction and wait for the fullnode to observe it
    /// (checkpoint inclusion on the gRPC backend, local execution on the
    /// JSON-RPC backend). The transaction execution is not guaranteed to
    /// succeed and may fail. This is usually only needed in non-test
    /// environment or the caller is explicitly testing some failure
    /// behavior.
    pub async fn execute_transaction_may_fail(
        &self,
        tx: Transaction,
    ) -> anyhow::Result<ExecutedTransaction> {
        match self.backend {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                let signed_transaction: iota_sdk_types::SignedTransaction = tx.try_into().map_err(
                    |e: iota_types::iota_sdk_types_conversions::SdkTypeConversionError| {
                        anyhow!("{e}")
                    },
                )?;
                Ok(client
                    .execute_transaction(
                        signed_transaction,
                        Some(ReadMask::from(EXECUTE_TRANSACTION_READ_MASK)),
                        Some(CHECKPOINT_INCLUSION_TIMEOUT_MS),
                    )
                    .await?
                    .into_inner())
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;
                let response = client
                    .quorum_driver_api()
                    .execute_transaction_block(
                        tx,
                        IotaTransactionBlockResponseOptions::new()
                            .with_raw_input()
                            .with_events()
                            .with_object_changes()
                            .with_balance_changes()
                            .with_raw_effects(),
                        iota_types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution,
                    )
                    .await?;
                ExecutedTransaction::try_from(&response).map_err(|e| anyhow!("{e}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_config::Config;
    use iota_keys::keystore::InMemKeystore;

    use super::*;
    use crate::iota_client_config::IotaClientConfig;

    /// Builds a `WalletContext` with a single active env, backed by an
    /// in-memory keystore and a config that is never written to disk (tests
    /// only exercise in-memory dispatch logic, never `PersistedConfig::save`).
    fn wallet_context_with_env(env: IotaEnv) -> WalletContext {
        let alias = env.alias().clone();
        let config = IotaClientConfig::new(Keystore::InMem(InMemKeystore::default()))
            .with_envs([env])
            .with_active_env(alias)
            .persisted(&std::env::temp_dir().join("iota-wallet-context-test.yaml"));
        WalletContext {
            config,
            request_timeout: None,
            client: Default::default(),
            grpc_client: Default::default(),
            max_concurrent_requests: None,
            env_override: None,
            backend: WalletBackend::default(),
        }
    }

    #[test]
    fn defaults_to_grpc_backend() {
        let ctx = wallet_context_with_env(IotaEnv::new("test", "https://rpc.example"));
        assert_eq!(ctx.backend, WalletBackend::Grpc);
    }

    #[test]
    fn with_jsonrpc_backend_selects_json_rpc() {
        let ctx = wallet_context_with_env(IotaEnv::new("test", "https://rpc.example"))
            .with_jsonrpc_backend();
        assert_eq!(ctx.backend, WalletBackend::JsonRpc);
    }

    /// The default gRPC backend errors loudly when the active env has no `grpc`
    /// URL, rather than silently falling back to JSON-RPC.
    #[tokio::test]
    async fn grpc_backend_errors_without_grpc_url() {
        let ctx = wallet_context_with_env(IotaEnv::new("test", "https://rpc.example"));
        let err = ctx.get_reference_gas_price().await.unwrap_err().to_string();
        assert!(
            err.contains("gRPC is not configured"),
            "expected a gRPC-not-configured error, got: {err}"
        );
    }
}
