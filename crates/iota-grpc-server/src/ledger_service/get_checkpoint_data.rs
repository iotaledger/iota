// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil},
    v0::{
        bcs::BcsData,
        checkpoint::{Checkpoint, CheckpointSummary},
        ledger_service::{
            CheckpointData, GetCheckpointDataRequest, checkpoint_data,
            get_checkpoint_data_request::CheckpointId,
        },
    },
};
use iota_types::messages_checkpoint::CertifiedCheckpointSummary;
use prost_types::FieldMask;
use tonic::Status;

use crate::types::GrpcReader;

pub const READ_MASK_DEFAULT: &str = crate::field_mask!("summary.bcs");

/// Get checkpoint data based on the request.
///
/// Returns a single checkpoint payload wrapped in a CheckpointData message.
#[tracing::instrument(skip(reader))]
pub fn get_checkpoint_data(
    reader: &GrpcReader,
    request: GetCheckpointDataRequest,
) -> Result<CheckpointData, Status> {
    // Parse and validate the read mask
    let read_mask = request
        .checkpoint_read_mask
        .unwrap_or_else(|| FieldMask::from_str(READ_MASK_DEFAULT));
    read_mask
        .validate::<Checkpoint>()
        .map_err(|path| Status::invalid_argument(format!("invalid read_mask path: {path}")))?;
    let read_mask = FieldMaskTree::from(read_mask);

    // Get the checkpoint based on the checkpoint_id
    let checkpoint_id = request
        .checkpoint_id
        .ok_or_else(|| Status::invalid_argument("checkpoint_id is required"))?;

    let checkpoint_summary = match checkpoint_id {
        CheckpointId::Latest(true) => reader
            .get_latest_checkpoint()
            .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?,
        CheckpointId::Latest(false) => {
            return Err(Status::invalid_argument("latest must be true if specified"));
        }
        CheckpointId::SequenceNumber(seq) => reader
            .get_checkpoint_summary(seq)
            .map_err(|e| Status::internal(format!("Failed to get checkpoint: {e}")))?
            .ok_or_else(|| {
                Status::not_found(format!("Checkpoint with sequence number {seq} not found"))
            })?,
        CheckpointId::Digest(_) => {
            return Err(Status::unimplemented(
                "Checkpoint lookup by digest is not supported",
            ));
        }
    };

    // Build the Checkpoint proto message
    let checkpoint = build_checkpoint_proto(&checkpoint_summary, &read_mask)?;

    Ok(CheckpointData {
        payload: Some(checkpoint_data::Payload::Checkpoint(checkpoint)),
    })
}

/// Build a Checkpoint proto message from a CertifiedCheckpointSummary.
fn build_checkpoint_proto(
    summary: &CertifiedCheckpointSummary,
    read_mask: &FieldMaskTree,
) -> Result<Checkpoint, Status> {
    let mut checkpoint = Checkpoint::default();

    // Populate sequence_number if requested
    if read_mask.contains(Checkpoint::SEQUENCE_NUMBER_FIELD.name) {
        checkpoint.sequence_number = Some(*summary.sequence_number());
    }

    // Populate summary if requested (handles nested fields like summary.bcs)
    if let Some(submask) = read_mask.subtree(Checkpoint::SUMMARY_FIELD.name) {
        let mut proto_summary = CheckpointSummary::default();

        // Serialize the full CertifiedCheckpointSummary as BCS
        // This is what the client expects to deserialize as SignedCheckpointSummary
        if submask.contains(CheckpointSummary::BCS_FIELD.name) {
            proto_summary.bcs = BcsData::serialize(summary).ok();
        }

        // Add digest if requested
        if submask.contains(CheckpointSummary::DIGEST_FIELD.name) {
            let digest: iota_sdk_types::Digest = (*summary.digest()).into();
            proto_summary.digest = Some(digest.into());
        }

        checkpoint.summary = Some(proto_summary);
    }

    // Populate signature if requested
    if read_mask.contains(Checkpoint::SIGNATURE_FIELD.name) {
        let sig: iota_sdk_types::ValidatorAggregatedSignature = summary.auth_sig().clone().into();
        checkpoint.signature = Some(sig.into());
    }

    Ok(checkpoint)
}
