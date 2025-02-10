// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::extract::State;
use bytes::Bytes;

use crate::{errors::ApiError, extractors::ExtractPath, types::SharedKvStoreService};

pub async fn data_as_bytes(
    ExtractPath(key): ExtractPath,
    State(kv_store_service): State<SharedKvStoreService>,
) -> Result<Bytes, ApiError> {
    match kv_store_service.get(key).await {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => Err(ApiError::NotFound),
        Err(err) => {
            tracing::error!("cannot fetch data from kv store: {err}");
            Err(ApiError::InternalServerError)
        }
    }
}
