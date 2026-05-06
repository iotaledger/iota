// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    effects::TransactionEvents,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSequenceNumber},
};

/// Per-epoch metadata sufficient to rebuild a per-epoch summary table
/// without reading historical checkpoint contents.
///
/// Stored as the value type of the `epoch_info` table on `CheckpointStore`,
/// alongside `epoch_last_checkpoint_map`. The table is populated incrementally
/// at each AdvanceEpoch transaction by the checkpoint executor and read by
/// the snapshot V2 writer to produce the snapshot's `EPOCH_INFO` file.
///
/// Wire-format stability: this struct is BCS-encoded both in RocksDB (as the
/// value of `epoch_info`) and on the snapshot wire (embedded inside
/// `EpochInfo::V1`). Adding, removing, or reordering fields would corrupt
/// every existing on-disk row AND change the on-wire layout under
/// `EpochInfo::V1`. Any schema change therefore requires bumping
/// `EpochInfo::V2` AND providing a separate `EpochInfoEntryV2`, mirroring
/// the `StoreObjectV1`/`StoreObjectV2` migration pattern. The
/// `epoch_info_entry_field_order_is_locked` test below locks the BCS
/// field order against silent reordering.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochInfoEntry {
    /// The certified summary of the last checkpoint of this epoch. Carries
    /// `end_of_epoch_data` (committee transition, next protocol version, epoch
    /// commitments, supply change), the gas summary, the timestamp, and quorum
    /// signatures so consumers can verify the entry against the prior epoch's
    /// committee.
    pub last_checkpoint_summary: CertifiedCheckpointSummary,

    /// First checkpoint sequence number of this epoch. For the genesis epoch
    /// this is `0`; for later epochs it equals the previous entry's
    /// `last_checkpoint_summary.sequence_number + 1`.
    pub first_checkpoint: CheckpointSequenceNumber,

    /// Raw events emitted by the AdvanceEpoch transaction (the last
    /// transaction of the epoch). Carries the `SystemEpochInfoEvent` from
    /// which storage charges/rebates, fees, mint/burn amounts, and stake
    /// rewards can be extracted. Stored as raw events so consumers can
    /// pick the fields they need rather than committing to a fixed
    /// projection here.
    pub end_of_epoch_tx_events: TransactionEvents,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::AuthorityStrongQuorumSignInfo, gas::GasCostSummary, message_envelope::Envelope,
        messages_checkpoint::CheckpointSummary,
    };

    fn empty_checkpoint_summary() -> CheckpointSummary {
        CheckpointSummary {
            epoch: 0,
            sequence_number: 0,
            network_total_transactions: 0,
            content_digest: Default::default(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: GasCostSummary::default(),
            end_of_epoch_data: None,
            timestamp_ms: 0,
            version_specific_data: Vec::new(),
            checkpoint_commitments: Vec::new(),
        }
    }

    fn empty_certified_summary() -> CertifiedCheckpointSummary {
        let sig = AuthorityStrongQuorumSignInfo {
            epoch: 0,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        Envelope::new_from_data_and_sig(empty_checkpoint_summary(), sig)
    }

    /// Locks the BCS field order of `EpochInfoEntry` against silent
    /// reordering. BCS encodes struct fields in declaration order, so
    /// swapping any two fields would silently corrupt every on-disk row
    /// in the `epoch_info` column family AND change the on-wire layout
    /// under `EpochInfo::V1` in the snapshot. If a deliberate schema
    /// change is required, follow the versioning recipe in the doc comment
    /// on `EpochInfoEntry` (introduce `EpochInfoEntryV2` rather than
    /// mutating this type).
    ///
    /// Asserts that `bcs(entry)` equals the concatenation
    /// `bcs(last_checkpoint_summary) ++ first_checkpoint.to_le_bytes()
    /// ++ bcs(end_of_epoch_tx_events)`. This both verifies the relative
    /// order of the three fields and detects any encoding-shape change
    /// in the inner types.
    #[test]
    fn epoch_info_entry_field_order_is_locked() {
        let entry = EpochInfoEntry {
            last_checkpoint_summary: empty_certified_summary(),
            // Distinct, recognizable u64 — easy to spot in a hex dump if
            // this assertion ever needs to be debugged.
            first_checkpoint: 0xDEAD_BEEF_CAFE_F00D,
            end_of_epoch_tx_events: TransactionEvents::default(),
        };

        let entry_bytes = bcs::to_bytes(&entry).expect("entry serialization");
        let summary_bytes =
            bcs::to_bytes(&entry.last_checkpoint_summary).expect("summary serialization");
        let events_bytes =
            bcs::to_bytes(&entry.end_of_epoch_tx_events).expect("events serialization");

        let mut expected = Vec::with_capacity(entry_bytes.len());
        expected.extend_from_slice(&summary_bytes);
        expected.extend_from_slice(&entry.first_checkpoint.to_le_bytes());
        expected.extend_from_slice(&events_bytes);

        assert_eq!(
            entry_bytes, expected,
            "EpochInfoEntry BCS layout changed; introduce EpochInfoEntryV2 \
             rather than mutating this type"
        );
    }
}
