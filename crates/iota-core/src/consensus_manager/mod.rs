// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use fastcrypto::traits::KeyPair as _;
use iota_config::{ConsensusConfig, NodeConfig, node::ConsensusProtocol};
use iota_metrics::RegistryService;
use iota_protocol_config::{ConsensusChoice, ProtocolVersion};
use iota_types::{committee::EpochId, error::IotaResult, messages_consensus::ConsensusTransaction};
use prometheus::{IntGauge, Registry, register_int_gauge_with_registry};
use tokio::{
    sync::{Mutex, MutexGuard},
    time::{sleep, timeout},
};
use tracing::info;

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    consensus_adapter::{BlockStatusReceiver, ConsensusClient},
    consensus_handler::ConsensusHandlerInitializer,
    consensus_manager::{mysticeti_manager::MysticetiManager, starfish_manager::StarfishManager},
    consensus_validator::IotaTxValidator,
    mysticeti_adapter::LazyMysticetiClient,
    starfish_adapter::LazyStarfishClient,
};

pub mod mysticeti_manager;
pub mod starfish_manager;

#[derive(PartialEq)]
pub(crate) enum Running {
    True(EpochId, ProtocolVersion),
    False,
}

#[async_trait]
#[enum_dispatch(ProtocolManager)]
pub trait ConsensusManagerTrait {
    async fn start(
        &self,
        node_config: &NodeConfig,
        epoch_store: Arc<AuthorityPerEpochStore>,
        consensus_handler_initializer: ConsensusHandlerInitializer,
        tx_validator: IotaTxValidator,
    );

    async fn shutdown(&self);

    async fn is_running(&self) -> bool;
}

// Wraps the underlying consensus protocol managers to make calling
// the ConsensusManagerTrait easier.
#[enum_dispatch]
enum ProtocolManager {
    Mysticeti(MysticetiManager),
    Starfish(StarfishManager),
}

impl ProtocolManager {
    /// Creates a new mysticeti manager.
    pub fn new_mysticeti(
        config: &NodeConfig,
        consensus_config: &ConsensusConfig,
        registry_service: &RegistryService,
        metrics: Arc<ConsensusManagerMetrics>,
        client: Arc<LazyMysticetiClient>,
    ) -> Self {
        Self::Mysticeti(MysticetiManager::new(
            config.protocol_key_pair().copy(),
            config.network_key_pair().copy(),
            consensus_config.db_path().to_path_buf(),
            registry_service.clone(),
            metrics,
            client,
        ))
    }

    /// Creates a new starfish manager.
    pub fn new_starfish(
        config: &NodeConfig,
        consensus_config: &ConsensusConfig,
        registry_service: &RegistryService,
        metrics: Arc<ConsensusManagerMetrics>,
        client: Arc<LazyStarfishClient>,
    ) -> Self {
        Self::Starfish(StarfishManager::new(
            config.protocol_key_pair().copy(),
            config.network_key_pair().copy(),
            consensus_config.db_path().to_path_buf(),
            registry_service.clone(),
            metrics,
            client,
        ))
    }
}

/// Used by IOTA validator to start consensus protocol for each epoch.
pub struct ConsensusManager {
    consensus_config: ConsensusConfig,
    mysticeti_manager: ProtocolManager,
    starfish_manager: ProtocolManager,
    mysticeti_client: Arc<LazyMysticetiClient>,
    starfish_client: Arc<LazyStarfishClient>,
    active: parking_lot::Mutex<Vec<bool>>,
    consensus_client: Arc<UpdatableConsensusClient>,
}

impl ConsensusManager {
    pub fn new(
        node_config: &NodeConfig,
        consensus_config: &ConsensusConfig,
        registry_service: &RegistryService,
        metrics_registry: &Registry,
        consensus_client: Arc<UpdatableConsensusClient>,
    ) -> Self {
        let metrics = Arc::new(ConsensusManagerMetrics::new(metrics_registry));
        let mysticeti_client = Arc::new(LazyMysticetiClient::new());
        let mysticeti_manager = ProtocolManager::new_mysticeti(
            node_config,
            consensus_config,
            registry_service,
            metrics.clone(),
            mysticeti_client.clone(),
        );
        let starfish_client = Arc::new(LazyStarfishClient::new());
        let starfish_manager = ProtocolManager::new_starfish(
            node_config,
            consensus_config,
            registry_service,
            metrics,
            starfish_client.clone(),
        );
        Self {
            consensus_config: consensus_config.clone(),
            mysticeti_manager,
            starfish_manager,
            mysticeti_client,
            starfish_client,
            active: parking_lot::Mutex::new(vec![false; 2]),
            consensus_client,
        }
    }

    pub fn get_storage_base_path(&self) -> PathBuf {
        self.consensus_config.db_path().to_path_buf()
    }

    // Picks the consensus protocol based on the protocol config and the epoch.
    fn pick_protocol(&self, epoch_store: &AuthorityPerEpochStore) -> ConsensusProtocol {
        let protocol_config = epoch_store.protocol_config();
        if let Ok(consensus_choice) = std::env::var("CONSENSUS_PROTOCOL") {
            match consensus_choice.to_lowercase().as_str() {
                "mysticeti" => return ConsensusProtocol::Mysticeti,
                "starfish" => return ConsensusProtocol::Starfish,
                "swap_each_epoch" => {
                    let protocol = if epoch_store.epoch().is_multiple_of(2) {
                        ConsensusProtocol::Starfish
                    } else {
                        ConsensusProtocol::Mysticeti
                    };
                    return protocol;
                }
                _ => {
                    info!(
                        "Invalid consensus choice {} in env var. Continue to pick consensus with protocol config",
                        consensus_choice
                    );
                }
            };
        }

        match protocol_config.consensus_choice() {
            ConsensusChoice::Mysticeti => ConsensusProtocol::Mysticeti,
            ConsensusChoice::Starfish => ConsensusProtocol::Starfish,
        }
    }
}

#[async_trait]
impl ConsensusManagerTrait for ConsensusManager {
    async fn start(
        &self,
        node_config: &NodeConfig,
        epoch_store: Arc<AuthorityPerEpochStore>,
        consensus_handler_initializer: ConsensusHandlerInitializer,
        tx_validator: IotaTxValidator,
    ) {
        let protocol_manager = {
            let mut active = self.active.lock();
            active.iter().enumerate().for_each(|(index, active)| {
                assert!(
                    !*active,
                    "Cannot start consensus. ConsensusManager protocol {index} is already running"
                );
            });
            let protocol = self.pick_protocol(&epoch_store);
            info!("Starting consensus protocol {protocol:?} ...");
            match protocol {
                ConsensusProtocol::Mysticeti => {
                    active[0] = true;
                    self.consensus_client.set(self.mysticeti_client.clone());
                    &self.mysticeti_manager
                }
                ConsensusProtocol::Starfish => {
                    active[1] = true;
                    self.consensus_client.set(self.starfish_client.clone());
                    &self.starfish_manager
                }
            }
        };

        protocol_manager
            .start(
                node_config,
                epoch_store,
                consensus_handler_initializer,
                tx_validator,
            )
            .await
    }

    async fn shutdown(&self) {
        info!("Shutting down consensus ...");
        let prev_active = {
            let mut active = self.active.lock();
            std::mem::replace(&mut *active, vec![false; 2])
        };
        if prev_active[0] {
            self.mysticeti_manager.shutdown().await;
        }

        if prev_active[1] {
            self.starfish_manager.shutdown().await;
        }

        self.consensus_client.clear();
    }

    async fn is_running(&self) -> bool {
        let active = self.active.lock();
        active.iter().any(|i| *i)
    }
}

/// A ConsensusClient that can be updated internally at any time. This usually
/// happening during epoch change where a client is set after the new consensus
/// is started for the new epoch.
#[derive(Default)]
pub struct UpdatableConsensusClient {
    // An extra layer of Arc<> is needed as required by ArcSwapAny.
    client: ArcSwapOption<Arc<dyn ConsensusClient>>,
}

impl UpdatableConsensusClient {
    pub fn new() -> Self {
        Self {
            client: ArcSwapOption::empty(),
        }
    }

    async fn get(&self) -> Arc<Arc<dyn ConsensusClient>> {
        const START_TIMEOUT: Duration = Duration::from_secs(30);
        const RETRY_INTERVAL: Duration = Duration::from_millis(100);
        if let Ok(client) = timeout(START_TIMEOUT, async {
            loop {
                let Some(client) = self.client.load_full() else {
                    sleep(RETRY_INTERVAL).await;
                    continue;
                };
                return client;
            }
        })
        .await
        {
            return client;
        }

        panic!("Timed out after {START_TIMEOUT:?} waiting for Consensus to start!",);
    }

    pub fn set(&self, client: Arc<dyn ConsensusClient>) {
        self.client.store(Some(Arc::new(client)));
    }

    pub fn clear(&self) {
        self.client.store(None);
    }
}

#[async_trait]
impl ConsensusClient for UpdatableConsensusClient {
    async fn submit(
        &self,
        transactions: &[ConsensusTransaction],
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<BlockStatusReceiver> {
        let client = self.get().await;
        client.submit(transactions, epoch_store).await
    }
}

pub struct ConsensusManagerMetrics {
    start_latency: IntGauge,
    shutdown_latency: IntGauge,
}

impl ConsensusManagerMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            start_latency: register_int_gauge_with_registry!(
                "consensus_manager_start_latency",
                "The latency of starting up consensus nodes",
                registry,
            )
            .unwrap(),
            shutdown_latency: register_int_gauge_with_registry!(
                "consensus_manager_shutdown_latency",
                "The latency of shutting down consensus nodes",
                registry,
            )
            .unwrap(),
        }
    }
}

pub(crate) struct RunningLockGuard<'a> {
    state_guard: MutexGuard<'a, Running>,
    metrics: &'a ConsensusManagerMetrics,
    epoch: Option<EpochId>,
    protocol_version: Option<ProtocolVersion>,
    start: Instant,
}

impl<'a> RunningLockGuard<'a> {
    pub(crate) async fn acquire_start(
        metrics: &'a ConsensusManagerMetrics,
        running_mutex: &'a Mutex<Running>,
        epoch: EpochId,
        version: ProtocolVersion,
    ) -> Option<RunningLockGuard<'a>> {
        let running = running_mutex.lock().await;
        if let Running::True(epoch, version) = *running {
            tracing::warn!(
                "Consensus is already Running for epoch {epoch:?} & protocol version {version:?} - shutdown first before starting",
            );
            return None;
        }

        tracing::info!("Starting up consensus for epoch {epoch:?} & protocol version {version:?}");

        Some(RunningLockGuard {
            state_guard: running,
            metrics,
            start: Instant::now(),
            epoch: Some(epoch),
            protocol_version: Some(version),
        })
    }

    pub(crate) async fn acquire_shutdown(
        metrics: &'a ConsensusManagerMetrics,
        running_mutex: &'a Mutex<Running>,
    ) -> Option<RunningLockGuard<'a>> {
        let running = running_mutex.lock().await;
        if let Running::True(epoch, version) = *running {
            tracing::info!(
                "Shutting down consensus for epoch {epoch:?} & protocol version {version:?}"
            );
        } else {
            tracing::warn!("Consensus shutdown was called but consensus is not running");
            return None;
        }

        Some(RunningLockGuard {
            state_guard: running,
            metrics,
            start: Instant::now(),
            epoch: None,
            protocol_version: None,
        })
    }
}

impl Drop for RunningLockGuard<'_> {
    fn drop(&mut self) {
        match *self.state_guard {
            // consensus was running and now will have to be marked as shutdown
            Running::True(epoch, version) => {
                tracing::info!(
                    "Consensus shutdown for epoch {epoch:?} & protocol version {version:?} is complete - took {} seconds",
                    self.start.elapsed().as_secs_f64()
                );

                self.metrics
                    .shutdown_latency
                    .set(self.start.elapsed().as_secs_f64() as i64);

                *self.state_guard = Running::False;
            }
            // consensus was not running and now will be marked as started
            Running::False => {
                tracing::info!(
                    "Starting up consensus for epoch {} & protocol version {:?} is complete - took {} seconds",
                    self.epoch.unwrap(),
                    self.protocol_version.unwrap(),
                    self.start.elapsed().as_secs_f64()
                );

                self.metrics
                    .start_latency
                    .set(self.start.elapsed().as_secs_f64() as i64);

                *self.state_guard =
                    Running::True(self.epoch.unwrap(), self.protocol_version.unwrap());
            }
        }
    }
}
