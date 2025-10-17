// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::stream::BoxStream;
use iota_grpc_types::transactions::{
    EffectsWithInput as EffectsWithInputGrpc, TransactionFilter as TransactionFilterGrpc,
};
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
    // For JSON-RPC subscriptions
    transaction_streamer_json:
        Streamer<EffectsWithInputJson, IotaTransactionBlockEffects, TransactionFilterJson>,
    // For gRPC subscriptions
    transaction_streamer_grpc:
        Streamer<EffectsWithInputGrpc, TransactionEffects, TransactionFilterGrpc>,
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

        // Send to gRPC streamer - use core type directly (no conversion needed)
        if let Err(e) = self
            .transaction_streamer_grpc
            .try_send(EffectsWithInputGrpc {
                input: input.clone(),
                effects: effects_core.clone(),
            })
        {
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

    /// Subscribe to transactions for gRPC
    pub fn subscribe_transactions_grpc(
        &self,
        filter: TransactionFilterGrpc,
    ) -> BoxStream<'static, TransactionEffects> {
        Box::pin(self.transaction_streamer_grpc.subscribe(filter))
    }
}
