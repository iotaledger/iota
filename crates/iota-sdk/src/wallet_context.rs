// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    path::Path,
    sync::Arc,
};

use anyhow::{anyhow, bail, ensure};
use colored::Colorize;
use futures::future;
use getset::{Getters, MutGetters};
use iota_config::{Config, PersistedConfig};
use iota_grpc_client::read_mask_fields::{ObjectField, TransactionField};
use iota_grpc_types::v1::transaction::ExecutedTransaction;
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::{
    Address, Coin, ExecutionStatus, Object, ObjectId, ObjectReference, StructTag, Transaction,
    Version, crypto::Intent,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    transaction::{TransactionAPI, TransactionEnvelope},
};
use tokio::sync::RwLock;
use tracing::warn;

use crate::{
    IotaClient,
    iota_client_config::{IotaClientConfig, IotaEnv},
};

/// How long the fullnode's gRPC API is asked to wait for an executed
/// transaction to be included in a checkpoint before returning, so subsequent
/// reads on the fullnode observe its outputs.
const CHECKPOINT_INCLUSION_TIMEOUT_MS: u64 = 60_000;

/// Wallet for managing accounts, objects, and interact with client APIs.
// Mainly used in the CLI and tests.
#[derive(Getters, MutGetters)]
#[getset(get = "pub", get_mut = "pub")]
pub struct WalletContext {
    config: PersistedConfig<IotaClientConfig>,
    request_timeout: Option<std::time::Duration>,
    client: Arc<RwLock<Option<IotaClient>>>,
    grpc_client: Arc<RwLock<Option<iota_grpc_client::GrpcClient>>>,
    max_concurrent_requests: Option<u64>,
    env_override: Option<String>,
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
        };
        Ok(context)
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

    /// Get the configured gRPC client, creating and caching it on first use.
    /// Errors if the active env has no `grpc` URL configured.
    pub async fn get_grpc_client(&self) -> Result<iota_grpc_client::GrpcClient, anyhow::Error> {
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
        let client = self.get_grpc_client().await?;
        let object = client
            .objects([object_id])
            .read_mask(ObjectField::REFERENCE)
            .await?
            .into_inner()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("object {object_id} not found"))??;
        Ok(object.object_reference()?)
    }

    /// Get all the gas objects (and conveniently, gas amounts) for the address,
    /// each once.
    pub async fn gas_objects(&self, address: Address) -> Result<Vec<(u64, Object)>, anyhow::Error> {
        let client = self.get_grpc_client().await?;
        let values_objects = client
            .owned_objects(address)
            .object_type(StructTag::new_gas_coin())
            .collect(None)
            .await?
            .into_inner()
            .into_iter()
            .map(|proto_object| {
                let object = proto_object.object()?;
                let balance = Coin::try_from_object(&object)?.balance();
                anyhow::Ok((balance, object))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Pages are ordered by balance, so a coin whose balance changes while
        // they are read can come back on a later page, with a higher version.
        Ok(newest_versions(values_objects, |(_, object)| {
            let object_ref = object.object_ref();
            (*object_ref.object_id(), object_ref.version())
        }))
    }

    /// Get the address that owns the object of the provided [`ObjectId`].
    pub async fn get_object_owner(&self, id: &ObjectId) -> Result<Address, anyhow::Error> {
        let client = self.get_grpc_client().await?;
        let object = client
            .objects([*id])
            .read_mask(ObjectField::BCS)
            .await?
            .into_inner()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("object {id} not found"))??
            .object()?;
        Ok(*object
            .owner()
            .address_or_object()
            .ok_or_else(|| anyhow!("not an address or object owner"))?)
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
    ) -> Result<(u64, Object), anyhow::Error> {
        for o in self.gas_objects(address).await? {
            if o.0 >= budget && !forbidden_gas_objects.contains(o.1.object_ref().object_id()) {
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
    /// default). Without a limit, returns the first page that holds gas
    /// objects.
    pub async fn get_gas_objects_owned_by_address(
        &self,
        address: Address,
        limit: impl Into<Option<usize>>,
    ) -> anyhow::Result<Vec<ObjectReference>> {
        let limit = limit.into();
        let client = self.get_grpc_client().await?;
        // `collect` auto-paginates and transparently skips the short or empty
        // pages the owned-objects endpoint may return before the last page.
        let objects = client
            .owned_objects(address)
            .object_type(StructTag::new_gas_coin())
            .collect(limit.map(|limit| limit as u32))
            .await?
            .into_inner();
        objects
            .iter()
            .map(|object| object.object_reference().map_err(|e| anyhow!("{e}")))
            .collect()
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
        let client = self.get_grpc_client().await?;
        Ok(client.reference_gas_price().await?.into_inner())
    }

    /// Add an account.
    pub fn add_account(&mut self, alias: impl Into<Option<String>>, keypair: SimpleKeypair) {
        self.config.keystore.add_key(alias.into(), keypair).unwrap();
    }

    /// Sign a transaction with a key currently managed by the WalletContext.
    pub fn sign_transaction(&self, tx: &Transaction) -> TransactionEnvelope {
        let sig = self
            .config
            .keystore
            .sign_secure(&tx.sender(), tx, Intent::iota_transaction())
            .unwrap();
        // TODO: To support sponsored transaction, we should also look at the gas owner.
        TransactionEnvelope::from_data(tx.clone(), vec![sig])
    }

    /// Execute a transaction via the fullnode's gRPC API and wait for it to be
    /// included in a checkpoint executed by the fullnode, so subsequent reads
    /// on the fullnode observe its outputs. Also expects the effects status to
    /// be `ExecutionStatus::Success`.
    pub async fn execute_transaction_must_succeed(
        &self,
        tx: TransactionEnvelope,
    ) -> ExecutedTransaction {
        tracing::debug!("Executing transaction: {:?}", tx);
        let executed = self.execute_transaction_may_fail(tx).await.unwrap();
        let effects = executed
            .effects()
            .expect("effects are requested in the read mask")
            .effects()
            .expect("effects BCS should deserialize");
        assert!(
            matches!(effects.status(), ExecutionStatus::Success),
            "Transaction failed: {effects:?}"
        );
        executed
    }

    /// Execute a transaction via the fullnode's gRPC API and wait for it to be
    /// included in a checkpoint executed by the fullnode. The transaction
    /// execution is not guaranteed to succeed and may fail. This is usually
    /// only needed in non-test environment or the caller is explicitly testing
    /// some failure behavior.
    pub async fn execute_transaction_may_fail(
        &self,
        tx: TransactionEnvelope,
    ) -> anyhow::Result<ExecutedTransaction> {
        let client = self.get_grpc_client().await?;
        let tx_digest = *tx.digest();
        let executed = client
            .execute_transaction(tx.into())
            .checkpoint_inclusion_timeout_ms(CHECKPOINT_INCLUSION_TIMEOUT_MS)
            .read_mask([
                TransactionField::TRANSACTION_DIGEST,
                TransactionField::EFFECTS_BCS,
                TransactionField::EVENTS_EVENTS_BCS,
                TransactionField::OBJECT_CHANGES,
                TransactionField::BALANCE_CHANGES,
                TransactionField::CHECKPOINT,
                TransactionField::TIMESTAMP,
            ])
            .await?
            .into_inner();
        // The server only sets `checkpoint` once the transaction has been
        // included in a checkpoint within the timeout; treat its absence as a
        // failure to finalize in time.
        if executed.checkpoint.is_none() {
            bail!(
                "transaction {tx_digest} was executed but not included in a checkpoint within {CHECKPOINT_INCLUSION_TIMEOUT_MS}ms"
            );
        }
        Ok(executed)
    }
}

/// `items` in their order, with each object once: an object that occurs more
/// than once keeps the place of its first occurrence and the contents of its
/// highest version.
fn newest_versions<T>(items: Vec<T>, key: impl Fn(&T) -> (ObjectId, Version)) -> Vec<T> {
    let mut positions: HashMap<ObjectId, usize> = HashMap::new();
    let mut kept: Vec<T> = Vec::with_capacity(items.len());
    for item in items {
        let (id, version) = key(&item);
        match positions.entry(id) {
            Entry::Occupied(position) => {
                let held = &mut kept[*position.get()];
                if version > key(held).1 {
                    *held = item;
                }
            }
            Entry::Vacant(position) => {
                position.insert(kept.len());
                kept.push(item);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An object that occurs again with a higher version keeps its first place
    /// with the newer contents, and an older copy is left out.
    #[test]
    fn an_object_keeps_its_first_place_and_its_highest_version() {
        let (a, b) = (ObjectId::random(), ObjectId::random());
        let items = vec![
            (a, Version::from_u64(1), "a1"),
            (b, Version::from_u64(2), "b2"),
            (a, Version::from_u64(3), "a3"),
            (b, Version::from_u64(1), "b1"),
        ];

        let kept = newest_versions(items, |(id, version, _)| (*id, *version));
        let names: Vec<_> = kept.iter().map(|(_, _, name)| *name).collect();
        assert_eq!(names, ["a3", "b2"]);
    }
}
