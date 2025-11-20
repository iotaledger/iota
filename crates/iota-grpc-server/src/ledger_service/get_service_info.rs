// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{proto::timestamp_ms_to_proto, v0::ledger_service::GetServiceInfoResponse};
use iota_types::digests::Digest;
use tap::Pipe;

use crate::{error::RpcError, ledger_service::LedgerGrpcService};

#[tracing::instrument(skip(service))]
pub fn get_service_info(service: &LedgerGrpcService) -> Result<GetServiceInfoResponse, RpcError> {
    let latest_checkpoint = service.reader.get_latest_checkpoint()?;
    let lowest_available_checkpoint = service.reader.get_lowest_available_checkpoint()?.pipe(Some);
    let lowest_available_checkpoint_objects = service
        .reader
        .get_lowest_available_checkpoint_objects()?
        .pipe(Some);

    Ok(GetServiceInfoResponse {
        chain_id: Some(Digest::new(service.chain_id.as_bytes().to_owned()).to_string()),
        chain: Some(service.chain_id.chain().as_str().into()),
        epoch: Some(latest_checkpoint.epoch()),
        executed_checkpoint_height: Some(latest_checkpoint.sequence_number),
        executed_checkpoint_timestamp: Some(timestamp_ms_to_proto(latest_checkpoint.timestamp_ms)),
        lowest_available_checkpoint,
        lowest_available_checkpoint_objects,
        server: service.server_version.as_ref().map(ToString::to_string),
    })
}
