// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use iota_execution::{self};
use iota_protocol_config::{ProtocolConfig, ProtocolVersion};
use iota_types::{
    digests::ChainIdentifier,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    epoch_data::EpochData,
    gas::IotaGasStatus,
    in_memory_storage::InMemoryStorage,
    messages_checkpoint::CheckpointTimestamp,
    metrics::LimitsMetrics,
    object::Object,
    transaction::{CheckedInputObjects, Transaction},
};

pub fn get_genesis_protocol_config(version: ProtocolVersion) -> ProtocolConfig {
    // We have a circular dependency here. Protocol config depends on chain ID,
    // which depends on genesis checkpoint (digest), which depends on genesis
    // transaction, which depends on protocol config.
    //
    // ChainIdentifier::default().chain() which can be overridden by the
    // IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE if necessary
    ProtocolConfig::get_for_version(version, ChainIdentifier::default().chain())
}

pub fn prepare_and_execute_genesis_transaction(
    chain_start_timestamp_ms: CheckpointTimestamp,
    protocol_version: ProtocolVersion,
    genesis_transaction: &Transaction,
) -> (TransactionEffects, TransactionEvents, Vec<Object>) {
    let registry = prometheus::Registry::new();
    let metrics = Arc::new(LimitsMetrics::new(&registry));
    let epoch_data = EpochData::new_genesis(chain_start_timestamp_ms);
    let protocol_config = get_genesis_protocol_config(protocol_version);

    execute_genesis_transaction(&epoch_data, &protocol_config, metrics, genesis_transaction)
}

pub fn execute_genesis_transaction(
    epoch_data: &EpochData,
    protocol_config: &ProtocolConfig,
    metrics: Arc<LimitsMetrics>,
    genesis_transaction: &Transaction,
) -> (TransactionEffects, TransactionEvents, Vec<Object>) {
    let genesis_digest = *genesis_transaction.digest();
    // execute txn to effects
    let silent = true;

    let executor = iota_execution::executor(protocol_config, silent, None)
        .expect("Creating an executor should not fail here");

    let expensive_checks = false;
    let certificate_deny_set = HashSet::new();
    let transaction_data = &genesis_transaction.data().intent_message().value;
    let (kind, signer, _) = transaction_data.execution_parts();
    let input_objects = CheckedInputObjects::new_for_genesis(vec![]);
    let (inner_temp_store, _, effects, _execution_error) = executor.execute_transaction_to_effects(
        &InMemoryStorage::new(Vec::new()),
        protocol_config,
        metrics,
        expensive_checks,
        &certificate_deny_set,
        &epoch_data.epoch_id(),
        epoch_data.epoch_start_timestamp(),
        input_objects,
        vec![],
        IotaGasStatus::new_unmetered(),
        kind,
        signer,
        genesis_digest,
    );
    assert!(inner_temp_store.input_objects.is_empty());
    assert!(inner_temp_store.mutable_inputs.is_empty());
    assert!(effects.mutated().is_empty());
    assert!(effects.unwrapped().is_empty());
    assert!(effects.deleted().is_empty());
    assert!(effects.wrapped().is_empty());
    assert!(effects.unwrapped_then_deleted().is_empty());

    let objects = inner_temp_store.written.into_values().collect();
    (effects, inner_temp_store.events, objects)
}
