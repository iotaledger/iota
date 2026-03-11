// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_types::{
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaResult,
    event::EventEnvelope,
    filter::{EffectsWithInput, EventFilter, TransactionFilter},
    transaction::TransactionData,
};
use prometheus::{
    IntCounterVec, IntGaugeVec, Registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry,
};
use tokio_stream::Stream;
use tracing::{error, instrument, trace};

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
    event_streamer: Streamer<EventEnvelope, EventEnvelope, EventFilter>,
    transaction_streamer: Streamer<EffectsWithInput, TransactionEffects, TransactionFilter>,
}

impl SubscriptionHandler {
    pub fn new(registry: &Registry) -> Self {
        let metrics = Arc::new(SubscriptionMetrics::new(registry));
        Self {
            event_streamer: Streamer::spawn(EVENT_DISPATCH_BUFFER_SIZE, metrics.clone(), "event"),
            transaction_streamer: Streamer::spawn(EVENT_DISPATCH_BUFFER_SIZE, metrics, "tx"),
        }
    }
}

impl SubscriptionHandler {
    /// Process a committed transaction for subscription dispatch.
    ///
    /// `event_envelopes` must have `parsed_json` populated by the caller using
    /// a layout resolver so that `MoveEventField` filters work correctly.
    #[instrument(level = "trace", skip_all, fields(tx_digest =? effects.transaction_digest()), err)]
    pub fn process_tx(
        &self,
        input: &TransactionData,
        effects: &TransactionEffects,
        event_envelopes: Vec<EventEnvelope>,
    ) -> IotaResult {
        let tx_digest = *effects.transaction_digest();
        trace!(
            num_events = event_envelopes.len(),
            ?tx_digest,
            "Processing tx/event subscription"
        );

        if let Err(e) = self.transaction_streamer.try_send(EffectsWithInput {
            input: input.clone(),
            effects: effects.clone(),
        }) {
            error!(error =? e, "Failed to send transaction to dispatch");
        }

        // Serially dispatch event processing to honor events' ordering.
        for envelope in event_envelopes {
            if let Err(e) = self.event_streamer.try_send(envelope) {
                error!(error =? e, "Failed to send event to dispatch");
            }
        }
        Ok(())
    }

    pub fn subscribe_events(&self, filter: EventFilter) -> impl Stream<Item = EventEnvelope> {
        self.event_streamer.subscribe(filter)
    }

    pub fn subscribe_transactions(
        &self,
        filter: TransactionFilter,
    ) -> impl Stream<Item = TransactionEffects> {
        self.transaction_streamer.subscribe(filter)
    }
}
