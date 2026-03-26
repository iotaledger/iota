// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub type IngestionResult<T, E = IngestionError> = core::result::Result<T, E>;

// TODO: make first letter lower-case to all messages
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IngestionError {
    #[error(transparent)]
    ObjectStore(#[from] object_store::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Bcs(#[from] bcs::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("grpc error: `{0}`")]
    Grpc(String),

    #[error("Register at least one worker pool")]
    EmptyWorkerPool,

    #[error("{component} shutdown error: `{msg}`")]
    Shutdown { component: String, msg: String },

    #[error("channel error: `{0}`")]
    Channel(String),

    #[error("checkpoint processing failed: `{0}`")]
    CheckpointProcessing(String),

    #[error("checkpoint hook processing failed: `{0}`")]
    CheckpointHookProcessing(String),

    #[error("progress store error: `{0}`")]
    ProgressStore(String),

    #[error("reducer error: `{0}`")]
    Reducer(String),

    #[error("deserialize checkpoint failed: `{0}`")]
    DeserializeCheckpoint(String),

    #[error(transparent)]
    Upstream(#[from] anyhow::Error),

    #[error("reading historical data failed: `{0}`")]
    HistoryRead(String),

    #[error("max downloaded checkpoints limit reached")]
    MaxCheckpointsCapacityReached,

    #[error("checkpoint not available yet")]
    CheckpointNotAvailableYet,

    #[error(transparent)]
    Sdk(#[from] iota_types::iota_sdk_types_conversions::SdkTypeConversionError),
}

impl From<iota_grpc_types::proto::TryFromProtoError> for IngestionError {
    fn from(err: iota_grpc_types::proto::TryFromProtoError) -> Self {
        Self::Grpc(err.to_string())
    }
}

impl From<iota_grpc_client::Error> for IngestionError {
    fn from(err: iota_grpc_client::Error) -> Self {
        Self::Grpc(err.to_string())
    }
}
