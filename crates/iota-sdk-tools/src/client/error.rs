// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Client errors.

use iota_types::base_types::IotaAddress;
use thiserror::Error;

pub(crate) type ClientResult<T> = std::result::Result<T, ClientError>;

#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum ClientError {
    #[error(
        "Gas budget {budget} is less than the reference gas price {gas_price}. The gas budget must be at least the current reference gas price of {gas_price}."
    )]
    InsufficientGasBudget { budget: u64, gas_price: u64 },
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("failed to parse move object from raw data")]
    ParseMoveObject,
    #[error(
        "Cannot find gas coin for signer address [{signer}] with amount sufficient for the required gas amount [{gas_budget}]."
    )]
    MissingGasCoin {
        signer: IotaAddress,
        gas_budget: u64,
    },
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("transaction error: {0:#?}")]
    TransactionFailure(String),
    #[error("execution errors: {0:#?}")]
    Execution(Vec<String>),
    #[error("missing object changes")]
    MissingObjectChanges,
    #[error("cannot obtain home directory path")]
    MissingHomeDirectory,

    #[error("IOTA error: {0}")]
    IotaSdk(#[from] iota_sdk::error::Error),
    #[error("IOTA error: {0}")]
    IotaTypes(#[from] iota_types::error::IotaError),
    #[error("IOTA user input error: {0}")]
    IotaUserInput(#[from] iota_types::error::UserInputError),
    #[error("IOTA object response error: {0}")]
    IotaObjectResponse(#[from] iota_types::error::IotaObjectResponseError),
    #[error("signature error: {0}")]
    Signature(#[from] signature::Error),

    #[error("json conversion error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bcs conversion error: {0}")]
    BCS(#[from] bcs::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ClientError {
    #[allow(unused)]
    pub(crate) fn other(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Other(Box::new(e))
    }
}
