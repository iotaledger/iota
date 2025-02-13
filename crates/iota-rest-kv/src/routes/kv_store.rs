// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::{body::Body, extract::State, http::StatusCode, response::IntoResponse};

use crate::{errors::ApiError, extractors::ExtractPath, types::SharedKvStoreClient};

pub async fn data_as_bytes(
    ExtractPath(key): ExtractPath,
    State(kv_store_client): State<SharedKvStoreClient>,
) -> Result<impl IntoResponse, ApiError> {
    match kv_store_client.get(key).await {
        Ok(Some(bytes)) => Ok(bytes.into_response()),
        Ok(None) => Ok((StatusCode::NO_CONTENT, Body::empty()).into_response()),
        Err(err) => {
            tracing::error!("cannot fetch data from kv store: {err}");
            Err(ApiError::InternalServerError)
        }
    }
}
