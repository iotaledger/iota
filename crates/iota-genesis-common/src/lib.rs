// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use iota_execution::{self, Executor};
use iota_protocol_config::{ProtocolConfig, ProtocolVersion};
use iota_types::{
    base_types::{ObjectID, TxContext},
    digests::ChainIdentifier,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    epoch_data::EpochData,
    gas::IotaGasStatus,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    messages_checkpoint::CheckpointTimestamp,
    metrics::LimitsMetrics,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CheckedInputObjects, Command, InputObjectKind, ObjectReadResult, Transaction},
};
use move_binary_format::CompiledModule;

pub fn get_genesis_protocol_config(version: ProtocolVersion) -> ProtocolConfig {
    // We have a circular dependency here. Protocol config depends on chain ID,
    // which depends on genesis checkpoint (digest), which depends on genesis
    // transaction, which depends on protocol config.
    //
    // ChainIdentifier::default().chain() which can be overridden by the
    // IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE if necessary
    ProtocolConfig::get_for_version(version, ChainIdentifier::default().chain())
}

pub fn process_package(
    store: &mut InMemoryStorage,
    executor: &dyn Executor,
    ctx: &mut TxContext,
    modules: &[CompiledModule],
    dependencies: Vec<ObjectID>,
    protocol_config: &ProtocolConfig,
    metrics: Arc<LimitsMetrics>,
) -> anyhow::Result<TransactionEvents> {
    let dependency_objects = store.get_objects(&dependencies);
    // When publishing genesis packages, since the std framework packages all have
    // non-zero addresses, [`Transaction::input_objects_in_compiled_modules`] will
    // consider them as dependencies even though they are not. Hence
    // input_objects contain objects that don't exist on-chain because they are
    // yet to be published.
    #[cfg(debug_assertions)]
    {
        use move_core_types::account_address::AccountAddress;
        let to_be_published_addresses: HashSet<_> = modules
            .iter()
            .map(|module| *module.self_id().address())
            .collect();
        assert!(
            // An object either exists on-chain, or is one of the packages to be published.
            dependencies
                .iter()
                .zip(dependency_objects.iter())
                .all(|(dependency, obj_opt)| obj_opt.is_some()
                    || to_be_published_addresses.contains(&AccountAddress::from(*dependency)))
        );
    }
    let loaded_dependencies: Vec<_> = dependencies
        .iter()
        .zip(dependency_objects)
        .filter_map(|(dependency, object)| {
            Some(ObjectReadResult::new(
                InputObjectKind::MovePackage(*dependency),
                object?.clone().into(),
            ))
        })
        .collect();

    let module_bytes = modules
        .iter()
        .map(|m| {
            let mut buf = vec![];
            m.serialize_with_version(m.version, &mut buf).unwrap();
            buf
        })
        .collect();
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        // executing in Genesis mode does not create an `UpgradeCap`.
        builder.command(Command::Publish(module_bytes, dependencies));
        builder.finish()
    };
    let InnerTemporaryStore {
        written, events, ..
    } = executor.update_genesis_state(
        &*store,
        protocol_config,
        metrics,
        ctx,
        CheckedInputObjects::new_for_genesis(loaded_dependencies),
        pt,
    )?;

    store.finish(written);

    Ok(events)
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
