// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, sync::Arc};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use fastcrypto::ed25519;
use iota_config::NodeConfig;
use iota_metrics::{RegistryID, RegistryService, monitored_mpsc::unbounded_channel};
use iota_types::{
    committee::EpochId,
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemStateTrait,
};
use prometheus::Registry;
use starfish_config::{Committee, NetworkKeyPair, Parameters, ProtocolKeyPair};
use starfish_core::{
    Clock, CommitConsumer, CommitConsumerMonitor, CommitIndex, ConsensusAuthority,
};
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    consensus_handler::{ConsensusHandlerInitializer, StarfishConsensusHandler},
    consensus_manager::{
        ConsensusManagerMetrics, ConsensusManagerTrait, Running, RunningLockGuard,
    },
    consensus_validator::IotaTxValidator,
    starfish_adapter::LazyStarfishClient,
};

#[cfg(test)]
#[path = "../unit_tests/starfish_manager_tests.rs"]
pub mod starfish_manager_tests;

pub struct StarfishManager {
    protocol_keypair: ProtocolKeyPair,
    network_keypair: NetworkKeyPair,
    storage_base_path: PathBuf,
    running: Mutex<Running>,
    metrics: Arc<ConsensusManagerMetrics>,
    registry_service: RegistryService,
    authority: ArcSwapOption<(ConsensusAuthority, RegistryID)>,
    boot_counter: Mutex<u64>,
    // Use a shared lazy starfish client so we can update the internal starfish
    // client that gets created for every new epoch.
    client: Arc<LazyStarfishClient>,
    consensus_handler: Mutex<Option<StarfishConsensusHandler>>,
    consumer_monitor: ArcSwapOption<CommitConsumerMonitor>,
}

impl StarfishManager {
    /// NOTE: Starfish protocol key uses Ed25519 instead of BLS.
    /// But for security, the protocol keypair must be different from the
    /// network keypair.
    pub fn new(
        protocol_keypair: ed25519::Ed25519KeyPair,
        network_keypair: ed25519::Ed25519KeyPair,
        storage_base_path: PathBuf,
        registry_service: RegistryService,
        metrics: Arc<ConsensusManagerMetrics>,
        client: Arc<LazyStarfishClient>,
    ) -> Self {
        Self {
            protocol_keypair: ProtocolKeyPair::new(protocol_keypair),
            network_keypair: NetworkKeyPair::new(network_keypair),
            storage_base_path,
            running: Mutex::new(Running::False),
            metrics,
            registry_service,
            authority: ArcSwapOption::empty(),
            client,
            consensus_handler: Mutex::new(None),
            boot_counter: Mutex::new(0),
            consumer_monitor: ArcSwapOption::empty(),
        }
    }

    fn get_store_path(&self, epoch: EpochId) -> PathBuf {
        let mut store_path = self.storage_base_path.clone();
        store_path.push(format!("{epoch}"));
        store_path
    }
}

#[async_trait]

impl ConsensusManagerTrait for StarfishManager {
    /// Starts the Starfish consensus manager for the current epoch.
    async fn start(
        &self,
        config: &NodeConfig,
        epoch_store: Arc<AuthorityPerEpochStore>,
        consensus_handler_initializer: ConsensusHandlerInitializer,
        tx_validator: IotaTxValidator,
    ) {
        let system_state = epoch_store.epoch_start_state();
        let committee: Committee = system_state.get_starfish_committee();
        let epoch = epoch_store.epoch();
        let protocol_config = epoch_store.protocol_config();

        let Some(_guard) = RunningLockGuard::acquire_start(
            &self.metrics,
            &self.running,
            epoch,
            protocol_config.version,
        )
        .await
        else {
            return;
        };

        let consensus_config = config
            .consensus_config()
            .expect("consensus_config should exist");

        let parameters = Parameters {
            db_path: self.get_store_path(epoch),
            ..consensus_config
                .starfish_parameters
                .clone()
                .unwrap_or_default()
        };

        let own_protocol_key = self.protocol_keypair.public();
        let (own_index, _) = committee
            .authorities()
            .find(|(_, a)| a.protocol_key == own_protocol_key)
            .expect("Own authority should be among the consensus authorities!");

        let registry = Registry::new_custom(Some("consensus".to_string()), None).unwrap();

        let (commit_sender, commit_receiver) = unbounded_channel("consensus_output");

        let consensus_handler = consensus_handler_initializer.new_consensus_handler();

        let num_prior_commits = protocol_config.consensus_num_requested_prior_commits_at_startup();
        let last_processed_commit = consensus_handler.last_processed_subdag_index() as CommitIndex;
        let starting_commit = last_processed_commit.saturating_sub(num_prior_commits);
        let consumer = CommitConsumer::new(commit_sender, starting_commit);
        let monitor = consumer.monitor();

        // If there is a previous consumer monitor, it indicates that the consensus
        // engine has been restarted, due to an epoch change. However, that on its
        // own doesn't tell us much whether it participated on an active epoch or an old
        // one. We need to check if it has handled any commits to determine this.
        // If indeed any commits did happen, then we assume that node did participate on
        // previous run.
        let participated_on_previous_run =
            if let Some(previous_monitor) = self.consumer_monitor.swap(Some(monitor.clone())) {
                previous_monitor.highest_handled_commit() > 0
            } else {
                false
            };

        // Increment the boot counter only if the consensus successfully participated in
        // the previous run. This is typical during normal epoch changes, where
        // the node restarts as expected, and the boot counter is incremented to prevent
        // amnesia recovery on the next start. If the node is recovering from a
        // restore process and catching up across multiple epochs, it won't handle any
        // commits until it reaches the last active epoch. In this scenario, we
        // do not increment the boot counter, as we need amnesia recovery to run.
        let mut boot_counter = self.boot_counter.lock().await;
        if participated_on_previous_run {
            *boot_counter += 1;
        } else {
            info!(
                "Node has not participated in previous epoch consensus. Boot counter ({}) will not increment.",
                *boot_counter
            );
        }

        let authority = ConsensusAuthority::start(
            epoch_store.epoch_start_config().epoch_start_timestamp_ms(),
            own_index,
            committee.clone(),
            parameters.clone(),
            protocol_config.clone(),
            self.protocol_keypair.clone(),
            self.network_keypair.clone(),
            Arc::new(Clock::default()),
            Arc::new(tx_validator.clone()),
            consumer,
            registry.clone(),
            *boot_counter,
        )
        .await;
        let client = authority.transaction_client();

        let registry_id = self.registry_service.add(registry.clone());

        let registered_authority = Arc::new((authority, registry_id));
        self.authority.swap(Some(registered_authority.clone()));

        // Initialize the client to send transactions to this Starfish instance.
        self.client.set(client);

        // spin up the new starfish consensus handler to listen for committed sub dags
        let handler = StarfishConsensusHandler::new(
            last_processed_commit,
            consensus_handler,
            commit_receiver,
            monitor,
        );

        let mut consensus_handler = self.consensus_handler.lock().await;
        *consensus_handler = Some(handler);

        // Wait until all locally available commits have been processed
        info!("replaying commits at startup");
        registered_authority.0.replay_complete().await;
        info!("Startup commit replay complete");
    }

    async fn shutdown(&self) {
        let Some(_guard) = RunningLockGuard::acquire_shutdown(&self.metrics, &self.running).await
        else {
            return;
        };

        // Stop consensus submissions.
        self.client.clear();

        // swap with empty to ensure there is no other reference to authority and we can
        // safely do Arc unwrap
        let r = self.authority.swap(None).unwrap();
        let Ok((authority, registry_id)) = Arc::try_unwrap(r) else {
            panic!("Failed to retrieve the starfish authority");
        };

        // shutdown the authority and wait for it
        authority.stop().await;

        // drop the old consensus handler to force stop any underlying task running.
        let mut consensus_handler = self.consensus_handler.lock().await;
        if let Some(mut handler) = consensus_handler.take() {
            handler.abort().await;
        }

        // unregister the registry id
        self.registry_service.remove(registry_id);
    }

    async fn is_running(&self) -> bool {
        Running::False != *self.running.lock().await
    }
}
