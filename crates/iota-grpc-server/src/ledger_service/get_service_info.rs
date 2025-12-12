// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil},
    google::rpc::bad_request::FieldViolation,
    proto::timestamp_ms_to_proto,
    v0::{
        error_reason::ErrorReason,
        ledger_service::{GetServiceInfoRequest, GetServiceInfoResponse},
    },
};
use iota_types::digests::Digest;
use prost_types::FieldMask;
use tap::Pipe;

use crate::{error::RpcError, ledger_service::LedgerGrpcService};

pub const READ_MASK_DEFAULT: &str =
    crate::field_mask!("chain_id", "epoch", "executed_checkpoint_height");

#[tracing::instrument(skip(service))]
pub fn get_service_info(
    service: &LedgerGrpcService,
    request: GetServiceInfoRequest,
) -> Result<GetServiceInfoResponse, RpcError> {
    let read_mask = {
        let read_mask = request
            .read_mask
            .unwrap_or_else(|| FieldMask::from_str(READ_MASK_DEFAULT));
        read_mask
            .validate::<GetServiceInfoResponse>()
            .map_err(|path| {
                FieldViolation::new("read_mask")
                    .with_description(format!("invalid read_mask path: {path}"))
                    .with_reason(ErrorReason::FieldInvalid)
            })?;
        FieldMaskTree::from(read_mask)
    };

    let latest_checkpoint = service.reader.get_latest_checkpoint()?;

    let mut message = GetServiceInfoResponse::default();

    if read_mask.contains(GetServiceInfoResponse::CHAIN_ID_FIELD.name) {
        message.chain_id = Some(Digest::new(service.chain_id.as_bytes().to_owned()).to_string());
    }

    if read_mask.contains(GetServiceInfoResponse::CHAIN_FIELD.name) {
        message.chain = Some(service.chain_id.chain().as_str().into());
    }

    if read_mask.contains(GetServiceInfoResponse::EPOCH_FIELD.name) {
        message.epoch = Some(latest_checkpoint.epoch());
    }

    if read_mask.contains(GetServiceInfoResponse::EXECUTED_CHECKPOINT_HEIGHT_FIELD.name) {
        message.executed_checkpoint_height = Some(latest_checkpoint.sequence_number);
    }

    if read_mask.contains(GetServiceInfoResponse::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD.name) {
        message.executed_checkpoint_timestamp =
            Some(timestamp_ms_to_proto(latest_checkpoint.timestamp_ms));
    }

    if read_mask.contains(GetServiceInfoResponse::LOWEST_AVAILABLE_CHECKPOINT_FIELD.name) {
        message.lowest_available_checkpoint =
            service.reader.get_lowest_available_checkpoint()?.pipe(Some);
    }

    if read_mask.contains(GetServiceInfoResponse::LOWEST_AVAILABLE_CHECKPOINT_OBJECTS_FIELD.name) {
        message.lowest_available_checkpoint_objects = service
            .reader
            .get_lowest_available_checkpoint_objects()?
            .pipe(Some);
    }

    if read_mask.contains(GetServiceInfoResponse::SERVER_FIELD.name) {
        message.server = service.server_version.as_ref().map(ToString::to_string);
    }

    Ok(message)
}
