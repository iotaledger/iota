// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::num::NonZeroUsize;

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use iota_kvstore::client::TransactionSequenceNumber;
use iota_storage::http_key_value_store::{ItemType, Key};
use iota_types::base_types::IotaAddress;
use serde::Deserialize;

use crate::{
    bigtable::{ObjectRangeKeyBound, ObjectsBeforeVersionRequest},
    errors::ApiError,
    extractors::ExtractPath,
    types::SharedRestServerAppState,
};

const BEFORE_VERSION_REQUIRES_OB_ERROR_MSG: &str =
    "`before_version` query parameter is only valid for `ob` item types";

/// Request payload for multi_get_objects_post containing list of keys.
#[derive(Deserialize, Debug)]
pub(crate) struct MultiGetRequest {
    /// List of base64url-encoded keys to retrieve.
    pub(crate) keys: Vec<String>,
}

/// Extracts the `?before_version` query parameter
#[derive(Deserialize, Debug, Default)]
pub(crate) struct BeforeVersion {
    #[serde(default)]
    pub(crate) before_version: bool,
}

/// Retrieves data associated with a given key from the KV store as raw
/// [`Bytes`](bytes::Bytes).
///
/// # Query Parameters
///
/// * `before_version` (optional, default `false`): only valid when `item_type`
///   is [`ItemType::Object`]. When `true`, returns the latest stored version
///   strictly less than the version encoded in the key. Returns `400 Bad
///   Request` if used with any other [`ItemType`].
///
/// # Returns
///
/// * If the key exists, the data is returned as a [`Bytes`](bytes::Bytes)
///   stream with a `200 OK` status code.
/// * If the key does not exist, a `404 Not Found` status code is returned with
///   an empty body.
/// * If an error occurs while interacting with the KV store, an `500 internal
///   server error` is returned.
pub async fn data_as_bytes(
    State(app_state): State<SharedRestServerAppState>,
    ExtractPath(key): ExtractPath,
    Query(BeforeVersion { before_version }): Query<BeforeVersion>,
) -> Result<impl IntoResponse, ApiError> {
    if before_version {
        let range = ObjectRangeKeyBound::try_from(key)
            .map_err(|_| ApiError::BadRequest(BEFORE_VERSION_REQUIRES_OB_ERROR_MSG.into()))?;

        let response = app_state
            .kv_store_client
            .object_before_version(range)
            .await?;

        return Ok(response.map_or_else(
            || (StatusCode::NOT_FOUND, Body::empty()).into_response(),
            |bytes| bytes.into_response(),
        ));
    }

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
/// # Query Parameters
///
/// * `before_version` (optional, default `false`): only valid when `item_type`
///   is [`ItemType::Object`]. When `true`, returns the latest stored version
///   strictly less than the version encoded in each key. Returns `400 Bad
///   Request` if used with any other [`ItemType`].
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
///  * If no keys are provided or the number of keys exceeds the configured
///    `multiget_max_items` limit, a `400 bad request error` is returned.
/// * If the keys cannot be parsed, a `400 bad request error` is returned.
/// * If an error occurs while interacting with the KV store, an `500 internal
///   server error` is returned.
pub async fn multi_get_data(
    State(app_state): State<SharedRestServerAppState>,
    Path(item_type): Path<ItemType>,
    Query(BeforeVersion { before_version }): Query<BeforeVersion>,
    Json(payload): Json<MultiGetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if payload.keys.is_empty() {
        return Err(ApiError::BadRequest("no keys provided".into()));
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

    let results = if before_version {
        let request = ObjectsBeforeVersionRequest::try_from(keys)
            .map_err(|_| ApiError::BadRequest(BEFORE_VERSION_REQUIRES_OB_ERROR_MSG.into()))?;
        app_state
            .kv_store_client
            .objects_before_version(request)
            .await?
    } else {
        app_state.kv_store_client.get_items(keys).await?
    };

    let bcs_data = bcs::to_bytes(&results).map_err(|_| ApiError::InternalServerError)?;
    Ok(bcs_data.into_response())
}

#[derive(Deserialize, Debug)]
pub(crate) struct TransactionDigestsByAddressQuery {
    pub(crate) cursor: Option<TransactionSequenceNumber>,
    pub(crate) limit: Option<NonZeroUsize>,
    #[serde(default)]
    pub(crate) oldest_first: bool,
}

/// Retrieves a paginated list of transactions that affect a given address.
///
/// An address is considered "affected" by a transaction if it appears as the
/// sender, a recipient, or the gas payer.
///
/// # Path Parameters
///
/// * `address`: Base64-url-encoded [`IotaAddress`].
///
/// # Query Parameters
///
/// * `cursor` (optional): The [`TransactionSequenceNumber`] used as an
///   exclusive pagination boundary. Omit for the first request.
/// * `limit` (optional): The maximum number of results to return. Defaults to
///   the server's configured `multiget_max_items` when omitted.
/// * `oldest_first` (optional, default `false`):
///   - `true`: Ascending sequence order (oldest first).
///   - `false`: Descending sequence order (newest first).
///
/// # Responses
///
/// * `200 OK`: A BCS-encoded `Vec<(TransactionSequenceNumber,
///   TransactionDigest)>`. Returns an empty list when no transaction digests
///   are found in the range scan.
/// * `400 Bad Request`: Returned if the provided `address` is malformed or
///   invalid.
/// * `500 Internal Server Error`: Returned if an error occurs interacting with
///   the KV store.
pub async fn transaction_digests_by_address(
    State(app_state): State<SharedRestServerAppState>,
    Path(address): Path<String>,
    Query(query): Query<TransactionDigestsByAddressQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let address = base64_url::decode(&address)
        .map_err(|_| ApiError::BadRequest("address is not valid base64-url".into()))?;

    let address = IotaAddress::from_bytes(&address)
        .map_err(|_| ApiError::BadRequest("invalid address".into()))?;

    let TransactionDigestsByAddressQuery {
        cursor,
        limit,
        oldest_first,
    } = query;

    let max_limit = app_state.multiget_max_items.get();
    let limit = limit.map_or(max_limit, |l| l.get());

    if limit > max_limit {
        return Err(ApiError::BadRequest(format!(
            "limit too large: maximum allowed is {max_limit}",
        )));
    }

    let transactions = app_state
        .kv_store_client
        .transactions_by_address(address, cursor, limit, oldest_first)
        .await?;

    let bcs_data = bcs::to_bytes(&transactions).map_err(|_| ApiError::InternalServerError)?;
    Ok(bcs_data.into_response())
}
