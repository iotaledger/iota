// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use fastcrypto::{bls12381, traits::KeyPair};
use iota_metrics::RegistryService;
use iota_swarm_config::network_config_builder::ConfigBuilder;
use iota_types::{
    iota_system_state::{
        IotaSystemStateTrait, epoch_start_iota_system_state::EpochStartSystemStateTrait,
    },
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary},
};
use narwhal_config::{Epoch, WorkerCache};
use narwhal_types::{TransactionProto, TransactionsClient};
use prometheus::Registry;
use tokio::{
    sync::{broadcast, mpsc},
    time::{interval, sleep},
};

use crate::{
    authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder},
    checkpoints::{CheckpointMetrics, CheckpointService, CheckpointServiceNoop},
    consensus_handler::ConsensusHandlerInitializer,
    consensus_manager::{
        ConsensusManagerMetrics, ConsensusManagerTrait,
        narwhal_manager::{NarwhalConfiguration, NarwhalManager},
    },
    consensus_validator::{IotaTxValidator, IotaTxValidatorMetrics},
    state_accumulator::StateAccumulator,
};

async fn send_transactions(
    name: &bls12381::min_sig::BLS12381PublicKey,
    worker_cache: WorkerCache,
    epoch: Epoch,
    mut rx_shutdown: broadcast::Receiver<()>,
) {
    let target = worker_cache
        .worker(name, /* id */ &0)
        .expect("Our key or worker id is not in the worker cache")
        .transactions;
    let config = iota_network_stack::config::Config::new();
    let channel = config.connect_lazy(&target).unwrap();
    let mut client = TransactionsClient::new(channel);
    // Make a transaction to submit forever.
    let tx = TransactionProto {
        transactions: vec![Bytes::from(epoch.to_be_bytes().to_vec())],
    };
    // Repeatedly send transactions.
    let interval = interval(Duration::from_millis(1));

    tokio::pin!(interval);
    let mut succeeded_once = false;
    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Send a transactions.
                let result = client.submit_transaction(tx.clone()).await;
                if result.is_ok() {
                    succeeded_once = true;
                }

            },
            _ = rx_shutdown.recv() => {
                break
            }
        }
    }
    assert!(succeeded_once);
}

pub fn checkpoint_service_for_testing(state: Arc<AuthorityState>) -> Arc<CheckpointService> {
    let (output, _result) = mpsc::channel::<(CheckpointContents, CheckpointSummary)>(10);
    let epoch_store = state.epoch_store_for_testing();
    let accumulator = Arc::new(StateAccumulator::new_for_tests(
        state.get_accumulator_store().clone(),
        &epoch_store,
    ));
    let (certified_output, _certified_result) = mpsc::channel::<CertifiedCheckpointSummary>(10);

    let (checkpoint_service, _) = CheckpointService::spawn(
        state.clone(),
        state.get_checkpoint_store().clone(),
        epoch_store.clone(),
        state.get_transaction_cache_reader().clone(),
        Arc::downgrade(&accumulator),
        Box::new(output),
        Box::new(certified_output),
        CheckpointMetrics::new_for_tests(),
        3,
        100_000,
    );
    checkpoint_service
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_narwhal_manager() {
    let configs = ConfigBuilder::new_with_temp_dir()
        .committee_size(1.try_into().unwrap())
        .build();
    let mut narwhal_managers = Vec::new();
    let mut shutdown_senders = Vec::new();

    for config in configs.validator_configs() {
        let consensus_config = config.consensus_config().unwrap();
        let registry_service = RegistryService::new(Registry::new());
        let secret = Arc::pin(config.protocol_key_pair().copy());
        let genesis = config.genesis().unwrap();

        let state = TestAuthorityBuilder::new()
            .with_genesis_and_keypair(genesis, &secret)
            .build()
            .await;

        let system_state = state
            .get_iota_system_state_object_for_testing()
            .expect("Reading Iota system state object cannot fail")
            .into_epoch_start_state();

        let transactions_addr = &config.consensus_config.as_ref().unwrap().address;
        let narwhal_committee = system_state.get_narwhal_committee();
        let worker_cache = system_state.get_narwhal_worker_cache(transactions_addr);

        let narwhal_config = NarwhalConfiguration {
            primary_keypair: config.protocol_key_pair().copy(),
            network_keypair: config.network_key_pair().copy(),
            worker_ids_and_keypairs: vec![(0, config.worker_key_pair().copy())],
            storage_base_path: consensus_config.db_path().to_path_buf(),
            parameters: consensus_config.narwhal_config().to_owned(),
            registry_service,
        };

        let metrics = Arc::new(ConsensusManagerMetrics::new(&Registry::new()));
        let epoch_store = state.epoch_store_for_testing();

        let narwhal_manager = NarwhalManager::new(narwhal_config, metrics);

        let consensus_handler_initializer = ConsensusHandlerInitializer::new_for_testing(
            state.clone(),
            checkpoint_service_for_testing(state.clone()),
        );

        // start narwhal
        narwhal_manager
            .start(
                config,
                epoch_store.clone(),
                consensus_handler_initializer,
                IotaTxValidator::new(
                    epoch_store.clone(),
                    Arc::new(CheckpointServiceNoop {}),
                    state.transaction_manager().clone(),
                    IotaTxValidatorMetrics::new(&Registry::new()),
                ),
            )
            .await;

        assert!(narwhal_manager.is_running().await);

        let name = config.protocol_key_pair().public().clone();
        narwhal_managers.push((
            narwhal_manager,
            state,
            transactions_addr.clone(),
            name.clone(),
        ));

        // Send some transactions
        let (tx_shutdown, rx_shutdown) = broadcast::channel(1);
        tokio::spawn(async move {
            send_transactions(
                &name,
                worker_cache.clone(),
                narwhal_committee.epoch(),
                rx_shutdown,
            )
            .await
        });
        shutdown_senders.push(tx_shutdown);
    }

    sleep(Duration::from_secs(1)).await;
    for tr_shutdown in shutdown_senders {
        _ = tr_shutdown.send(());
    }
    let mut shutdown_senders = Vec::new();

    for ((narwhal_manager, state, transactions_addr, name), config) in narwhal_managers
        .into_iter()
        .zip(configs.validator_configs())
    {
        // stop narwhal instance
        narwhal_manager.shutdown().await;

        // ensure that no primary or worker node is running
        assert!(!narwhal_manager.is_running().await);
        assert!(!narwhal_manager.primary_node.is_running().await);
        assert!(
            narwhal_manager
                .worker_nodes
                .workers_running()
                .await
                .is_empty()
        );

        let system_state = state
            .get_iota_system_state_object_for_testing()
            .expect("Reading Iota system state object cannot fail")
            .into_epoch_start_state();
        let narwhal_committee = system_state.get_narwhal_committee();
        let worker_cache = system_state.get_narwhal_worker_cache(&transactions_addr);

        let epoch_store = state.epoch_store_for_testing();

        let consensus_handler_initializer = ConsensusHandlerInitializer::new_for_testing(
            state.clone(),
            checkpoint_service_for_testing(state.clone()),
        );

        // start narwhal with advanced epoch
        narwhal_manager
            .start(
                config,
                epoch_store.clone(),
                consensus_handler_initializer,
                IotaTxValidator::new(
                    epoch_store.clone(),
                    Arc::new(CheckpointServiceNoop {}),
                    state.transaction_manager().clone(),
                    IotaTxValidatorMetrics::new(&Registry::new()),
                ),
            )
            .await;

        // Send some transactions
        let (tr_shutdown, rx_shutdown) = broadcast::channel(1);
        tokio::spawn(async move {
            send_transactions(
                &name,
                worker_cache.clone(),
                narwhal_committee.epoch(),
                rx_shutdown,
            )
            .await
        });

        shutdown_senders.push(tr_shutdown);
    }
    sleep(Duration::from_secs(5)).await;
    for tr_shutdown in shutdown_senders {
        _ = tr_shutdown.send(());
    }
}
