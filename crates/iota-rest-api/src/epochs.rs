// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::extract::{Path, State};
use iota_sdk2::types::{CheckpointSequenceNumber, EpochId, SignedCheckpointSummary};
use tap::Pipe;

use crate::{
    RestService, Result,
    accept::AcceptFormat,
    openapi::{ApiEndpoint, OperationBuilder, ResponseBuilder, RouteHandler},
    reader::StateReader,
    response::ResponseContent,
};

pub struct GetEpochLastCheckpoint;

impl ApiEndpoint<RestService> for GetEpochLastCheckpoint {
    fn method(&self) -> axum::http::Method {
        axum::http::Method::GET
    }

    fn path(&self) -> &'static str {
        "/epochs/{epoch}/last-checkpoint"
    }

    fn operation(
        &self,
        generator: &mut schemars::gen::SchemaGenerator,
    ) -> openapiv3::v3_1::Operation {
        OperationBuilder::new()
            .tag("Epochs")
            .operation_id("GetEpochLastCheckpoint")
            .path_parameter::<CheckpointSequenceNumber>("checkpoint", generator)
            .response(
                200,
                ResponseBuilder::new()
                    .json_content::<SignedCheckpointSummary>(generator)
                    .bcs_content()
                    .build(),
            )
            .response(404, ResponseBuilder::new().build())
            .build()
    }

    fn handler(&self) -> RouteHandler<RestService> {
        RouteHandler::new(self.method(), get_epoch_last_checkpoint)
    }
}

async fn get_epoch_last_checkpoint(
    Path(epoch): Path<EpochId>,
    accept: AcceptFormat,
    State(state): State<StateReader>,
) -> Result<ResponseContent<SignedCheckpointSummary>> {
    let summary = state
        .inner()
        .get_epoch_last_checkpoint(epoch)?
        .ok_or_else(|| EpochLastCheckpointNotFoundError::new(epoch))?
        .into_inner()
        .try_into()?;

    match accept {
        AcceptFormat::Json => ResponseContent::Json(summary),
        AcceptFormat::Bcs => ResponseContent::Bcs(summary),
    }
    .pipe(Ok)
}

#[derive(Debug)]
pub struct EpochLastCheckpointNotFoundError {
    epoch: EpochId,
}

impl EpochLastCheckpointNotFoundError {
    pub fn new(epoch: EpochId) -> Self {
        Self { epoch }
    }
}

impl std::fmt::Display for EpochLastCheckpointNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Epoch {} last checkpoint not found", self.epoch)
    }
}

impl std::error::Error for EpochLastCheckpointNotFoundError {}

impl From<EpochLastCheckpointNotFoundError> for crate::RestError {
    fn from(value: EpochLastCheckpointNotFoundError) -> Self {
        Self::new(axum::http::StatusCode::NOT_FOUND, value.to_string())
    }
}
