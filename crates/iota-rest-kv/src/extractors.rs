// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes custom extractors needed for validation and custom
//! errors messages on the input provided by the client.

use core::str;

use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
};
use iota_storage::http_key_value_store::{ItemType, Key};
use serde::Deserialize;

use crate::errors::ApiError;

/// Path segment labels will be matched with struct field names.
#[derive(Deserialize, Debug)]
struct RequestParams {
    /// The supported items that are associated with the [`Key`].
    item_type: ItemType,
    /// The **digest**, **object id**, or a **checkpoint sequence number**
    /// encoded as [`base64_url`].
    key: String,
}

/// We define our own extractor that includes validation and custom error
/// message.
///
/// This custom extractor matches [`Path`] segments and deserilize them
/// internally into [`RequestParams`] and constructs a [`Key`].
pub struct ExtractPath(pub Key);

impl<S> FromRequestParts<S> for ExtractPath
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Path::<RequestParams>::from_request_parts(parts, state).await {
            Ok(value) => {
                // based on the item type and encoded key construct the Key enum
                let key = Key::new(value.item_type.to_string().as_str(), value.key.as_str())
                    .map_err(|err| ApiError::BadRequest(format!("invalid input: {err}")))?;
                Ok(ExtractPath(key))
            }
            Err(e) => Err(ApiError::BadRequest(format!(
                "invalid path parameter provided: {e}",
            ))),
        }
    }
}
