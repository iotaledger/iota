// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::error::FastCryptoError;
use iota_data_ingestion_core::IngestionError;
use iota_json_rpc_api::{error_object_from_rpc, internal_error};
use iota_types::{
    base_types::ObjectIDParseError,
    error::{IotaError, IotaObjectResponseError, UserInputError},
};
use jsonrpsee::{core::ClientError as RpcError, types::ErrorObjectOwned};
use thiserror::Error;

#[derive(Debug, Error)]
pub struct DataDownloadError {
    pub error: IndexerError,
    pub next_checkpoint_sequence_number: u64,
}

impl std::fmt::Display for DataDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "next_checkpoint_seq: {}, error: {}",
            self.next_checkpoint_sequence_number, self.error
        )
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IndexerError {
    #[error("Indexer failed to convert timestamp to NaiveDateTime with error: `{0}`")]
    DateTimeParsing(String),

    #[error("Indexer failed to deserialize event from events table with error: `{0}`")]
    EventDeserialization(String),

    #[error(
        "Fullnode returns unexpected responses, which may block indexers from proceeding, with error: `{0}`"
    )]
    UnexpectedFullnodeResponse(String),

    #[error("Indexer failed to transform data with error: `{0}`")]
    DataTransformation(String),

    #[error("Indexer failed to read fullnode with error: `{0}`")]
    FullNodeReading(String),

    #[error("Indexer failed to convert structs to diesel Insertable with error: `{0}`")]
    InsertableParsing(String),

    #[error("Indexer failed to build JsonRpcServer with error: `{0}`")]
    JsonRpcServer(#[from] iota_json_rpc::error::Error),

    #[error("Indexer failed to find object mutations, which should never happen.")]
    ObjectMutationNotAvailable,

    #[error("Indexer failed to build PG connection pool with error: `{0}`")]
    PgConnectionPoolInit(String),

    #[error("Indexer failed to get a pool connection from PG connection pool with error: `{0}`")]
    PgPoolConnection(String),

    #[error("Indexer failed to read PostgresDB with error: `{0}`")]
    PostgresRead(String),

    #[error("Indexer failed to reset PostgresDB with error: `{0}`")]
    PostgresReset(String),

    #[error("Indexer failed to commit changes to PostgresDB with error: `{0}`")]
    PostgresWrite(String),

    #[error(transparent)]
    Postgres(#[from] diesel::result::Error),

    #[error("Indexer failed to initialize fullnode Http client with error: `{0}`")]
    HttpClientInit(String),

    #[error("Indexer failed to serialize/deserialize with error: `{0}`")]
    Serde(String),

    #[error("Indexer error related to dynamic field: `{0}`")]
    DynamicField(String),

    #[error("Indexer does not support the feature with error: `{0}`")]
    NotSupported(String),

    #[error("Indexer read corrupted/incompatible data from persistent storage: `{0}`")]
    PersistentStorageDataCorruption(String),

    #[error("Indexer generic error: `{0}`")]
    Generic(String),

    #[error("Indexer failed to resolve object to move struct with error: `{0}`")]
    ResolveMoveStruct(String),

    #[error(transparent)]
    Uncategorized(#[from] anyhow::Error),

    #[error(transparent)]
    ObjectIdParse(#[from] ObjectIDParseError),

    #[error("Invalid transaction digest with error: `{0}`")]
    InvalidTransactionDigest(String),

    #[error(transparent)]
    Iota(#[from] IotaError),

    #[error(transparent)]
    Bcs(#[from] bcs::Error),

    #[error("Invalid argument with error: `{0}`")]
    InvalidArgument(String),

    #[error(transparent)]
    UserInput(#[from] UserInputError),

    #[error("Indexer failed to resolve module with error: `{0}`")]
    ModuleResolution(String),

    #[error(transparent)]
    ObjectResponse(#[from] IotaObjectResponseError),

    #[error(transparent)]
    FastCrypto(#[from] FastCryptoError),

    #[error("`{0}`: `{1}`")]
    ErrorWithContext(String, Box<IndexerError>),

    #[error("Indexer failed to send item to channel with error: `{0}`")]
    MpscChannel(String),

    #[error("Failed to process checkpoint(s): `{0}`")]
    CheckpointProcessing(String),

    #[error(transparent)]
    Ingestion(#[from] IngestionError),
}

pub trait Context<T> {
    fn context(self, context: &str) -> Result<T, IndexerError>;
}

impl<T> Context<T> for Result<T, IndexerError> {
    fn context(self, context: &str) -> Result<T, IndexerError> {
        self.map_err(|e| IndexerError::ErrorWithContext(context.to_string(), Box::new(e)))
    }
}

impl From<IndexerError> for RpcError {
    fn from(e: IndexerError) -> Self {
        RpcError::Call(internal_error(e))
    }
}

impl From<IndexerError> for ErrorObjectOwned {
    fn from(value: IndexerError) -> Self {
        error_object_from_rpc(value.into())
    }
}

impl From<tokio::task::JoinError> for IndexerError {
    fn from(value: tokio::task::JoinError) -> Self {
        IndexerError::Uncategorized(anyhow::Error::from(value))
    }
}
