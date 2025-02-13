// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes the error types the REST API sends back to the client.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
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
    InternalServerError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorResponse {
            error_code: status_code.as_u16().to_string(),
            error_message: self.to_string(),
        });

        (status_code, body).into_response()
    }
}

/// Describes the response body of a unsuccessful HTTP request.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ErrorResponse {
    error_code: String,
    error_message: String,
}
