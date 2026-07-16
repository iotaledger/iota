// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use jsonrpsee::types::{ErrorObject, ErrorObjectOwned, error::ErrorCode};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Unsupported Feature: {0}")]
    UnsupportedFeature(String),
}

impl From<Error> for ErrorObjectOwned {
    fn from(e: Error) -> ErrorObjectOwned {
        match e {
            Error::UnsupportedFeature(_) => {
                ErrorObject::owned::<()>(ErrorCode::InvalidRequest.code(), e.to_string(), None)
            }
        }
    }
}
