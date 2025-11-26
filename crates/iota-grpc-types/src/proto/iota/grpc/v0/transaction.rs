// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction.rs");
include!("../../../generated/iota.grpc.v0.transaction.field_info.rs");
include!("../../../generated/iota.grpc.v0.transaction.accessors.rs");

use crate::{
    field::FieldMaskTree,
    merge::Merge,
    proto::timestamp_ms_to_proto,
    v0::{bcs::BcsData, types::Digest},
};

/// Bundles all transaction-related data needed for gRPC response merging.
///
/// This struct serves as a read context that groups related transaction data
/// together, avoiding the need to pass 6 separate parameters to merge
/// operations. Different merge targets use different subsets of fields:
/// - `ExecutedTransaction`: uses all fields
/// - `Transaction`: uses digest + transaction
/// - `UserSignatures`: uses transaction only
///
/// Note: The digest must be provided separately because
/// `iota_sdk2::SignedTransaction` doesn't expose a digest() method - it's
/// computed from `iota_types::TransactionData`.
pub struct TransactionReadSource<'a> {
    pub digest: iota_sdk_types::Digest,
    pub transaction: &'a iota_sdk_types::SignedTransaction,
    pub effects: &'a iota_sdk_types::TransactionEffects,
    pub events: Option<&'a iota_sdk_types::TransactionEvents>,
    pub checkpoint: Option<iota_sdk_types::CheckpointSequenceNumber>,
    pub timestamp_ms: Option<u64>,
}

impl Merge<&TransactionReadSource<'_>> for ExecutedTransaction {
    fn merge(&mut self, source: &TransactionReadSource, mask: &FieldMaskTree) {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(Digest {
                digest: source.digest.into_inner().to_vec().into(),
            });
        }

        // Set transaction if requested
        if let Some(tx_mask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            let mut proto_tx = Transaction::default();
            proto_tx.merge(source, &tx_mask);
            self.transaction = Some(proto_tx);
        }

        // Set signatures if requested
        if let Some(signatures_mask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            let mut proto_signatures = super::signatures::UserSignatures::default();
            proto_signatures.merge(source.transaction, &signatures_mask);
            self.signatures = Some(proto_signatures);
        }

        // Set effects if requested
        if let Some(effects_mask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            let mut proto_effects = TransactionEffects::default();
            proto_effects.merge(source.effects, &effects_mask);
            self.effects = Some(proto_effects);
        }

        // Set checkpoint if requested
        if mask.contains(Self::CHECKPOINT_FIELD.name) {
            self.checkpoint = source.checkpoint;
        }

        // Set timestamp if requested
        if mask.contains(Self::TIMESTAMP_FIELD.name) {
            self.timestamp = source.timestamp_ms.map(timestamp_ms_to_proto);
        }

        // Note: Events, input_objects, and output_objects are handled
        // separately by the caller as they require additional context
        // and data not present in TransactionReadSource
    }
}

// TODO: Wrap Transaction into a type with a version
impl Merge<&TransactionReadSource<'_>> for Transaction {
    fn merge(&mut self, source: &TransactionReadSource, mask: &FieldMaskTree) {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(Digest {
                digest: source.digest.into_inner().to_vec().into(),
            });
        }

        // Set BCS if requested
        if mask.contains(Self::BCS_FIELD.name) {
            if let Ok(bcs_bytes) = bcs::to_bytes(&source.transaction.transaction) {
                self.bcs = Some(BcsData {
                    data: bcs_bytes.into(),
                });
            }
        }
    }
}

// TODO: Wrap TransactionEffects into a type with a version
impl Merge<&iota_sdk_types::TransactionEffects> for TransactionEffects {
    fn merge(&mut self, source: &iota_sdk_types::TransactionEffects, mask: &FieldMaskTree) {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            let transaction_digest = match source {
                iota_sdk_types::TransactionEffects::V1(effects) => &effects.transaction_digest,
            };
            self.digest = Some(Digest {
                digest: transaction_digest.into_inner().to_vec().into(),
            });
        }

        // Set BCS if requested
        if mask.contains(Self::BCS_FIELD.name) {
            if let Ok(bcs_bytes) = bcs::to_bytes(source) {
                self.bcs = Some(BcsData {
                    data: bcs_bytes.into(),
                });
            }
        }
    }
}

// TODO: Wrap TransactionEvents into a type with a version
impl Merge<&iota_sdk_types::TransactionEvents> for TransactionEvents {
    fn merge(&mut self, source: &iota_sdk_types::TransactionEvents, mask: &FieldMaskTree) {
        // Note: digest is set from TransactionEffects.events_digest by the caller
        // The digest should be obtained from the parent TransactionEffects, not
        // computed here

        // Set events if requested
        if let Some(events_mask) = mask.subtree(Self::EVENTS_FIELD.name) {
            let mut proto_events = super::event::Events::default();
            proto_events.merge(source, &events_mask);
            self.events = Some(proto_events);
        }
    }
}
