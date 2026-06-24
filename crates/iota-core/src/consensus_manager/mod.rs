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
use fastcrypto::traits::KeyPair as _;
use iota_config::{ConsensusConfig, NodeConfig};
use iota_metrics::{RegistryID, RegistryService};
use iota_protocol_config::ProtocolVersion;
use iota_types::{committee::EpochId, error::IotaResult, messages_consensus::ConsensusTransaction};
use prometheus::{IntGauge, Registry, register_int_gauge_with_registry};
use starfish_core::ConsensusAuthority;
use tokio::{
    sync::{Mutex, MutexGuard},
    time::{sleep, timeout},
};
use tracing::info;

use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    consensus_adapter::{BlockStatusReceiver, ConsensusClient},
    consensus_handler::ConsensusHandlerInitializer,
    consensus_manager::starfish_manager::StarfishManager,
    consensus_validator::IotaTxValidator,
    starfish_adapter::LazyStarfishClient,
};

pub mod starfish_manager;

#[derive(PartialEq)]
pub(crate) enum Running {
    True(EpochId, ProtocolVersion),
    False,
}

#[async_trait]
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

    fn replay_waiter(&self) -> Option<ReplayWaiter>;
}

/// Used by IOTA validator to start consensus protocol for each epoch.
pub struct ConsensusManager {
    consensus_config: ConsensusConfig,
    starfish_manager: StarfishManager,
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
        let starfish_client = Arc::new(LazyStarfishClient::new());
        consensus_client.set(starfish_client.clone());
        let starfish_manager = StarfishManager::new(
            node_config.protocol_key_pair().copy(),
            node_config.network_key_pair().copy(),
            consensus_config.db_path().to_path_buf(),
            registry_service.clone(),
            metrics,
            starfish_client,
        );
        Self {
            consensus_config: consensus_config.clone(),
            starfish_manager,
        }
    }

    pub fn get_storage_base_path(&self) -> PathBuf {
        self.consensus_config.db_path().to_path_buf()
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
        info!("Starting consensus protocol Starfish ...");
        self.starfish_manager
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
        self.starfish_manager.shutdown().await;
    }

    async fn is_running(&self) -> bool {
        self.starfish_manager.is_running().await
    }

    fn replay_waiter(&self) -> Option<ReplayWaiter> {
        self.starfish_manager.replay_waiter()
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

#[derive(Clone)]
pub struct ReplayWaiter {
    authority: Arc<(ConsensusAuthority, RegistryID)>,
}

impl ReplayWaiter {
    pub(crate) fn new(authority: Arc<(ConsensusAuthority, RegistryID)>) -> Self {
        Self { authority }
    }

    pub(crate) async fn wait_for_replay(&self) {
        self.authority.0.replay_complete().await;
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
