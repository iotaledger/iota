// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Debug;

use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use fastcrypto::error::FastCryptoError;
use iota_types::error::IotaError;
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use strum::{EnumProperty, IntoEnumIterator};
use strum_macros::{Display, EnumDiscriminants, EnumIter};
use thiserror::Error;
use typed_store::TypedStoreError;

use crate::types::{BlockHash, IotaEnv, OperationType, PublicKey};

/// IOTA Rosetta specific error types.
/// This contains all the errors returns by the iota-rosetta server.
#[derive(Debug, Error, EnumDiscriminants, EnumProperty)]
#[strum_discriminants(
    name(ErrorType),
    derive(Display, EnumIter),
    strum(serialize_all = "kebab-case")
)]
pub enum Error {
    #[error("Unsupported blockchain: {0}")]
    UnsupportedBlockchain(String),
    #[error("Unsupported network: {0:?}")]
    UnsupportedNetwork(IotaEnv),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Missing input: {0}")]
    MissingInput(String),
    #[error("Missing metadata")]
    MissingMetadata,
    #[error("{0}")]
    MalformedOperation(String),
    #[error("Unsupported operation: {0:?}")]
    UnsupportedOperation(OperationType),
    #[error("Data error: {0}")]
    Data(String),
    #[error("Block not found, index: {index:?}, hash: {hash:?}")]
    BlockNotFound {
        index: Option<u64>,
        hash: Option<BlockHash>,
    },
    #[error("Public key deserialization error: {0:?}")]
    PublicKeyDeserialization(PublicKey),

    #[error("Error executing transaction: {0}")]
    TransactionExecution(String),

    #[error("{0}")]
    TransactionDryRun(String),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
    #[error(transparent)]
    BCSSerialization(#[from] bcs::Error),
    #[error(transparent)]
    Crypto(#[from] FastCryptoError),
    #[error(transparent)]
    Iota(#[from] IotaError),
    #[error(transparent)]
    IotaRpc(#[from] iota_sdk::error::Error),
    #[error(transparent)]
    Encoding(#[from] eyre::Report),
    #[error(transparent)]
    DB(#[from] TypedStoreError),
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),

    #[error("Retries exhausted while getting balance. try again.")]
    #[strum(props(retriable = "true"))]
    RetryExhausted(String),
}

impl Serialize for ErrorType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.json().serialize(serializer)
    }
}

trait CustomProperties {
    fn retriable(&self) -> bool;
}

impl CustomProperties for Error {
    fn retriable(&self) -> bool {
        matches!(self.get_str("retriable"), Some("true"))
    }
}

impl ErrorType {
    fn json(&self) -> Value {
        let retriable = false;
        // Safe to unwrap
        let error_code = ErrorType::iter().position(|e| &e == self).unwrap();
        let message = format!("{self}").replace('-', " ");
        let message = message[0..1].to_uppercase() + &message[1..];

        json![{
            "code": error_code,
            "message": message,
            "retriable": retriable,
        }]
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let type_: ErrorType = self.into();
        let mut json = type_.json();
        // Safe to unwrap, we know ErrorType must be an object.
        let error = json.as_object_mut().unwrap();
        error.insert("details".into(), json!({ "error": format!("{self}") }));
        error.insert("retriable".into(), json!(self.retriable()));
        error.serialize(serializer)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}
