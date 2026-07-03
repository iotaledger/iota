// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use hyper::header::InvalidHeaderValue;
use iota_json_rpc_api::error_object_from_rpc;
use iota_json_rpc_types::IotaObjectResponseError;
use iota_types::error::{IotaError, UserInputError};
use jsonrpsee::{
    core::{ClientError as RpcError, RegisterMethodError},
    types::{
        ErrorObject, ErrorObjectOwned,
        error::{CALL_EXECUTION_FAILED_CODE, ErrorCode},
    },
};
use thiserror::Error;

pub type RpcInterimResult<T = ()> = Result<T, Error>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Iota(IotaError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),

    #[error("Deserialization error: {0}")]
    Bcs(#[from] bcs::Error),
    #[error("Unexpected error: {0}")]
    Unexpected(String),

    #[error(transparent)]
    RPCServer(#[from] RpcError),
    #[error(transparent)]
    RPCRegisterMethod(#[from] RegisterMethodError),

    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error(transparent)]
    UserInput(#[from] UserInputError),

    #[error(transparent)]
    IotaObjectResponse(#[from] IotaObjectResponseError),

    #[error(transparent)]
    IotaRpcInput(#[from] IotaRpcInputError),

    #[error("Unsupported Feature: {0}")]
    UnsupportedFeature(String),
}

impl From<IotaError> for Error {
    fn from(e: IotaError) -> Self {
        match e {
            IotaError::UserInput { error } => Self::UserInput(error),
            IotaError::UnsupportedFeature { error } => Self::UnsupportedFeature(error),
            other => Self::Iota(other),
        }
    }
}

impl From<Error> for RpcError {
    /// `InvalidParams`/`INVALID_PARAMS_CODE` for client errors.
    fn from(e: Error) -> RpcError {
        match e {
            Error::UserInput(_) | Error::UnsupportedFeature(_) => RpcError::Call(
                ErrorObject::owned::<()>(ErrorCode::InvalidRequest.code(), e.to_string(), None),
            ),
            Error::IotaObjectResponse(err) => match err {
                IotaObjectResponseError::NotExists { .. }
                | IotaObjectResponseError::DynamicFieldNotFound { .. }
                | IotaObjectResponseError::Deleted { .. }
                | IotaObjectResponseError::Display { .. } => {
                    RpcError::Call(ErrorObject::owned::<()>(
                        ErrorCode::InvalidParams.code(),
                        err.to_string(),
                        None,
                    ))
                }
                _ => RpcError::Call(ErrorObject::owned::<()>(
                    CALL_EXECUTION_FAILED_CODE,
                    err.to_string(),
                    None,
                )),
            },
            Error::IotaRpcInput(err) => RpcError::Call(ErrorObject::owned::<()>(
                ErrorCode::InvalidParams.code(),
                err.to_string(),
                None,
            )),
            Error::Iota(iota_error) => match iota_error {
                IotaError::TransactionNotFound { .. }
                | IotaError::TransactionsNotFound { .. }
                | IotaError::TransactionEventsNotFound { .. } => {
                    RpcError::Call(ErrorObject::owned::<()>(
                        ErrorCode::InvalidParams.code(),
                        iota_error.to_string(),
                        None,
                    ))
                }
                _ => RpcError::Call(ErrorObject::owned::<()>(
                    CALL_EXECUTION_FAILED_CODE,
                    iota_error.to_string(),
                    None,
                )),
            },
            _ => RpcError::Call(ErrorObject::owned::<()>(
                CALL_EXECUTION_FAILED_CODE,
                e.to_string(),
                None,
            )),
        }
    }
}

impl From<Error> for ErrorObjectOwned {
    fn from(value: Error) -> Self {
        error_object_from_rpc(value.into())
    }
}

#[derive(Debug, Error)]
pub enum IotaRpcInputError {
    #[error("Input exceeds limit of {0}")]
    SizeLimitExceeded(String),

    #[error("{0}")]
    GenericNotFound(String),

    #[error(
        "request_type` must set to `None` or `WaitForLocalExecution` if effects is required in the response"
    )]
    InvalidExecuteTransactionRequestType,

    #[error("Unsupported protocol version requested. Min supported: {0}, max supported: {1}")]
    ProtocolVersionUnsupported(u64, u64),

    #[error("{0}")]
    CannotParseIotaStructTag(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl From<IotaRpcInputError> for RpcError {
    fn from(e: IotaRpcInputError) -> Self {
        RpcError::Call(ErrorObject::owned::<()>(
            ErrorCode::InvalidParams.code(),
            e.to_string(),
            None,
        ))
    }
}

impl From<IotaRpcInputError> for ErrorObjectOwned {
    fn from(value: IotaRpcInputError) -> Self {
        error_object_from_rpc(value.into())
    }
}
