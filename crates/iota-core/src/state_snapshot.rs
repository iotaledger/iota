// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The hand-over from the epoch boundary to the state snapshot writer.
//!
//! A formal snapshot must describe the live object set exactly as the epoch
//! ended with it, but scanning that set takes minutes and the node has an
//! epoch to get on with. The boundary therefore only takes a read view of the
//! perpetual store and hands it over; the scan and the upload happen while the
//! node executes the next epoch.

use std::sync::Arc;

use iota_types::committee::EpochId;
use tokio::sync::oneshot;

use crate::authority::authority_store_tables::AuthorityPerpetualTables;

/// Asks for the state snapshot of `epoch` to be written.
///
/// Sent while execution is paused at the epoch boundary, so that the read view
/// the writer takes still holds the state the epoch ended with.
pub struct EpochSnapshotRequest {
    /// The epoch whose live object set is to be captured.
    pub epoch: EpochId,
    /// The store the writer takes its read view of.
    pub perpetual_tables: Arc<AuthorityPerpetualTables>,
    /// Signalled once the read view exists. The boundary waits for this and
    /// for nothing after it: a view is cheap, the scan behind it is not.
    ///
    /// Dropped without a signal if the snapshot could not be started, which
    /// leaves the epoch without a snapshot but must not hold the node up.
    pub view_taken: oneshot::Sender<()>,
}
