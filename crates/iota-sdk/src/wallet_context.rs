// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, path::Path, sync::Arc};

use anyhow::{anyhow, bail, ensure};
use colored::Colorize;
use futures::future;
use getset::{Getters, MutGetters};
use iota_config::{Config, PersistedConfig};
use iota_json_rpc_types::{
    IotaObjectData, IotaObjectDataOptions, IotaObjectResponse, IotaTransactionBlockEffects,
    IotaTransactionBlockResponse,
};
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_sdk_types::{Address, ObjectId, SignedTransaction, StructTag, crypto::Intent};
use iota_types::{
    base_types::ObjectRef,
    crypto::IotaKeyPair,
    effects::{TransactionEffects, TransactionEffectsAPI},
    gas_coin::GasCoin,
    object::{Object, ObjectRead},
    transaction::{Transaction, TransactionData, TransactionDataAPI},
};
use tokio::sync::RwLock;
use tracing::warn;

use crate::{
    IotaClient,
    iota_client_config::{IotaClientConfig, IotaEnv},
};

/// Checkpoint-inclusion timeout for driving a transaction through the fullnode
/// gRPC API, so later reads observe the committed effects (mirrors the old
/// `WaitForLocalExecution`).
const CHECKPOINT_INCLUSION_TIMEOUT_MS: u64 = 60_000;

/// Build an [`IotaTransactionBlockResponse`] from raw transaction effects.
///
/// Populates the digest and effects only. Events, object changes, and balance
/// changes require Move layouts served by the indexer and are left unset.
pub fn response_from_raw_effects(
    effects: TransactionEffects,
) -> anyhow::Result<IotaTransactionBlockResponse> {
    let mut response = IotaTransactionBlockResponse::new(*effects.transaction_digest());
    response.effects =
        Some(IotaTransactionBlockEffects::try_from(effects).map_err(|e| anyhow!("{e}"))?);
    response.confirmed_local_execution = Some(true);
    Ok(response)
}

/// Wallet for managing accounts, objects, and interact with client APIs.
// Mainly used in the CLI and tests.
#[derive(Getters, MutGetters)]
#[getset(get = "pub", get_mut = "pub")]
pub struct WalletContext {
    config: PersistedConfig<IotaClientConfig>,
    request_timeout: Option<std::time::Duration>,
    client: Arc<RwLock<Option<IotaClient>>>,
    max_concurrent_requests: Option<u64>,
    env_override: Option<String>,
    // Node gRPC client for reads (gas, gas price, object refs/owners) and
    // transaction execution. Built lazily from the active env's `grpc` URL
    // unless pre-seeded via `with_grpc_client`.
    #[getset(skip)]
    grpc_client: Arc<RwLock<Option<iota_grpc_client::Client>>>,
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
            max_concurrent_requests: None,
            env_override: None,
            grpc_client: Default::default(),
        };
        Ok(context)
    }

    /// Pre-seed the node gRPC client, overriding the client that would
    /// otherwise be built lazily from the active env's `grpc` URL. Used by
    /// tests where the node binds a dynamic port.
    pub fn with_grpc_client(mut self, client: iota_grpc_client::Client) -> Self {
        self.grpc_client = Arc::new(RwLock::new(Some(client)));
        self
    }

    /// Get the node gRPC client, building it from the active env's `grpc` URL
    /// on first use unless one was pre-seeded via
    /// [`Self::with_grpc_client`].
    pub async fn grpc_client(&self) -> anyhow::Result<iota_grpc_client::Client> {
        if let Some(client) = self.grpc_client.read().await.as_ref() {
            return Ok(client.clone());
        }
        let client = self.active_env()?.create_grpc_client()?;
        Ok(self.grpc_client.write().await.insert(client).clone())
    }

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
    pub async fn get_object_ref(&self, object_id: ObjectId) -> Result<ObjectRef, anyhow::Error> {
        let objects = self
            .grpc_client()
            .await?
            .get_objects(&[(object_id, None)], None)
            .await?;
        let object = objects
            .body()
            .first()
            .ok_or_else(|| anyhow!("object {object_id} not found"))?;
        object.object_reference().map_err(|e| anyhow!("{e}"))
    }

    /// Get all the gas objects (and conveniently, gas amounts) for the address.
    pub async fn gas_objects(
        &self,
        address: Address,
    ) -> Result<Vec<(u64, IotaObjectData)>, anyhow::Error> {
        let objects = self
            .grpc_client()
            .await?
            .list_owned_objects(address, Some(StructTag::new_gas_coin()), None, None, None)
            .collect(None)
            .await?;
        objects
            .body()
            .iter()
            .map(|proto| {
                let object: Object = proto.object().map_err(|e| anyhow!("{e}"))?.into();
                let value = GasCoin::try_from(&object)
                    .map_err(|e| anyhow!("{e}"))?
                    .value();
                let object_ref = object.object_ref();
                // Every listed object is a gas coin, so its Move layout is the
                // static `GasCoin` layout; needed to render `full_content`.
                let response: IotaObjectResponse = (
                    ObjectRead::Exists(object_ref, object, Some(GasCoin::layout())),
                    IotaObjectDataOptions::full_content(),
                )
                    .try_into()?;
                Ok((value, response.into_object()?))
            })
            .collect()
    }

    /// Get the address that owns the object of the provided [`ObjectId`].
    pub async fn get_object_owner(&self, id: &ObjectId) -> Result<Address, anyhow::Error> {
        let objects = self
            .grpc_client()
            .await?
            .get_objects(&[(*id, None)], None)
            .await?;
        let object = objects
            .body()
            .first()
            .ok_or_else(|| anyhow!("object {id} not found"))?
            .object()
            .map_err(|e| anyhow!("{e}"))?;
        object
            .owner()
            .address_or_object()
            .copied()
            .ok_or_else(|| anyhow!("not an address or object owner"))
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
    ) -> Result<(u64, IotaObjectData), anyhow::Error> {
        for o in self.gas_objects(address).await? {
            if o.0 >= budget && !forbidden_gas_objects.contains(&o.1.object_id) {
                return Ok((o.0, o.1));
            }
        }
        bail!(
            "No non-argument gas objects found for this address with value >= budget {budget}. Run iota client gas to check for gas objects."
        )
    }

    /// Get the [`ObjectRef`]s for all gas objects owned by the provided
    /// address.
    pub async fn get_all_gas_objects_owned_by_address(
        &self,
        address: Address,
    ) -> anyhow::Result<Vec<ObjectRef>> {
        self.get_gas_objects_owned_by_address(address, None).await
    }

    /// Get [`ObjectRef`]s for gas objects owned by the provided address;
    /// `None` returns all of them, `Some(n)` at most `n`.
    pub async fn get_gas_objects_owned_by_address(
        &self,
        address: Address,
        limit: impl Into<Option<usize>>,
    ) -> anyhow::Result<Vec<ObjectRef>> {
        let limit = limit.into().map(|l| l as u32);
        let objects = self
            .grpc_client()
            .await?
            .list_owned_objects(address, Some(StructTag::new_gas_coin()), limit, None, None)
            .collect(limit)
            .await?;
        objects
            .body()
            .iter()
            .map(|o| o.object_reference().map_err(|e| anyhow!("{e}")))
            .collect()
    }

    /// Given an address, return one gas object owned by this address.
    ///
    /// Returns the highest-balance gas coin (the gRPC owner index sorts coins
    /// richest-first). The selection is not stable across calls: spending gas
    /// shifts the ordering. Callers that need the same coin across repeated
    /// calls must select deterministically (e.g. by object id).
    pub async fn get_one_gas_object_owned_by_address(
        &self,
        address: Address,
    ) -> anyhow::Result<Option<ObjectRef>> {
        Ok(self
            .get_gas_objects_owned_by_address(address, 1)
            .await?
            .pop())
    }

    /// Return one address and all gas objects owned by that address.
    pub async fn get_one_account(&self) -> anyhow::Result<(Address, Vec<ObjectRef>)> {
        let address = self.get_addresses().pop().unwrap();
        Ok((
            address,
            self.get_all_gas_objects_owned_by_address(address).await?,
        ))
    }

    /// Return a gas object owned by an arbitrary address managed by the wallet.
    pub async fn get_one_gas_object(&self) -> anyhow::Result<Option<(Address, ObjectRef)>> {
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
    ) -> anyhow::Result<Vec<(Address, Vec<ObjectRef>)>> {
        let mut result = vec![];
        for address in self.get_addresses() {
            let objects = self.get_gas_objects_owned_by_address(address, None).await?;
            result.push((address, objects));
        }
        Ok(result)
    }

    pub async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        Ok(self
            .grpc_client()
            .await?
            .get_reference_gas_price()
            .await?
            .into_inner())
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
            "Transaction failed: {response:?}"
        );
        response
    }

    /// Execute a transaction over the node gRPC API and wait for its effects to
    /// be committed. The execution is not guaranteed to succeed; a failed
    /// execution is returned as `Ok` with failure effects.
    ///
    /// The response carries the digest and effects only; parsed events, object
    /// changes, and balance changes require Move layouts served by the indexer.
    pub async fn execute_transaction_may_fail(
        &self,
        tx: Transaction,
    ) -> anyhow::Result<IotaTransactionBlockResponse> {
        let signed: SignedTransaction = tx.try_into().map_err(|e| anyhow!("{e}"))?;
        let executed = self
            .grpc_client()
            .await?
            .execute_transaction(signed, None, Some(CHECKPOINT_INCLUSION_TIMEOUT_MS))
            .await?;
        let effects = executed
            .body()
            .effects()
            .map_err(|e| anyhow!("{e}"))?
            .effects()
            .map_err(|e| anyhow!("{e}"))?;
        response_from_raw_effects(effects)
    }

    /// Simulate a transaction over the node gRPC and return its raw effects.
    ///
    /// Used for gas-budget estimation. Rendering a full dry-run response
    /// (parsed input/events) requires layouts served by the indexer, not
    /// the node.
    pub async fn simulate_transaction_effects(
        &self,
        tx_data: TransactionData,
    ) -> anyhow::Result<TransactionEffects> {
        // `TransactionData` and the SDK `Transaction` share a BCS encoding.
        let tx: iota_sdk_types::Transaction =
            bcs::from_bytes(&bcs::to_bytes(&tx_data)?).map_err(|e| anyhow!("{e}"))?;
        let simulated = self
            .grpc_client()
            .await?
            .simulate_transaction(tx, false, None)
            .await?;
        simulated
            .body()
            .executed_transaction()
            .map_err(|e| anyhow!("{e}"))?
            .effects()
            .map_err(|e| anyhow!("{e}"))?
            .effects()
            .map_err(|e| anyhow!("{e}"))
    }
}
