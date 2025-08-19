// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_json_rpc_types::{
    EffectsWithInput, EventFilter, IotaEvent, IotaTransactionBlockEffects,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockEvents, TransactionFilter,
};
use iota_types::{error::IotaResult, transaction::TransactionData};
use prometheus::{
    IntCounterVec, IntGaugeVec, Registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry,
};
use tokio::sync::broadcast::Sender;
use tokio_stream::Stream;
use tracing::{debug, error, instrument, trace};

use crate::streamer::Streamer;

#[cfg(test)]
#[path = "unit_tests/subscription_handler_tests.rs"]
mod subscription_handler_tests;

pub const EVENT_DISPATCH_BUFFER_SIZE: usize = 1000;

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

pub struct SubscriptionHandler {
    event_streamer: Streamer<IotaEvent, IotaEvent, EventFilter>,
    transaction_streamer:
        Streamer<EffectsWithInput, IotaTransactionBlockEffects, TransactionFilter>,
    grpc_event_broadcast_tx: Option<Sender<Arc<IotaEvent>>>,
}

impl SubscriptionHandler {
    pub fn new(
        registry: &Registry,
        grpc_event_broadcast_tx: Option<Sender<Arc<IotaEvent>>>,
    ) -> Self {
        let metrics = Arc::new(SubscriptionMetrics::new(registry));
        Self {
            event_streamer: Streamer::spawn(EVENT_DISPATCH_BUFFER_SIZE, metrics.clone(), "event"),
            transaction_streamer: Streamer::spawn(EVENT_DISPATCH_BUFFER_SIZE, metrics, "tx"),
            grpc_event_broadcast_tx,
        }
    }
}

impl SubscriptionHandler {
    #[instrument(level = "trace", skip_all, fields(tx_digest =? effects.transaction_digest()), err)]
    pub fn process_tx(
        &self,
        input: &TransactionData,
        effects: &IotaTransactionBlockEffects,
        events: &IotaTransactionBlockEvents,
    ) -> IotaResult {
        trace!(
            num_events = events.data.len(),
            tx_digest =? effects.transaction_digest(),
            "Processing tx/event subscription"
        );

        if let Err(e) = self.transaction_streamer.try_send(EffectsWithInput {
            input: input.clone(),
            effects: effects.clone(),
        }) {
            error!(error =? e, "Failed to send transaction to dispatch");
        }

        // serially dispatch event processing to honor events' orders.
        for (index, event) in events.data.clone().iter().enumerate() {
            // Send to existing event streamer
            if let Err(e) = self.event_streamer.try_send(event.clone()) {
                error!(error =? e, "Failed to send event to dispatch");
            }

            // Also send to gRPC broadcast channel if available
            if let Some(ref grpc_tx) = self.grpc_event_broadcast_tx {
                if grpc_tx.receiver_count() > 0 {
                    let event_arc = Arc::new(event.clone());
                    match grpc_tx.send(event_arc) {
                        Ok(subscriber_count) => {
                            debug!(
                                event_index = index,
                                subscriber_count = subscriber_count,
                                tx_digest =? effects.transaction_digest(),
                                event_type = %event.type_.name,
                                "Broadcasted event to {subscriber_count} gRPC subscriber(s)"
                            );
                        }
                        Err(e) => {
                            error!(
                                error = ?e,
                                event_index = index,
                                tx_digest =? effects.transaction_digest(),
                                event_type = %event.type_.name,
                                "Failed to broadcast event to gRPC channel"
                            );
                        }
                    }
                } else {
                    debug!(
                        event_index = index,
                        tx_digest =? effects.transaction_digest(),
                        event_type = %event.type_.name,
                        "No gRPC subscribers, skipping broadcast"
                    );
                }
            }
        }
        Ok(())
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
