// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anemo::codegen::InboundRequestLayer;
use anemo_tower::{inflight_limit, rate_limit};
use iota_config::{node::CheckpointArchiveConfig, p2p::StateSyncConfig};
use iota_types::{
    messages_checkpoint::VerifiedCheckpoint,
    storage::{ApplyCheckpointResults, WriteStore},
};
use tap::Pipe;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};

use super::{
    Handle, PeerHeights, StateSync, StateSyncEventLoop, StateSyncMessage, StateSyncServer,
    metrics::Metrics,
    server::{CheckpointContentsDownloadLimitLayer, Server},
};

pub struct Builder<S> {
    store: Option<S>,
    config: Option<StateSyncConfig>,
    metrics: Option<Metrics>,
    checkpoint_archive_config: Option<CheckpointArchiveConfig>,
    results_applier: Option<Arc<dyn ApplyCheckpointResults>>,
}

impl Builder<()> {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            store: None,
            config: None,
            metrics: None,
            checkpoint_archive_config: None,
            results_applier: None,
        }
    }
}

impl<S> Builder<S> {
    pub fn store<NewStore>(self, store: NewStore) -> Builder<NewStore> {
        Builder {
            store: Some(store),
            config: self.config,
            metrics: self.metrics,
            checkpoint_archive_config: self.checkpoint_archive_config,
            results_applier: self.results_applier,
        }
    }

    pub fn config(mut self, config: StateSyncConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_metrics(mut self, registry: &prometheus_filtered::Registry) -> Self {
        self.metrics = Some(Metrics::enabled(registry));
        self
    }

    pub fn checkpoint_archive_config(
        mut self,
        checkpoint_archive_config: Option<CheckpointArchiveConfig>,
    ) -> Self {
        self.checkpoint_archive_config = checkpoint_archive_config;
        self
    }

    /// Sets what writes the results of checkpoints downloaded from the archive,
    /// so their transactions do not have to be executed. Leave unset to have
    /// the checkpoint executor produce the results instead.
    pub fn results_applier(
        mut self,
        results_applier: Option<Arc<dyn ApplyCheckpointResults>>,
    ) -> Self {
        self.results_applier = results_applier;
        self
    }
}

impl<S> Builder<S>
where
    S: WriteStore + Clone + Send + Sync + 'static,
{
    pub fn build(self) -> (UnstartedStateSync<S>, StateSyncServer<impl StateSync>) {
        let state_sync_config = self.config.clone().unwrap_or_default();
        let (mut builder, server) = self.build_internal();
        let mut state_sync_server = StateSyncServer::new(server);

        // Apply rate limits from configuration as needed.
        if let Some(limit) = state_sync_config.push_checkpoint_summary_rate_limit {
            state_sync_server = state_sync_server.add_layer_for_push_checkpoint_summary(
                InboundRequestLayer::new(rate_limit::RateLimitLayer::new(
                    governor::Quota::per_second(limit),
                    rate_limit::WaitMode::Block,
                )),
            );
        }
        if let Some(limit) = state_sync_config.get_checkpoint_summary_rate_limit {
            state_sync_server = state_sync_server.add_layer_for_get_checkpoint_summary(
                InboundRequestLayer::new(rate_limit::RateLimitLayer::new(
                    governor::Quota::per_second(limit),
                    rate_limit::WaitMode::Block,
                )),
            );
        }
        if let Some(limit) = state_sync_config.get_checkpoint_contents_rate_limit {
            state_sync_server = state_sync_server.add_layer_for_get_checkpoint_contents(
                InboundRequestLayer::new(rate_limit::RateLimitLayer::new(
                    governor::Quota::per_second(limit),
                    rate_limit::WaitMode::Block,
                )),
            );
        }
        if let Some(limit) = state_sync_config.get_checkpoint_contents_inflight_limit {
            state_sync_server = state_sync_server.add_layer_for_get_checkpoint_contents(
                InboundRequestLayer::new(inflight_limit::InflightLimitLayer::new(
                    limit,
                    inflight_limit::WaitMode::ReturnError,
                )),
            );
        }
        if let Some(limit) = state_sync_config.get_checkpoint_contents_per_checkpoint_limit {
            let layer = CheckpointContentsDownloadLimitLayer::new(limit);
            builder.download_limit_layer = Some(layer.clone());
            state_sync_server = state_sync_server
                .add_layer_for_get_checkpoint_contents(InboundRequestLayer::new(layer));
        }

        (builder, state_sync_server)
    }

    pub(super) fn build_internal(self) -> (UnstartedStateSync<S>, Server<S>) {
        let Builder {
            store,
            config,
            metrics,
            checkpoint_archive_config,
            results_applier,
        } = self;
        let store = store.unwrap();
        let config = config.unwrap_or_default();
        let metrics = metrics.unwrap_or_else(Metrics::disabled);

        let (sender, mailbox) = mpsc::channel(config.mailbox_capacity());
        let (checkpoint_event_sender, _receiver) =
            broadcast::channel(config.synced_checkpoint_broadcast_channel_capacity());
        let weak_sender = sender.downgrade();
        let handle = Handle {
            sender,
            checkpoint_event_sender: checkpoint_event_sender.clone(),
        };
        let peer_heights = PeerHeights {
            peers: HashMap::new(),
            unprocessed_checkpoints: HashMap::new(),
            sequence_number_to_digest: HashMap::new(),
            wait_interval_when_no_peer_to_sync_content: config
                .wait_interval_when_no_peer_to_sync_content(),
        }
        .pipe(RwLock::new)
        .pipe(Arc::new);

        let genesis_checkpoint = Arc::new(
            store
                .get_checkpoint_by_sequence_number(0)
                .expect("store should contain genesis checkpoint before building state sync"),
        );

        let server = Server {
            store: store.clone(),
            peer_heights: peer_heights.clone(),
            sender: weak_sender,
            genesis_checkpoint: genesis_checkpoint.clone(),
        };

        (
            UnstartedStateSync {
                config,
                handle,
                mailbox,
                store,
                download_limit_layer: None,
                peer_heights,
                checkpoint_event_sender,
                metrics,
                checkpoint_archive_config,
                results_applier,
                genesis_checkpoint,
            },
            server,
        )
    }
}

pub struct UnstartedStateSync<S> {
    pub(super) config: StateSyncConfig,
    pub(super) handle: Handle,
    pub(super) mailbox: mpsc::Receiver<StateSyncMessage>,
    pub(super) download_limit_layer: Option<CheckpointContentsDownloadLimitLayer>,
    pub(super) store: S,
    pub(super) peer_heights: Arc<RwLock<PeerHeights>>,
    pub(super) checkpoint_event_sender: broadcast::Sender<VerifiedCheckpoint>,
    pub(super) metrics: Metrics,
    pub(super) checkpoint_archive_config: Option<CheckpointArchiveConfig>,
    pub(super) results_applier: Option<Arc<dyn ApplyCheckpointResults>>,
    /// Cached genesis checkpoint, shared with the RPC server.
    pub(super) genesis_checkpoint: Arc<VerifiedCheckpoint>,
}

impl<S> UnstartedStateSync<S>
where
    S: WriteStore + Clone + Send + Sync + 'static,
{
    pub(super) fn build(self, network: anemo::Network) -> (StateSyncEventLoop<S>, Handle) {
        let Self {
            config,
            handle,
            mailbox,
            download_limit_layer,
            store,
            peer_heights,
            checkpoint_event_sender,
            metrics,
            checkpoint_archive_config,
            results_applier,
            genesis_checkpoint,
        } = self;

        (
            StateSyncEventLoop {
                config,
                mailbox,
                weak_sender: handle.sender.downgrade(),
                tasks: JoinSet::new(),
                sync_checkpoint_summaries_task: None,
                sync_checkpoint_contents_task: None,
                download_limit_layer,
                store,
                peer_heights,
                checkpoint_event_sender,
                network,
                metrics,
                checkpoint_archive_config,
                results_applier,
                sync_checkpoint_from_archive_task: None,
                genesis_checkpoint,
            },
            handle,
        )
    }

    pub fn start(self, network: anemo::Network) -> Handle {
        let (event_loop, handle) = self.build(network);
        tokio::spawn(event_loop.start());

        handle
    }
}
