// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module contains the transactional test runner instantiation for the Sui adapter

pub mod args;
pub mod programmable_transaction_test_parser;
pub mod test_adapter;

pub use move_transactional_test_runner::framework::run_test_impl;
use rand::rngs::StdRng;
use simulacrum::{InMemoryStore, Simulacrum};
use std::path::Path;
use sui_json_rpc_types::DevInspectResults;
use sui_types::base_types::SuiAddress;
use sui_types::digests::TransactionDigest;
use sui_types::effects::TransactionEffects;
use sui_types::error::ExecutionError;
use sui_types::error::SuiResult;
use sui_types::event::Event;
use sui_types::messages_checkpoint::VerifiedCheckpoint;
use sui_types::storage::ReadStore;
use sui_types::sui_system_state::epoch_start_sui_system_state::EpochStartSystemStateTrait;
use sui_types::transaction::Transaction;
use sui_types::transaction::TransactionKind;
use test_adapter::{SuiTestAdapter, PRE_COMPILED};

#[cfg_attr(not(msim), tokio::main)]
#[cfg_attr(msim, msim::main)]
pub async fn run_test(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    telemetry_subscribers::init_for_testing();
    run_test_impl::<SuiTestAdapter>(path, Some(&*PRE_COMPILED)).await?;
    Ok(())
}

#[allow(unused_variables)]
/// TODO: better name?
#[async_trait::async_trait]
pub trait TransactionalAdapter: Send + Sync + ReadStore {
    async fn execute_txn(
        &mut self,
        transaction: Transaction,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)>;

    async fn create_checkpoint(&mut self) -> anyhow::Result<VerifiedCheckpoint>;

    async fn advance_clock(
        &mut self,
        duration: std::time::Duration,
    ) -> anyhow::Result<TransactionEffects>;

    async fn advance_epoch(&mut self, create_random_state: bool) -> anyhow::Result<()>;

    async fn request_gas(
        &mut self,
        address: SuiAddress,
        amount: u64,
    ) -> anyhow::Result<TransactionEffects>;

    async fn dev_inspect_transaction_block(
        &self,
        sender: SuiAddress,
        transaction_kind: TransactionKind,
        gas_price: Option<u64>,
    ) -> SuiResult<DevInspectResults>;

    async fn query_tx_events_asc(
        &self,
        tx_digest: &TransactionDigest,
        limit: usize,
    ) -> SuiResult<Vec<Event>>;

    async fn get_active_validator_addresses(&self) -> SuiResult<Vec<SuiAddress>>;
}

#[async_trait::async_trait]
impl TransactionalAdapter for Simulacrum<StdRng, InMemoryStore> {
    async fn execute_txn(
        &mut self,
        transaction: Transaction,
    ) -> anyhow::Result<(TransactionEffects, Option<ExecutionError>)> {
        Ok(self.execute_transaction(transaction)?)
    }

    async fn dev_inspect_transaction_block(
        &self,
        _sender: SuiAddress,
        _transaction_kind: TransactionKind,
        _gas_price: Option<u64>,
    ) -> SuiResult<DevInspectResults> {
        unimplemented!("dev_inspect_transaction_block not supported in simulator mode")
    }

    async fn query_tx_events_asc(
        &self,
        tx_digest: &TransactionDigest,
        _limit: usize,
    ) -> SuiResult<Vec<Event>> {
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

    async fn advance_epoch(&mut self, create_random_state: bool) -> anyhow::Result<()> {
        self.advance_epoch(create_random_state);
        Ok(())
    }

    async fn request_gas(
        &mut self,
        address: SuiAddress,
        amount: u64,
    ) -> anyhow::Result<TransactionEffects> {
        self.request_gas(address, amount)
    }

    async fn get_active_validator_addresses(&self) -> SuiResult<Vec<SuiAddress>> {
        // TODO: this is a hack to get the validator addresses. Currently using start state
        //       but we should have a better way to get this information after reconfig
        Ok(self.epoch_start_state().get_validator_addresses())
    }
}
