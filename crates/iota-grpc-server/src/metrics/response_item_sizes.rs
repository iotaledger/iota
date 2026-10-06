// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The sizes of the items built for a response.

use crate::{metrics::RequestMetrics, utils::checkpoint_data_wrapper_overhead};

/// What an item of a response is, for the item size metrics.
#[derive(Clone, Copy, strum::IntoStaticStr, strum::EnumIter)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum ResponseItemKind {
    Object,
    Transaction,
    Event,
    Checkpoint,
    DynamicField,
    OwnedObject,
}

/// Adds up the sizes of the messages of one checkpoint while the stream builds
/// them. Does nothing when the request has no metrics.
pub(crate) struct CheckpointSizeTracker {
    metrics: RequestMetrics,
    bytes: usize,
}

impl CheckpointSizeTracker {
    pub(crate) fn new(metrics: &RequestMetrics) -> Self {
        Self {
            metrics: metrics.clone(),
            bytes: 0,
        }
    }

    /// Adds a message that holds a batch of items. `batch_size` is the size of
    /// the items in the batch, with the overhead of each item, as the stream
    /// counts them. The overhead of the message itself is added here.
    pub(crate) fn add_batch_size(&mut self, batch_size: usize) {
        if self.metrics.is_enabled() {
            self.bytes += batch_size + checkpoint_data_wrapper_overhead(batch_size);
        }
    }

    /// Adds the encoded size of `message`.
    pub(crate) fn add_message_size(&mut self, message: &impl prost::Message) {
        if self.metrics.is_enabled() {
            self.bytes += message.encoded_len();
        }
    }

    /// Records the sum of the added sizes.
    pub(crate) fn record(self) {
        self.metrics
            .record_response_item(ResponseItemKind::Checkpoint, self.bytes);
    }
}
