// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes custom extractors needed for validation and custom
//! errors messages on the input provided by the client.

use core::str;

use axum::{
    async_trait,
    extract::{FromRequestParts, Path},
    http::request::Parts,
};
use iota_storage::http_key_value_store::{Key, TaggedKey};
use iota_types::{
    digests::{CheckpointDigest, TransactionDigest},
    storage::ObjectKey,
};
use serde::Deserialize;

use crate::{errors::ApiError, types::ItemType};

/// Path segment labels will be matched with struct field names.
#[derive(Deserialize, Debug)]
struct RequestParams {
    /// The **digest**, **object id**, or a **checkpoint sequence number**
    /// encoded as [`base64_url`].
    key: String,
    /// The supported items that are associated with the [`Key`].
    item_type: ItemType,
}

/// We define our own extractor that includes validation and custom error
/// message.
///
/// This custom extractor matches [`Path`] segments and deserilize them
/// internally into [`RequestParams`] and constructs a [`Key`].
pub struct ExtractPath(pub Key);

#[async_trait]
impl<S> FromRequestParts<S> for ExtractPath
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Path::<RequestParams>::from_request_parts(parts, state).await {
            Ok(value) => {
                // based on the item type construct the Key enum
                let key = path_elements_to_key(&value.key, value.item_type)?;
                Ok(ExtractPath(key))
            }
            Err(e) => Err(ApiError::BadRequest(format!(
                "invalid path parameter provided: {e}",
            ))),
        }
    }
}

/// Create a a [`Key`] instance based on the provided [`base64_url`] encoded
/// string and item type.
pub fn path_elements_to_key(digest: &str, item_type: ItemType) -> Result<Key, ApiError> {
    let decoded_key = base64_url::decode(digest)
        .map_err(|err| ApiError::BadRequest(format!("invalid base64 url value: {err}")))?;

    match item_type {
        ItemType::Tx => Ok(Key::Tx(
            TransactionDigest::try_from(decoded_key.as_slice())
                .map_err(|err| ApiError::BadRequest(err.to_string()))?,
        )),
        ItemType::Fx => Ok(Key::Fx(
            TransactionDigest::try_from(decoded_key.as_slice())
                .map_err(|err| ApiError::BadRequest(err.to_string()))?,
        )),
        ItemType::CheckpointContents => {
            let tagged_key = bcs::from_bytes(&decoded_key)
                .map_err(|err| ApiError::BadRequest(err.to_string()))?;
            match tagged_key {
                TaggedKey::CheckpointSequenceNumber(seq) => Ok(Key::CheckpointContents(seq)),
            }
        }
        ItemType::CheckpointSummary => {
            // first try to decode as digest, otherwise try to decode as tagged key
            match CheckpointDigest::try_from(decoded_key.clone()) {
                Err(_) => {
                    let tagged_key = bcs::from_bytes(&decoded_key)
                        .map_err(|err| ApiError::BadRequest(err.to_string()))?;
                    match tagged_key {
                        TaggedKey::CheckpointSequenceNumber(seq) => Ok(Key::CheckpointSummary(seq)),
                    }
                }
                Ok(cs_digest) => Ok(Key::CheckpointSummaryByDigest(cs_digest)),
            }
        }
        ItemType::TxToCheckpoint => Ok(Key::TxToCheckpoint(
            TransactionDigest::try_from(decoded_key.as_slice())
                .map_err(|err| ApiError::BadRequest(err.to_string()))?,
        )),
        ItemType::ObjectKey => {
            let object_key: ObjectKey = bcs::from_bytes(&decoded_key)
                .map_err(|err| ApiError::BadRequest(err.to_string()))?;
            Ok(Key::ObjectKey(object_key.0, object_key.1))
        }
        ItemType::EventsByTxDigest => Ok(Key::EventsByTxDigest(
            TransactionDigest::try_from(decoded_key.as_slice())
                .map_err(|err| ApiError::BadRequest(err.to_string()))?,
        )),
    }
}
