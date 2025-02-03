// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

pub type IngestionResult<T, E = IngestionError> = core::result::Result<T, E>;

#[derive(Debug, Error)]
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

    #[error(transparent)]
    Uncategorized(#[from] anyhow::Error),
}
