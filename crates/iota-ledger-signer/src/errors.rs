// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_ledger::LedgerError;
pub use iota_sdk::{error::Error as IotaSdkError, types::error::UserInputError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerSignerError {
    #[error("Ledger error: {0}")]
    Ledger(#[from] LedgerError),

    #[error("IotaSdk error: {0}")]
    Sdk(#[from] IotaSdkError),

    #[error("UserInput error: {0}")]
    UserInput(#[from] UserInputError),
}
