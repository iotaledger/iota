// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Node-internal implementation of the SDK's [`TransactionBuilderClient`].
//!
//! [`NodeTransactionBuilderClient`] backs the SDK
//! [`TransactionBuilder`](iota_sdk_transaction_builder::TransactionBuilder)
//! with a node's local state instead of a remote gRPC or GraphQL endpoint:
//! reads go through [`GrpcStateReader`] (the same interface the gRPC server
//! uses) and execution / dry runs go through
//! [`TransactionExecutor`] (implemented by the transaction orchestrator on a
//! fullnode, and by simulacrum in tests).

use std::{sync::Arc, time::Duration};

use iota_node_storage::GrpcStateReader;
use iota_protocol_config::{
    ProtocolConfig as NodeProtocolConfig, ProtocolConfigValue, ProtocolVersion,
};
use iota_sdk_transaction_builder::{
    ObjectsPage, ProtocolConfig, TransactionBuilderClient, WaitForTx,
};
use iota_sdk_types::{
    Address, Object, ObjectId, SignedTransaction, StructTag, Transaction, TransactionDigest,
    TransactionEffects, UserSignature, Version,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::IotaError,
    iota_sdk_types_conversions::SdkTypeConversionError,
    iota_system_state::{IotaSystemStateTrait, get_iota_system_state},
    quorum_driver_types::{ExecuteTransactionRequestV1, QuorumDriverError},
    storage::OwnedObjectCursor,
    transaction::TransactionDataAPI,
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};
use typed_store_error::TypedStoreError;

/// Default number of objects returned by
/// [`TransactionBuilderClient::objects`] when no limit is given.
const DEFAULT_OBJECTS_PAGE_SIZE: usize = 50;

/// Upper bound on the number of objects returned by
/// [`TransactionBuilderClient::objects`].
const MAX_OBJECTS_PAGE_SIZE: usize = 1000;

/// How long [`TransactionBuilderClient::wait_for_tx`] waits before giving up.
const WAIT_FOR_TX_TIMEOUT: Duration = Duration::from_secs(60);

/// Interval between polls in [`TransactionBuilderClient::wait_for_tx`].
const WAIT_FOR_TX_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Error type for [`NodeTransactionBuilderClient`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] iota_types::storage::error::Error),
    #[error(transparent)]
    Store(#[from] TypedStoreError),
    #[error(transparent)]
    Conversion(#[from] SdkTypeConversionError),
    #[error(transparent)]
    Execution(#[from] QuorumDriverError),
    #[error(transparent)]
    Node(#[from] IotaError),
    #[error("invalid objects page cursor: {0}")]
    Cursor(bcs::Error),
    #[error("gRPC indexes are disabled on this node")]
    IndexesDisabled,
    #[error("protocol version {0} is not supported by this node")]
    UnsupportedProtocolVersion(u64),
    #[error("timed out waiting for transaction {0}")]
    WaitForTransactionTimeout(TransactionDigest),
}

/// A [`TransactionBuilderClient`] backed directly by a node's local state.
#[derive(Clone)]
pub struct NodeTransactionBuilderClient {
    reader: Arc<dyn GrpcStateReader>,
    executor: Arc<dyn TransactionExecutor>,
}

impl NodeTransactionBuilderClient {
    pub fn new(reader: Arc<dyn GrpcStateReader>, executor: Arc<dyn TransactionExecutor>) -> Self {
        Self { reader, executor }
    }

    /// Protocol config of the current epoch.
    fn node_protocol_config(&self) -> Result<NodeProtocolConfig, Error> {
        let system_state = get_iota_system_state(self.reader.as_ref())?;
        let chain = self.reader.get_chain_identifier()?.chain();
        let protocol_version = system_state.protocol_version();
        NodeProtocolConfig::get_for_version_if_supported(
            ProtocolVersion::new(protocol_version),
            chain,
        )
        .ok_or(Error::UnsupportedProtocolVersion(protocol_version))
    }

    /// Prepare a transaction for simulation: with a zero gas budget the dry
    /// run would fail upfront, so run it with the maximum budget instead —
    /// the effects then carry the actual cost. Mirrors the gRPC server's
    /// handling of zero-budget simulations.
    fn transaction_for_simulation(&self, tx: &Transaction) -> Result<Transaction, Error> {
        let mut transaction = tx.clone();
        if transaction.gas_data().budget == 0 {
            transaction.gas_data_mut().budget = self.node_protocol_config()?.max_tx_gas();
        }
        Ok(transaction)
    }
}

fn protocol_config_value_to_string(value: ProtocolConfigValue) -> String {
    match value {
        ProtocolConfigValue::u16(x) => x.to_string(),
        ProtocolConfigValue::u32(x) => x.to_string(),
        ProtocolConfigValue::u64(x) => x.to_string(),
        ProtocolConfigValue::bool(x) => x.to_string(),
    }
}

impl TransactionBuilderClient for NodeTransactionBuilderClient {
    type Error = Error;
    type DryRunResult = SimulateTransactionResult;

    async fn object(
        &self,
        object_id: ObjectId,
        version: impl Into<Option<Version>>,
    ) -> Result<Option<Object>, Self::Error> {
        let object = match version.into() {
            Some(version) => self.reader.try_get_object_by_key(&object_id, version)?,
            None => self.reader.try_get_object(&object_id)?,
        };
        object.map(Object::try_from).transpose().map_err(Into::into)
    }

    async fn objects(
        &self,
        struct_tag: Option<StructTag>,
        owner: Address,
        cursor: Option<Vec<u8>>,
        limit: Option<usize>,
    ) -> Result<ObjectsPage, Self::Error> {
        let limit = limit
            .unwrap_or(DEFAULT_OBJECTS_PAGE_SIZE)
            .clamp(1, MAX_OBJECTS_PAGE_SIZE);
        let cursor: Option<OwnedObjectCursor> = cursor
            .map(|bytes| bcs::from_bytes(&bytes))
            .transpose()
            .map_err(Error::Cursor)?;

        let indexes = self.reader.grpc_indexes().ok_or(Error::IndexesDisabled)?;

        // The index iterator's cursor bound is inclusive, so skip the cursor
        // item itself to advance past the previous page.
        let skip = usize::from(cursor.is_some());
        let mut iter = indexes
            .account_owned_objects_info_iter(owner, cursor.as_ref(), struct_tag)?
            .skip(skip);

        let mut data = Vec::with_capacity(limit);
        let mut last_cursor = None;
        for item in iter.by_ref() {
            let (info, item_cursor) = item?;
            let Some(object) = self
                .reader
                .try_get_object_by_key(&info.object_id, info.version)?
            else {
                // The object is no longer at the indexed version (e.g. mutated
                // between the index scan and the fetch).
                tracing::debug!(
                    object_id = %info.object_id,
                    version = %info.version,
                    "object not found while iterating owned objects, skipping",
                );
                continue;
            };
            data.push(Object::try_from(object)?);
            last_cursor = Some(item_cursor);
            if data.len() >= limit {
                break;
            }
        }

        let has_more = iter.next().transpose()?.is_some();
        let next_cursor = if has_more {
            last_cursor
                .map(|cursor| bcs::to_bytes(&cursor))
                .transpose()
                .map_err(Error::Cursor)?
        } else {
            None
        };

        Ok(ObjectsPage { data, next_cursor })
    }

    async fn protocol_config(&self) -> Result<ProtocolConfig, Self::Error> {
        let attributes = self
            .node_protocol_config()?
            .attr_map()
            .into_iter()
            .filter_map(|(name, value)| {
                value.map(|value| (name, protocol_config_value_to_string(value)))
            })
            .collect();
        Ok(ProtocolConfig { attributes })
    }

    async fn transaction(
        &self,
        digest: TransactionDigest,
    ) -> Result<Option<SignedTransaction>, Self::Error> {
        let Some(transaction) = self.reader.try_get_transaction(&digest)? else {
            return Ok(None);
        };
        let transaction = Arc::try_unwrap(transaction).unwrap_or_else(|arc| (*arc).clone());
        Ok(Some(transaction.into_inner().into()))
    }

    async fn transaction_effects(
        &self,
        digest: TransactionDigest,
    ) -> Result<Option<TransactionEffects>, Self::Error> {
        Ok(self.reader.try_get_transaction_effects(&digest)?)
    }

    async fn reference_gas_price(
        &self,
        epoch: impl Into<Option<u64>>,
    ) -> Result<Option<u64>, Self::Error> {
        match epoch.into() {
            None => {
                let system_state = get_iota_system_state(self.reader.as_ref())?;
                Ok(Some(system_state.reference_gas_price()))
            }
            Some(epoch) => Ok(self
                .reader
                .get_epoch_info(epoch)?
                .map(|info| info.reference_gas_price())),
        }
    }

    async fn estimate_tx_budget(&self, tx: &Transaction) -> Result<Option<u64>, Self::Error> {
        let transaction = self.transaction_for_simulation(tx)?;
        let result = self
            .executor
            .simulate_transaction(transaction, VmChecks::Disabled)?;
        Ok(Some(result.effects.gas_cost_summary().gas_used()))
    }

    async fn dry_run_tx(
        &self,
        tx: &Transaction,
        skip_checks: bool,
    ) -> Result<Self::DryRunResult, Self::Error> {
        let checks = if skip_checks {
            VmChecks::Disabled
        } else {
            VmChecks::Enabled
        };
        // Only apply the zero-budget replacement with checks disabled: with
        // checks enabled the caller asked for full validation, and a zero
        // budget should fail it.
        let transaction = if checks.disabled() {
            self.transaction_for_simulation(tx)?
        } else {
            tx.clone()
        };
        Ok(self.executor.simulate_transaction(transaction, checks)?)
    }

    async fn execute_tx(
        &self,
        signatures: &[UserSignature],
        tx: &Transaction,
        wait_for: impl Into<Option<WaitForTx>>,
    ) -> Result<TransactionEffects, Self::Error> {
        let signed_transaction = SignedTransaction {
            transaction: tx.clone(),
            signatures: signatures.to_vec(),
        };
        let request = ExecuteTransactionRequestV1::new(iota_types::transaction::Transaction::from(
            signed_transaction,
        ));
        let response = self
            .executor
            .execute_transaction(request, false, None)
            .await?;

        if let Some(wait_for) = wait_for.into() {
            self.wait_for_tx(tx.digest(), wait_for).await?;
        }

        Ok(response.effects.effects)
    }

    async fn wait_for_tx(
        &self,
        digest: TransactionDigest,
        wait_for: WaitForTx,
    ) -> Result<(), Self::Error> {
        match wait_for {
            WaitForTx::IndexedOnNode => {
                let poll = async {
                    let mut interval = tokio::time::interval(WAIT_FOR_TX_POLL_INTERVAL);
                    loop {
                        interval.tick().await;
                        if self.reader.try_get_transaction_effects(&digest)?.is_some() {
                            return Ok(());
                        }
                    }
                };
                tokio::time::timeout(WAIT_FOR_TX_TIMEOUT, poll)
                    .await
                    .map_err(|_| Error::WaitForTransactionTimeout(digest))?
            }
            WaitForTx::Finalized => {
                let included = self
                    .executor
                    .wait_for_checkpoint_inclusion(&[digest], WAIT_FOR_TX_TIMEOUT)
                    .await?;
                if included.contains_key(&digest) {
                    Ok(())
                } else {
                    Err(Error::WaitForTransactionTimeout(digest))
                }
            }
            _ => unimplemented!("a new WaitForTx enum variant was added and needs to be handled"),
        }
    }
}
