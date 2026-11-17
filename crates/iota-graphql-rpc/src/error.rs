// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{ErrorExtensionValues, ErrorExtensions, Pos, Response, ServerError};
use async_graphql_axum::GraphQLResponse;
use iota_indexer::{
    errors::IndexerError, pruning::oldest_available_checkpoint::OldestAvailableCheckpoint,
};
use iota_indexer_streaming::error::IndexerStreamingError;
use iota_names::error::IotaNamesError;

/// Error codes for the `extensions.code` field of a GraphQL error that
/// originates from outside GraphQL.
/// `<https://www.apollographql.com/docs/apollo-server/data/errors/#built-in-error-codes>`
pub(crate) mod code {
    pub const BAD_REQUEST: &str = "BAD_REQUEST";
    pub const BAD_USER_INPUT: &str = "BAD_USER_INPUT";
    pub const DATA_PRUNED: &str = "DATA_PRUNED";
    pub const INTERNAL_SERVER_ERROR: &str = "INTERNAL_SERVER_ERROR";
    pub const REQUEST_TIMEOUT: &str = "REQUEST_TIMEOUT";
    pub const TOO_MANY_REQUESTS: &str = "TOO_MANY_REQUESTS";
    pub const UNKNOWN: &str = "UNKNOWN";
}

/// Create a GraphQL Response containing an Error.
///
/// Most errors produced by the service will automatically be wrapped in a
/// `GraphQLResponse`, because they will originate from within the GraphQL
/// implementation.  This function is intended for errors that originated from
/// outside of GraphQL (such as in middleware), but that need to be ingested by
/// GraphQL clients.
pub(crate) fn graphql_error_response(code: &str, message: impl Into<String>) -> GraphQLResponse {
    let error = graphql_error(code, message);
    Response::from_errors(error.into()).into()
}

/// Create a generic GraphQL Server Error.
///
/// This error has no path, source, or locations, just a message and an error
/// code.
pub(crate) fn graphql_error(code: &str, message: impl Into<String>) -> ServerError {
    let mut ext = ErrorExtensionValues::default();
    ext.set("code", code);

    ServerError {
        message: message.into(),
        source: None,
        locations: vec![],
        path: vec![],
        extensions: Some(ext),
    }
}

pub(crate) fn graphql_error_at_pos(
    code: &str,
    message: impl Into<String>,
    pos: Pos,
) -> ServerError {
    let mut ext = ErrorExtensionValues::default();
    ext.set("code", code);

    ServerError {
        message: message.into(),
        source: None,
        locations: vec![pos],
        path: vec![],
        extensions: Some(ext),
    }
}

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Unsupported protocol version requested. Min supported: {0}, max supported: {1}")]
    ProtocolVersionUnsupported(u64, u64),
    #[error("'first' and 'last' must not be used together")]
    CursorNoFirstLast,
    #[error("Connection's page size of {0} exceeds max of {1}")]
    PageTooLarge(u64, u32),
    // Catch-all for client-fault errors
    #[error("{0}")]
    Client(String),
    #[error("Requested data has been pruned: {message}")]
    DataPruned {
        message: String,
        /// The oldest checkpoint the request could have found data in.
        ///
        /// Anything before it has been pruned and is no longer served.
        oldest_available_checkpoint: OldestAvailableCheckpoint,
    },
    #[error("Internal error occurred while processing request: {0}")]
    Internal(String),
    #[error(transparent)]
    IotaNames(#[from] IotaNamesError),
    #[error("{0}")]
    ServerInit(String),
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
    /// The fullnode refused the request for now, and the client can retry it
    /// later.
    #[error("{0}")]
    TooManyRequests(String),
}

impl ErrorExtensions for Error {
    fn extend(&self) -> async_graphql::Error {
        async_graphql::Error::new(format!("{self}")).extend_with(|_err, e| match self {
            Error::CursorNoFirstLast
            | Error::PageTooLarge(_, _)
            | Error::ProtocolVersionUnsupported(_, _)
            | Error::Client(_) => {
                e.set("code", code::BAD_USER_INPUT);
            }
            Error::DataPruned {
                oldest_available_checkpoint,
                ..
            } => {
                e.set("code", code::DATA_PRUNED);
                e.set(
                    "oldestAvailableCheckpoint",
                    u64::from(*oldest_available_checkpoint),
                );
            }
            Error::Internal(_) => {
                e.set("code", code::INTERNAL_SERVER_ERROR);
            }
            Error::IotaNames(_) => {
                e.set("code", code::BAD_REQUEST);
            }
            Error::ServerInit(_) => {
                e.set("code", code::UNKNOWN);
            }
            Error::UnsupportedFeature(_) => {
                e.set("code", code::BAD_REQUEST);
            }
            Error::TooManyRequests(_) => {
                e.set("code", code::TOO_MANY_REQUESTS);
            }
        })
    }
}

impl From<IndexerError> for Error {
    fn from(e: IndexerError) -> Self {
        match e {
            IndexerError::DataPruned {
                message,
                oldest_available_checkpoint,
            } => Error::DataPruned {
                message,
                oldest_available_checkpoint,
            },
            IndexerError::ServerBusy(message) => Error::TooManyRequests(message),
            _ => Error::Internal(e.to_string()),
        }
    }
}

/// The GraphQL error for a view call that failed in the write API of the
/// indexer. A refusal of the fullnode stays a refusal; any other error reaches
/// the client as the text of the JSON-RPC error of the write API, with no
/// GraphQL error code.
pub(crate) fn view_call_error(err: IndexerError) -> async_graphql::Error {
    match err {
        IndexerError::ServerBusy(message) => Error::TooManyRequests(message).extend(),
        err => jsonrpsee::types::ErrorObjectOwned::from(err).into(),
    }
}

impl From<IndexerStreamingError> for Error {
    fn from(e: IndexerStreamingError) -> Self {
        match e {
            IndexerStreamingError::RecoveryDistanceExceeded { .. }
            | IndexerStreamingError::NotFound(_) => Error::Client(e.to_string()),
            _ => Error::Internal(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refusal of the fullnode reaches a GraphQL client with the code
    /// `TOO_MANY_REQUESTS` and the message of the fullnode.
    #[test]
    fn a_refusal_of_the_fullnode_reaches_the_client_as_a_refusal() {
        let error = Error::from(IndexerError::ServerBusy("Too many requests".to_string())).extend();
        assert_eq!(error.message, "Too many requests");
        assert_eq!(
            error.extensions.unwrap().get("code"),
            Some(&async_graphql::Value::from(code::TOO_MANY_REQUESTS))
        );
        let error = Error::from(IndexerError::Grpc("down".to_string())).extend();
        assert_eq!(
            error.extensions.unwrap().get("code"),
            Some(&async_graphql::Value::from(code::INTERNAL_SERVER_ERROR))
        );
    }

    /// A view call refused by the fullnode reaches a GraphQL client as a
    /// refusal, and other errors of a view call reach it as the text of the
    /// JSON-RPC error of the write API, with no GraphQL error code.
    #[test]
    fn a_view_call_refusal_is_a_refusal_and_other_errors_are_unchanged() {
        let error = view_call_error(IndexerError::ServerBusy("Too many requests".to_string()));
        assert_eq!(error.message, "Too many requests");
        assert_eq!(
            error.extensions.unwrap().get("code"),
            Some(&async_graphql::Value::from(code::TOO_MANY_REQUESTS))
        );
        let error = view_call_error(IndexerError::InvalidArgument(
            "no such function".to_string(),
        ));
        assert!(
            error.message.contains("no such function"),
            "{}",
            error.message
        );
        assert!(error.extensions.is_none());
    }
}
