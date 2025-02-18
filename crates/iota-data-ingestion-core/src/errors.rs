// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub type IngestionResult<T, E = IngestionError> = core::result::Result<T, E>;

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

    #[error(transparent)]
    RestApi(#[from] iota_rest_api::client::sdk::Error),

    #[error("Register at least one worker pool")]
    EmptyWorkerPool,

    #[error("{component} shutdown error: `{msg}`")]
    Shutdown { component: String, msg: String },

    #[error("Channel error: `{0}`")]
    Channel(String),

    #[error("Checkpoint processing failed: `{0}`")]
    CheckpointProcessing(String),

    #[error("Checkpoint hook processing failed: `{0}`")]
    CheckpointHookProcessing(String),

    #[error("Progress Store error: `{0}`")]
    ProgressStore(String),

    #[error("Deserialize checkpoint failed: `{0}`")]
    DeserializeCheckpoint(String),
}
