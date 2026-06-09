// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    proto::timestamp_ms_to_proto,
    read_masks::GET_SERVICE_INFO_READ_MASK,
    v1::ledger_service::{GetServiceInfoRequest, GetServiceInfoResponse},
};

use crate::{error::RpcError, ledger_service::LedgerGrpcService, validation::validate_read_mask};

/// Available Read Mask Fields
///
/// The `get_service_info` function supports the following `read_mask` fields to
/// control which data is included in the response:
///
/// ## Network Fields
/// - `chain_id` - the ID of the chain, which can be used to identify the
///   network
/// - `chain` - the chain identifier, which can be used to identify the network
///
/// ## Current State Fields
/// - `epoch` - the current epoch
/// - `executed_checkpoint_height` - the height of the last executed checkpoint
/// - `executed_checkpoint_timestamp` - the timestamp of the last executed
///   checkpoint
///
/// ## Availability Fields
/// - `lowest_available_checkpoint` - lowest available checkpoint for which
///   transaction and checkpoint data can be requested
/// - `lowest_available_checkpoint_objects` - lowest available checkpoint for
///   which object data can be requested
///
/// ## Server Fields
/// - `server` - the server version
#[tracing::instrument(skip(service))]
pub fn get_service_info(
    service: &LedgerGrpcService,
    request: GetServiceInfoRequest,
) -> Result<GetServiceInfoResponse, RpcError> {
    let read_mask = validate_read_mask::<GetServiceInfoResponse>(
        request.read_mask,
        GET_SERVICE_INFO_READ_MASK,
    )?;

    let latest_checkpoint = service.reader.get_latest_checkpoint()?;

    let mut message = GetServiceInfoResponse::default();

    if read_mask.contains(GetServiceInfoResponse::CHAIN_ID_FIELD.name) {
        message = message.with_chain_id(iota_sdk_types::Digest::new(
            service.chain_id.digest().into_inner(),
        ));
    }

    if read_mask.contains(GetServiceInfoResponse::CHAIN_FIELD.name) {
        message = message.with_chain(service.chain_id.chain().as_str());
    }

    if read_mask.contains(GetServiceInfoResponse::EPOCH_FIELD.name) {
        message = message.with_epoch(latest_checkpoint.epoch());
    }

    if read_mask.contains(GetServiceInfoResponse::EXECUTED_CHECKPOINT_HEIGHT_FIELD.name) {
        message = message.with_executed_checkpoint_height(latest_checkpoint.sequence_number);
    }

    if read_mask.contains(GetServiceInfoResponse::EXECUTED_CHECKPOINT_TIMESTAMP_FIELD.name) {
        message = message.with_executed_checkpoint_timestamp(timestamp_ms_to_proto(
            latest_checkpoint.timestamp_ms,
        ));
    }

    if read_mask.contains(GetServiceInfoResponse::LOWEST_AVAILABLE_CHECKPOINT_FIELD.name) {
        message = message
            .with_lowest_available_checkpoint(service.reader.get_lowest_available_checkpoint()?);
    }

    if read_mask.contains(GetServiceInfoResponse::LOWEST_AVAILABLE_CHECKPOINT_OBJECTS_FIELD.name) {
        message = message.with_lowest_available_checkpoint_objects(
            service.reader.get_lowest_available_checkpoint_objects()?,
        );
    }

    if read_mask.contains(GetServiceInfoResponse::SERVER_FIELD.name) {
        if let Some(server) = service.reader.server_version() {
            message = message.with_server(server);
        }
    }

    Ok(message)
}
