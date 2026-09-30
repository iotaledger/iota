// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Reporting of the oldest checkpoint a response can contain data from.
//!
//! A handler that reads pruned tables reports how far its response reaches
//! back with [`report_oldest_available_checkpoint`], and that checkpoint is
//! returned in the [`OLDEST_AVAILABLE_CHECKPOINT_HEADER`] header. A response
//! whose handler reported nothing returns no header.

use std::sync::{Arc, OnceLock};

use jsonrpsee::Extensions;

/// The response header carrying the oldest checkpoint the response can contain
/// data from.
pub const OLDEST_AVAILABLE_CHECKPOINT_HEADER: &str = "x-iota-oldest-available-checkpoint";

/// The checkpoint a single request reports, written by the handler that serves
/// it and read once the response is built.
///
/// For each user request a new instance of this struct is made and passed to
/// the request's [`Extensions`].
#[derive(Clone, Debug, Default)]
pub struct OldestAvailableCheckpoint(Arc<OnceLock<u64>>);

impl OldestAvailableCheckpoint {
    /// Returns the reported checkpoint, or `None` when the handler did not
    /// report one.
    pub fn get(&self) -> Option<u64> {
        self.0.get().copied()
    }

    fn set(&self, checkpoint: u64) {
        // It is expected that each handler will report once.
        // Subsequent calls to this function are ignored.
        let _ = self.0.set(checkpoint);
    }
}

/// Records the oldest checkpoint the response being served can contain data
/// from.
///
/// This function is called by request handlers, and is expected to be called
/// at most once per request. Can be called for both successful and failed
/// requests.
///
/// The reported value is returned to the client in an HTTP header.
///
/// Does nothing on a server that does not report the available range.
pub fn report_oldest_available_checkpoint(extensions: &Extensions, checkpoint: u64) {
    if let Some(reported_cp) = extensions.get::<OldestAvailableCheckpoint>() {
        reported_cp.set(checkpoint);
    }
}
