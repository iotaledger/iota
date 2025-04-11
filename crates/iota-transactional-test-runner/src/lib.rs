// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module contains the transactional test runner instantiation for the
//! IOTA adapter

pub mod args;
pub mod programmable_transaction_test_parser;
mod simulator_persisted_store;
pub mod test_adapter;

use std::{path::Path, sync::Arc};

use iota_core::authority::{
    AuthorityState, authority_per_epoch_store::CertLockGuard,
    authority_test_utils::send_and_confirm_transaction_with_execution_error,
};
use iota_json_rpc::authority_state::StateRead;
use iota_json_rpc_types::{DevInspectResults, DryRunTransactionBlockResponse, EventFilter};
use iota_storage::key_value_store::TransactionKeyValueStore;
use iota_types::{
    base_types::{IotaAddress, ObjectID, VersionNumber},
    committee::EpochId,
    digests::{TransactionDigest, TransactionEventsDigest},
    effects::{TransactionEffects, TransactionEvents},
    error::{ExecutionError, IotaError, IotaResult},
    event::Event,
    executable_transaction::{ExecutableTransaction, VerifiedExecutableTransaction},
    iota_system_state::{
        IotaSystemStateTrait, epoch_start_iota_system_state::EpochStartSystemStateTrait,
        iota_system_state_summary::IotaSystemStateSummary,
    },
    messages_checkpoint::{CheckpointContentsDigest, VerifiedCheckpoint},
    object::Object,
    storage::{ObjectStore, ReadStore},
    transaction::{
        InputObjects, Transaction, TransactionData, TransactionDataAPI, TransactionKind,
    },
};
pub use move_transactional_test_runner::framework::run_test_impl;
use rand::rngs::StdRng;
use simulacrum::{Simulacrum, SimulatorStore};
use simulator_persisted_store::PersistedStore;
use test_adapter::{IotaTestAdapter, PRE_COMPILED};

#[cfg_attr(not(msim), tokio::main)]
#[cfg_attr(msim, msim::main)]
pub async fn run_test(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (_guard, _filter_handle) = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    run_test_impl::<IotaTestAdapter>(path, Some(std::sync::Arc::new(PRE_COMPILED.clone()))).await?;
    Ok(())
}

pub struct ValidatorWithFullnode {
    pub validator: Arc<AuthorityState>,
    pub fullnode: Arc<AuthorityState>,
    pub kv_store: Arc<TransactionKeyValueStore>,
}

/// TODO: better name?
#[async_trait::async_trait]
pub trait TransactionalAdapter: Send + Sync + ReadStore {
    async fn execute_txn(
        &mut self,
        transaction: Transaction,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)>;

    async fn read_input_objects(&self, transaction: Transaction) -> IotaResult<InputObjects>;

    fn prepare_txn(
        &self,
        transaction: Transaction,
        input_objects: InputObjects,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)>;

    async fn create_checkpoint(&mut self) -> anyhow::Result<VerifiedCheckpoint>;

    async fn advance_clock(
        &mut self,
        duration: std::time::Duration,
    ) -> anyhow::Result<TransactionEffects>;

    async fn advance_epoch(&mut self) -> anyhow::Result<()>;

    async fn request_gas(
        &mut self,
        address: IotaAddress,
        amount: u64,
    ) -> anyhow::Result<TransactionEffects>;

    async fn dry_run_transaction_block(
        &self,
        transaction_block: TransactionData,
        transaction_digest: TransactionDigest,
    ) -> IotaResult<DryRunTransactionBlockResponse>;

    async fn dev_inspect_transaction_block(
        &self,
        sender: IotaAddress,
        transaction_kind: TransactionKind,
        gas_price: Option<u64>,
    ) -> IotaResult<DevInspectResults>;

    async fn query_tx_events_asc(
        &self,
        tx_digest: &TransactionDigest,
        limit: usize,
    ) -> IotaResult<Vec<Event>>;

    async fn get_active_validator_addresses(&self) -> IotaResult<Vec<IotaAddress>>;
}

#[async_trait::async_trait]
impl TransactionalAdapter for ValidatorWithFullnode {
    async fn execute_txn(
        &mut self,
        transaction: Transaction,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)> {
        let with_shared = transaction
            .data()
            .intent_message()
            .value
            .contains_shared_object();
        let (_, effects, execution_error) = send_and_confirm_transaction_with_execution_error(
            &self.validator,
            Some(&self.fullnode),
            transaction,
            with_shared,
            false,
        )
        .await?;
        Ok((effects.into_data(), execution_error))
    }

    async fn read_input_objects(&self, transaction: Transaction) -> IotaResult<InputObjects> {
        let tx = VerifiedExecutableTransaction::new_unchecked(
            ExecutableTransaction::new_from_data_and_sig(
                transaction.data().clone(),
                iota_types::executable_transaction::CertificateProof::Checkpoint(0, 0),
            ),
        );

        let epoch_store = self.validator.load_epoch_store_one_call_per_task().clone();
        self.validator.read_objects_for_execution(
            &CertLockGuard::guard_for_tests(),
            &tx,
            &epoch_store,
        )
    }

    fn prepare_txn(
        &self,
        transaction: Transaction,
        input_objects: InputObjects,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)> {
        let tx = VerifiedExecutableTransaction::new_unchecked(
            ExecutableTransaction::new_from_data_and_sig(
                transaction.data().clone(),
                iota_types::executable_transaction::CertificateProof::Checkpoint(0, 0),
            ),
        );

        let epoch_store = self.validator.load_epoch_store_one_call_per_task().clone();
        let (_, effects, error) =
            self.validator
                .prepare_certificate_for_benchmark(&tx, input_objects, &epoch_store)?;
        Ok((effects, error))
    }

    async fn dry_run_transaction_block(
        &self,
        transaction_block: TransactionData,
        transaction_digest: TransactionDigest,
    ) -> IotaResult<DryRunTransactionBlockResponse> {
        self.fullnode
            .dry_exec_transaction(transaction_block, transaction_digest)
            .await
            .map(|result| result.0)
    }

    async fn dev_inspect_transaction_block(
        &self,
        sender: IotaAddress,
        transaction_kind: TransactionKind,
        gas_price: Option<u64>,
    ) -> IotaResult<DevInspectResults> {
        self.fullnode
            .dev_inspect_transaction_block(
                sender,
                transaction_kind,
                gas_price,
                None,
                None,
                None,
                None,
                None,
            )
            .await
    }

    async fn query_tx_events_asc(
        &self,
        tx_digest: &TransactionDigest,
        limit: usize,
    ) -> IotaResult<Vec<Event>> {
        Ok(self
            .validator
            .query_events(
                &self.kv_store,
                EventFilter::Transaction(*tx_digest),
                None,
                limit,
                false,
            )
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|iota_event| iota_event.into())
            .collect())
    }

    async fn create_checkpoint(&mut self) -> anyhow::Result<VerifiedCheckpoint> {
        unimplemented!("create_checkpoint not supported")
    }

    async fn advance_clock(
        &mut self,
        _duration: std::time::Duration,
    ) -> anyhow::Result<TransactionEffects> {
        unimplemented!("advance_clock not supported")
    }

    async fn advance_epoch(&mut self) -> anyhow::Result<()> {
        self.validator.reconfigure_for_testing().await;
        self.fullnode.reconfigure_for_testing().await;
        Ok(())
    }

    async fn request_gas(
        &mut self,
        _address: IotaAddress,
        _amount: u64,
    ) -> anyhow::Result<TransactionEffects> {
        unimplemented!("request_gas not supported")
    }

    async fn get_active_validator_addresses(&self) -> IotaResult<Vec<IotaAddress>> {
        let system_state_summary = self
            .fullnode
            .get_system_state()
            .map_err(|e| {
                IotaError::IotaSystemStateRead(format!(
                    "Failed to get system state from fullnode: {}",
                    e
                ))
            })?
            .into_iota_system_state_summary();
        let active_validators = match system_state_summary {
            IotaSystemStateSummary::V1(inner) => inner.active_validators,
            IotaSystemStateSummary::V2(inner) => inner.active_validators,
            _ => unimplemented!(),
        };

        Ok(active_validators
            .iter()
            .map(|x| x.iota_address)
            .collect::<Vec<_>>())
    }
}

impl ReadStore for ValidatorWithFullnode {
    fn get_committee(
        &self,
        _epoch: EpochId,
    ) -> iota_types::storage::error::Result<Option<Arc<iota_types::committee::Committee>>> {
        todo!()
    }

    fn get_latest_epoch_id(&self) -> iota_types::storage::error::Result<EpochId> {
        Ok(self.validator.epoch_store_for_testing().epoch())
    }

    fn get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        let sequence_number = self
            .validator
            .get_latest_checkpoint_sequence_number()
            .unwrap();
        self.get_checkpoint_by_sequence_number(sequence_number)
            .map(|c| c.unwrap())
    }

    fn get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        todo!()
    }

    fn get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        todo!()
    }

    fn get_lowest_available_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<iota_types::messages_checkpoint::CheckpointSequenceNumber>
    {
        todo!()
    }

    fn get_checkpoint_by_digest(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointDigest,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        todo!()
    }

    fn get_checkpoint_by_sequence_number(
        &self,
        sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        self.validator
            .get_checkpoint_store()
            .get_checkpoint_by_sequence_number(sequence_number)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        self.validator
            .get_checkpoint_store()
            .get_checkpoint_contents(digest)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_checkpoint_contents_by_sequence_number(
        &self,
        _sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        todo!()
    }

    fn get_transaction(
        &self,
        tx_digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<Arc<iota_types::transaction::VerifiedTransaction>>>
    {
        self.validator
            .get_transaction_cache_reader()
            .get_transaction_block(tx_digest)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_transaction_effects(
        &self,
        tx_digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionEffects>> {
        self.validator
            .get_transaction_cache_reader()
            .get_executed_effects(tx_digest)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_events(
        &self,
        event_digest: &TransactionEventsDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionEvents>> {
        self.validator
            .get_transaction_cache_reader()
            .get_events(event_digest)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_full_checkpoint_contents_by_sequence_number(
        &self,
        _sequence_number: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        todo!()
    }

    fn get_full_checkpoint_contents(
        &self,
        _digest: &CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        todo!()
    }
}

impl ObjectStore for ValidatorWithFullnode {
    fn get_object(
        &self,
        object_id: &ObjectID,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        self.validator.get_object_store().get_object(object_id)
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        self.validator
            .get_object_store()
            .get_object_by_key(object_id, version)
    }
}

#[async_trait::async_trait]
impl TransactionalAdapter for Simulacrum<StdRng, PersistedStore> {
    async fn execute_txn(
        &mut self,
        transaction: Transaction,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)> {
        Ok(self.execute_transaction(transaction)?)
    }

    async fn read_input_objects(&self, _transaction: Transaction) -> IotaResult<InputObjects> {
        unimplemented!("read_input_objects not supported in simulator mode")
    }

    fn prepare_txn(
        &self,
        _transaction: Transaction,
        _input_objects: InputObjects,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)> {
        unimplemented!("prepare_txn not supported in simulator mode")
    }

    async fn dev_inspect_transaction_block(
        &self,
        _sender: IotaAddress,
        _transaction_kind: TransactionKind,
        _gas_price: Option<u64>,
    ) -> IotaResult<DevInspectResults> {
        unimplemented!("dev_inspect_transaction_block not supported in simulator mode")
    }

    async fn dry_run_transaction_block(
        &self,
        _transaction_block: TransactionData,
        _transaction_digest: TransactionDigest,
    ) -> IotaResult<DryRunTransactionBlockResponse> {
        unimplemented!("dry_run_transaction_block not supported in simulator mode")
    }

    async fn query_tx_events_asc(
        &self,
        tx_digest: &TransactionDigest,
        _limit: usize,
    ) -> IotaResult<Vec<Event>> {
        Ok(self
            .store()
            .get_transaction_events_by_tx_digest(tx_digest)
            .map(|x| x.data)
            .unwrap_or_default())
    }

    async fn create_checkpoint(&mut self) -> anyhow::Result<VerifiedCheckpoint> {
        Ok(self.create_checkpoint())
    }

    async fn advance_clock(
        &mut self,
        duration: std::time::Duration,
    ) -> anyhow::Result<TransactionEffects> {
        Ok(self.advance_clock(duration))
    }

    async fn advance_epoch(&mut self) -> anyhow::Result<()> {
        self.advance_epoch();
        Ok(())
    }

    async fn request_gas(
        &mut self,
        address: IotaAddress,
        amount: u64,
    ) -> anyhow::Result<TransactionEffects> {
        self.request_gas(address, amount)
    }

    async fn get_active_validator_addresses(&self) -> IotaResult<Vec<IotaAddress>> {
        // TODO: this is a hack to get the validator addresses. Currently using start
        // state       but we should have a better way to get this information
        // after reconfig
        Ok(self.epoch_start_state().get_validator_addresses())
    }
}
