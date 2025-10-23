// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::stream::BoxStream;
use iota_json_rpc_types::{
    EffectsWithInput as EffectsWithInputJson, EventFilter, IotaEvent, IotaTransactionBlockEffects,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockEvents,
    TransactionFilter as TransactionFilterJson,
};
use iota_types::{effects::TransactionEffects, error::IotaResult, transaction::TransactionData};
use prometheus::{
    IntCounterVec, IntGaugeVec, Registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry,
};
use tracing::{error, instrument, trace};

use crate::streamer::Streamer;

#[cfg(test)]
#[path = "unit_tests/subscription_handler_tests.rs"]
mod subscription_handler_tests;

pub const EVENT_DISPATCH_BUFFER_SIZE: usize = 1000;

/// Core representation of transaction effects with input, used for streaming
/// This avoids depending on gRPC or JSON-RPC specific types
#[derive(Clone, Debug)]
pub struct EffectsWithInput {
    pub input: TransactionData,
    pub effects: TransactionEffects,
}

/// A filter that accepts all items (no filtering)
#[derive(Clone, Debug)]
pub struct NoFilter;

impl iota_json_rpc_types::Filter<EffectsWithInput> for NoFilter {
    fn matches(&self, _item: &EffectsWithInput) -> bool {
        true
    }
}

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
    // For JSON-RPC subscriptions - filters at subscription level
    transaction_streamer_json:
        Streamer<EffectsWithInputJson, IotaTransactionBlockEffects, TransactionFilterJson>,
    // For gRPC subscriptions - broadcasts all transactions without filtering
    // This is efficient: a single stream serves multiple gRPC clients, each applying
    // their own filters at the service layer
    transaction_streamer_grpc: Streamer<EffectsWithInput, EffectsWithInput, NoFilter>,
}

impl SubscriptionHandler {
    pub fn new(registry: &Registry) -> Self {
        let metrics = Arc::new(SubscriptionMetrics::new(registry));
        Self {
            event_streamer: Streamer::spawn(EVENT_DISPATCH_BUFFER_SIZE, metrics.clone(), "event"),
            transaction_streamer_json: Streamer::spawn(
                EVENT_DISPATCH_BUFFER_SIZE,
                metrics.clone(),
                "tx_json",
            ),
            transaction_streamer_grpc: Streamer::spawn(
                EVENT_DISPATCH_BUFFER_SIZE,
                metrics,
                "tx_grpc",
            ),
        }
    }
}

impl SubscriptionHandler {
    #[instrument(level = "trace", skip_all, fields(tx_digest =? effects_json.transaction_digest()), err)]
    pub fn process_tx(
        &self,
        input: &TransactionData,
        effects_json: &IotaTransactionBlockEffects,
        effects_core: &TransactionEffects,
        events: &IotaTransactionBlockEvents,
    ) -> IotaResult {
        trace!(
            num_events = events.data.len(),
            tx_digest =? effects_json.transaction_digest(),
            "Processing tx/event subscription"
        );

        // Send to JSON-RPC streamer
        if let Err(e) = self
            .transaction_streamer_json
            .try_send(EffectsWithInputJson {
                input: input.clone(),
                effects: effects_json.clone(),
            })
        {
            error!(error =? e, "Failed to send transaction to JSON-RPC dispatch");
        }

        // Send to gRPC transaction streamer (broadcasts to all gRPC subscribers)
        // Each gRPC client will apply its own filter at the service layer
        if let Err(e) = self.transaction_streamer_grpc.try_send(EffectsWithInput {
            input: input.clone(),
            effects: effects_core.clone(),
        }) {
            error!(error =? e, "Failed to send transaction to gRPC dispatch");
        }

        // serially dispatch event processing to honor events' orders.
        for event in events.data.clone() {
            // Send to unified event streamer (serves both JSON-RPC and gRPC subscribers)
            if let Err(e) = self.event_streamer.try_send(event) {
                error!(error =? e, "Failed to send event to dispatch");
            }
        }
        Ok(())
    }

    pub fn subscribe_events(&self, filter: EventFilter) -> BoxStream<'static, IotaEvent> {
        Box::pin(self.event_streamer.subscribe(filter))
    }

    /// Subscribe to transactions for JSON-RPC
    pub fn subscribe_transactions(
        &self,
        filter: TransactionFilterJson,
    ) -> BoxStream<'static, IotaTransactionBlockEffects> {
        Box::pin(self.transaction_streamer_json.subscribe(filter))
    }

    /// Subscribe to transaction effects for gRPC
    ///
    /// Returns an unfiltered stream of all transactions for efficiency.
    /// This design allows a single broadcast stream to serve multiple gRPC
    /// clients, where each client applies its own filter at the gRPC
    /// service layer.
    pub fn subscribe_transactions_grpc(&self) -> BoxStream<'static, EffectsWithInput> {
        Box::pin(self.transaction_streamer_grpc.subscribe(NoFilter))
    }
}
