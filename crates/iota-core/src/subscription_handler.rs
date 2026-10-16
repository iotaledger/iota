// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_json_rpc_types::{
    EffectsWithInput, EventFilter, IotaEvent, IotaTransactionBlockEffects, TransactionFilter,
};
use iota_metrics::spawn_monitored_task;
use iota_sdk_types::Transaction;
use prometheus_filtered::{
    IntCounterVec, IntGaugeVec, Registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry,
};
use tokio::sync::mpsc::{self, error::TrySendError};
use tokio_stream::Stream;
use tracing::{error, warn};

use crate::streamer::Streamer;

#[cfg(test)]
#[path = "unit_tests/subscription_handler_tests.rs"]
mod subscription_handler_tests;

pub const EVENT_DISPATCH_BUFFER_SIZE: usize = 1000;

/// How many checkpoints [`SubscriptionHandler::notify_checkpoint`] queues
/// before it drops the subscribers.
const CHECKPOINT_QUEUE_SIZE: usize = 32;

/// What one transaction sends to the subscribers.
pub struct SubscriberUpdate {
    pub input: Transaction,
    pub effects: IotaTransactionBlockEffects,
    pub events: Vec<IotaEvent>,
}

/// Converts the transactions of one checkpoint for the subscribers, in
/// checkpoint order. Runs on a blocking thread, so it may read the database.
pub type CheckpointConversion = Box<dyn FnOnce() -> Vec<SubscriberUpdate> + Send>;

pub struct SubscriptionMetrics {
    pub streaming_success: IntCounterVec,
    pub streaming_failure: IntCounterVec,
    pub streaming_active_subscriber_number: IntGaugeVec,
    pub dropped_submissions: IntCounterVec,
}

impl SubscriptionMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            streaming_success: register_int_counter_vec_with_registry!(
                "streaming_success",
                "Total number of items that are streamed successfully",
                &["type"],
                registry,
            )
            .unwrap(),
            streaming_failure: register_int_counter_vec_with_registry!(
                "streaming_failure",
                "Total number of items that fail to be streamed",
                &["type"],
                registry,
            )
            .unwrap(),
            streaming_active_subscriber_number: register_int_gauge_vec_with_registry!(
                "streaming_active_subscriber_number",
                "Current number of active subscribers",
                &["type"],
                registry,
            )
            .unwrap(),
            dropped_submissions: register_int_counter_vec_with_registry!(
                "streaming_dropped_submissions",
                "Total number of submissions that are dropped",
                &["type"],
                registry,
            )
            .unwrap(),
        }
    }
}

type EventStreamer = Streamer<IotaEvent, IotaEvent, EventFilter>;
type TransactionStreamer =
    Streamer<EffectsWithInput, IotaTransactionBlockEffects, TransactionFilter>;

pub struct SubscriptionHandler {
    event_streamer: Arc<EventStreamer>,
    transaction_streamer: Arc<TransactionStreamer>,
    checkpoint_queue: mpsc::Sender<CheckpointConversion>,
    metrics: Arc<SubscriptionMetrics>,
}

impl SubscriptionHandler {
    pub fn new(registry: &Registry) -> Self {
        let metrics = Arc::new(SubscriptionMetrics::new(registry));
        let event_streamer = Arc::new(Streamer::spawn(
            EVENT_DISPATCH_BUFFER_SIZE,
            metrics.clone(),
            "event",
        ));
        let transaction_streamer = Arc::new(Streamer::spawn(
            EVENT_DISPATCH_BUFFER_SIZE,
            metrics.clone(),
            "tx",
        ));
        let (checkpoint_queue, checkpoints) = mpsc::channel(CHECKPOINT_QUEUE_SIZE);
        let dispatch = Self::dispatch_checkpoints(
            checkpoints,
            event_streamer.clone(),
            transaction_streamer.clone(),
        );
        spawn_monitored_task!(dispatch);
        Self {
            event_streamer,
            transaction_streamer,
            checkpoint_queue,
            metrics,
        }
    }

    async fn dispatch_checkpoints(
        mut checkpoints: mpsc::Receiver<CheckpointConversion>,
        event_streamer: Arc<EventStreamer>,
        transaction_streamer: Arc<TransactionStreamer>,
    ) {
        while let Some(convert) = checkpoints.recv().await {
            let updates = match tokio::task::spawn_blocking(convert).await {
                Ok(updates) => updates,
                Err(e) => {
                    error!("converting a checkpoint for the subscribers failed: {e}");
                    continue;
                }
            };
            for update in updates {
                transaction_streamer
                    .send(EffectsWithInput {
                        input: update.input,
                        effects: update.effects,
                    })
                    .await;
                for event in update.events {
                    event_streamer.send(event).await;
                }
            }
        }
    }
}

impl SubscriptionHandler {
    /// Whether any transaction subscription is active.
    pub fn has_transaction_subscribers(&self) -> bool {
        self.transaction_streamer.has_subscribers()
    }

    /// Whether any event subscription is active.
    pub fn has_event_subscribers(&self) -> bool {
        self.event_streamer.has_subscribers()
    }

    /// Queues `convert` and sends what it returns to the subscribers after
    /// everything queued before it. Never waits, and must be called within a
    /// Tokio runtime: if the subscribers fall
    /// `CHECKPOINT_QUEUE_SIZE` checkpoints behind, every subscription is
    /// ended, so that none of them misses a checkpoint unnoticed.
    pub fn notify_checkpoint(&self, convert: CheckpointConversion) {
        match self.checkpoint_queue.try_send(convert) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                warn!(
                    "the subscribers are {CHECKPOINT_QUEUE_SIZE} checkpoints behind, \
                     ending every subscription"
                );
                self.metrics
                    .dropped_submissions
                    .with_label_values(&["checkpoint"])
                    .inc();
                // Off the caller's thread: ending them waits for the
                // dispatch task, which holds the subscriber lock while it
                // matches an item against every subscription's filter.
                let transaction_streamer = self.transaction_streamer.clone();
                let event_streamer = self.event_streamer.clone();
                tokio::task::spawn_blocking(move || {
                    transaction_streamer.drop_subscribers();
                    event_streamer.drop_subscribers();
                });
            }
            Err(TrySendError::Closed(_)) => {
                error!("the subscription dispatch task has stopped");
            }
        }
    }

    pub fn subscribe_events(&self, filter: EventFilter) -> impl Stream<Item = IotaEvent> {
        self.event_streamer.subscribe(filter)
    }

    pub fn subscribe_transactions(
        &self,
        filter: TransactionFilter,
    ) -> impl Stream<Item = IotaTransactionBlockEffects> {
        self.transaction_streamer.subscribe(filter)
    }
}
