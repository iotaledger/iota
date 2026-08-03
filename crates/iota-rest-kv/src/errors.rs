// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes the error types the REST API sends back to the client.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use iota_storage::http_key_value_store::ItemType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An Error type which represents the possible errors the REST API server can
/// send back to the client.
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("internal server error")]
    InternalServerError(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        match self {
            ApiError::InternalServerError(ref e) => {
                tracing::Span::current().record("error", format_args!("{e:#}"));
            }
            ApiError::BadRequest(ref e) => {
                tracing::Span::current().record("error", format_args!("{e}"));
            }
            ApiError::NotFound => {}
        }

        let body = Json(ErrorResponse {
            error_code: status_code.as_u16().to_string(),
            error_message: self.to_string(),
        });

        (status_code, body).into_response()
    }
}

/// Describes the response body of a unsuccessful HTTP request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error_code: String,
    pub(crate) error_message: String,
}

#[derive(Error, Debug)]
pub enum RangeKeyBoundError {
    #[error("expected `{expected}` item type: {detail}")]
    UnexpectedItemType { expected: ItemType, detail: String },
}

impl From<RangeKeyBoundError> for ApiError {
    fn from(err: RangeKeyBoundError) -> Self {
        ApiError::BadRequest(err.to_string())
    }
}
