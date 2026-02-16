// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use iota_storage::http_key_value_store::{ItemType, Key};
use serde::Deserialize;

use crate::{errors::ApiError, extractors::ExtractPath, types::SharedRestServerAppState};

/// Request payload for multi_get_objects_post containing list of keys.
#[derive(Deserialize, Debug)]
pub struct MultiGetRequest {
    /// List of base64url-encoded keys to retrieve.
    pub keys: Vec<String>,
}

/// Retrieves data associated with a given key from the KV store as raw
/// [`Bytes`](bytes::Bytes).
///
/// # Returns
///
/// * If the key exists, the data is returned as a [`Bytes`](bytes::Bytes)
///   stream with a `200 OK` status code.
/// * If the key does not exist, a `204 No Content` status code is returned with
///   an empty body.
/// * If an error occurs while interacting with the KV store, an `500 internal
///   server error` is returned.
pub async fn data_as_bytes(
    ExtractPath(key): ExtractPath,
    State(app_state): State<SharedRestServerAppState>,
) -> Result<impl IntoResponse, ApiError> {
    app_state
        .kv_store_client
        .get(key)
        .await
        .map(|res| match res {
            Some(bytes) => bytes.into_response(),
            None => (StatusCode::NOT_FOUND, Body::empty()).into_response(),
        })
}

/// Retrieves multiple objects via POST request with JSON payload.
///
/// # Path Parameters
///
/// - `item_type`: The type of items to get (e.g., "cs", "cc", "tx")
///
/// # Request Body
///
/// JSON object with `keys` field:
///
/// ```json
/// {
///   "keys": ["AAEAAAAAAAAA", "AAIAAAAAAAAA", "AAMAAAAAAAAA"]
/// }
/// ```
///
/// Where:
/// - `keys`: Array of base64url-encoded keys for given `item_type`. The same
///   kind of key and encoding user would use in single item GET request.
///
/// # Returns
///
/// * If successful, returns a BCS-serialized
///   [`Vec`]<[`Option`]<[`Bytes`](bytes::Bytes)>> with a `200 OK` status code.
///   The vector has the same length and order as the `keys` list in the request
///   body. Each entry is `Some(bytes)` if the key was found, or `None` if the
///   key was not found.
/// * If no keys are provided or the number of keys exceeds the configured
///   `multiget_max_items` limit, a `400 bad request error` is returned.
/// * If the keys cannot be parsed, a `400 bad request error` is returned.
/// * If heterogenous key types are requested (e.g. checkpoints by sequence id
///   and by digest), a `400 bad request error` is returned.
/// * If an error occurs while interacting with the KV store, an `500 internal
///   server error` is returned.
pub async fn multi_get_data(
    Path(item_type): Path<ItemType>,
    State(app_state): State<SharedRestServerAppState>,
    Json(payload): Json<MultiGetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if payload.keys.is_empty() {
        return Err(ApiError::BadRequest("no keys provided".to_string()));
    }

    if payload.keys.len() > app_state.multiget_max_items.get() {
        return Err(ApiError::BadRequest(format!(
            "too many keys: requested {}, maximum allowed is {}",
            payload.keys.len(),
            app_state.multiget_max_items
        )));
    }

    let item_type_str = item_type.to_string();
    let keys = payload
        .keys
        .iter()
        .map(|encoded_key| {
            Key::new(item_type_str.as_str(), encoded_key.as_str())
                .map_err(|err| ApiError::BadRequest(format!("invalid key '{encoded_key}': {err}")))
        })
        .collect::<Result<Vec<Key>, ApiError>>()?;

    let results = app_state.kv_store_client.multi_get(keys).await?;

    let bcs_data = bcs::to_bytes(&results).map_err(|_| ApiError::InternalServerError)?;
    Ok(bcs_data.into_response())
}
