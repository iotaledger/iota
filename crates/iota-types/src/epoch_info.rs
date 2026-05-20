// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    effects::TransactionEvents,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSequenceNumber},
};

/// Per-epoch metadata sufficient to rebuild the indexer's `epochs` table
/// without reading historical checkpoint contents. See iotaledger/iota#11254.
///
/// **Producer:** `grpc_indexes::index_epoch`, over two checkpoint boundaries —
/// `first_checkpoint` + `start_system_state` at the previous epoch's close;
/// `last_checkpoint_summary` + `end_of_epoch_tx_events` at this epoch's close.
/// The two trailing fields are `Option<>` because boundary 2 may not have run
/// yet; `Watermark::EpochIndexed` tracks fully-populated rows.
///
/// **Consumer:** the snapshot V2 writer (`EPOCH_INFO` file, wrapped in
/// `EpochInfo::V1`). Refuses to publish unless `EpochIndexed >=
/// snapshot_epoch`, so emitted entries always have all four fields set.
///
/// Wire-format: BCS-encoded in RocksDB and on the snapshot wire. The field
/// order is pinned by `epoch_info_entry_field_order_is_locked` in
/// [`unit_tests/epoch_info_tests.rs`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochInfoEntry {
    /// First checkpoint of this epoch (`0` for genesis; otherwise the prior
    /// epoch's `last_checkpoint_summary.sequence_number + 1`).
    pub first_checkpoint: CheckpointSequenceNumber,

    /// BCS-encoded `IotaSystemState` of object `0x5` right after the
    /// AdvanceEpoch tx of the previous epoch (or the genesis tx for epoch 0).
    /// Opaque bytes so the inner enum can evolve through its own variant axis
    /// without forcing a schema bump here; mirrors `Epoch.bcs_system_state`
    /// (proto tag 3). Decode:
    /// ```ignore
    /// let s: IotaSystemState = bcs::from_bytes(&entry.start_system_state)?;
    /// let summary = s.into_iota_system_state_summary();
    /// ```
    pub start_system_state: Vec<u8>,

    /// Certified summary of this epoch's last checkpoint — carries
    /// `end_of_epoch_data`, gas summary, timestamp, quorum signatures.
    /// `None` before boundary 2.
    pub last_checkpoint_summary: Option<CertifiedCheckpointSummary>,

    /// Events from the AdvanceEpoch tx — carries `SystemEpochInfoEvent`
    /// (storage/computation accounting, mint/burn, stake rewards). `None`
    /// before boundary 2.
    pub end_of_epoch_tx_events: Option<TransactionEvents>,
}

#[cfg(test)]
#[path = "unit_tests/epoch_info_tests.rs"]
mod epoch_info_tests;
