// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains Prometheus metrics for the streaming components.

use prometheus::{
    Histogram, IntGauge, IntGaugeVec, Registry, register_histogram_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry,
};

/// Represents a Prometheus metric label to identify stream responsible for
/// broadcasting events.
pub const METRICS_EVENT_LABEL: &str = "events";
/// Represents a Prometheus metric label to identify stream responsible for
/// broadcasting transactions.
pub const METRICS_TRANSACTION_LABEL: &str = "transactions";

/// Holds all available Prometheus metrics for the
/// [`InMemory`](crate::memory::InMemory) streamer implementation.
pub struct InMemoryStreamMetrics {
    /// Current number of active subscribers.
    pub active_subscriber_number: IntGaugeVec,
    /// Current number of lagging subscribers.
    pub lagging_subscribers: IntGaugeVec,
    /// Current notified checkpoint sequence number from the indexer.
    pub notified_checkpoint_sequence_number: IntGauge,
    /// Current notified transaction sequence number start range from the
    /// indexer.
    pub notified_tx_seq_num_start_range: IntGauge,
    /// Current notified transaction sequence number end range from the indexer.
    pub notified_tx_seq_num_end_range: IntGauge,
    /// Current broadcasted transaction sequence number start range to
    /// subscribers.
    pub broadcasted_to_subscribers_tx_seq_num_start_range: IntGauge,
    /// Current broadcasted transaction sequence number end range to
    /// subscribers.
    pub broadcasted_to_subscribers_tx_seq_num_end_range: IntGauge,
    /// Latency of querying a range of transactions from the indexer database.
    pub query_tx_from_indexer_db_latency: Histogram,
    /// Latency of broadcasting transactions and events to subscribers.
    pub broadcast_tx_and_ev_to_subscribers_latency: Histogram,
    /// The number of messages not yet received by all active subscribers.
    pub channel_pending_messages: IntGaugeVec,
    /// Latency of processing a batch of transactions, from query to broadcast
    /// to subscribers.
    pub process_transaction_batch_latency: Histogram,
    /// Latency of processing a batch of database notifications. It includes the
    /// tx range bounds resolution, the time taken to query the database, and
    /// broadcast them to subscribers.
    pub process_notification_batch_latency: Histogram,
}

impl InMemoryStreamMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            active_subscriber_number: register_int_gauge_vec_with_registry!(
                "active_subscriber_number",
                "Current number of active subscribers",
                &["type"],
                registry,
            )
            .unwrap(),
            lagging_subscribers: register_int_gauge_vec_with_registry!(
                "lagging_subscribers",
                "Current number of lagging subscribers",
                &["type"],
                registry,
            )
            .unwrap(),
            notified_checkpoint_sequence_number: register_int_gauge_with_registry!(
                "notified_checkpoint_sequence_number",
                "Current notified checkpoint sequence number from the indexer",
                registry,
            )
            .unwrap(),
            notified_tx_seq_num_start_range: register_int_gauge_with_registry!(
                "notified_tx_seq_num_start_range",
                "Current notified transaction sequence number start range from the indexer",
                registry,
            )
            .unwrap(),
            notified_tx_seq_num_end_range: register_int_gauge_with_registry!(
                "notified_tx_seq_num_end_range",
                "Current notified transaction sequence number end range from the indexer",
                registry,
            )
            .unwrap(),
            broadcasted_to_subscribers_tx_seq_num_start_range: register_int_gauge_with_registry!(
                "broadcasted_to_subscribers_tx_seq_num_start_range",
                "Current broadcasted transaction sequence number start range to subscribers",
                registry,
            )
            .unwrap(),
            broadcasted_to_subscribers_tx_seq_num_end_range: register_int_gauge_with_registry!(
                "broadcasted_to_subscribers_tx_seq_num_end_range",
                "Current broadcasted transaction sequence number end range to subscribers",
                registry,
            )
            .unwrap(),
            query_tx_from_indexer_db_latency: register_histogram_with_registry!(
                "query_tx_from_indexer_db_latency",
                "Latency of querying a range of transactions from the indexer database",
                registry,
            )
            .unwrap(),
            broadcast_tx_and_ev_to_subscribers_latency: register_histogram_with_registry!(
                "broadcast_tx_and_ev_to_subscribers_latency",
                "Latency of broadcasting transactions and events to subscribers",
                registry,
            )
            .unwrap(),
            channel_pending_messages: register_int_gauge_vec_with_registry!(
                "channel_pending_messages",
                "The number of messages not yet received by all active subscribers",
                &["type"],
                registry,
            )
            .unwrap(),
            process_transaction_batch_latency: register_histogram_with_registry!(
                "process_transaction_batch_latency",
                "Latency of processing a batch of transactions, from query to broadcast to subscribers",
                registry,
            )
            .unwrap(),
            process_notification_batch_latency: register_histogram_with_registry!(
                "process_notification_batch_latency",
                "Latency of processing a batch of database notifications. It includes the tx range bounds resolution, the time taken to query the database, and broadcast them to subscribers",
                registry,
            )
            .unwrap(),
        }
    }
}
