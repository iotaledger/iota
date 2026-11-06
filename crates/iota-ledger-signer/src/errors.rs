// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_ledger::LedgerError;
pub use iota_sdk::{error::Error as IotaSdkError, types::error::UserInputError};
use iota_sdk_types::ObjectId;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerSignerError {
    #[error("Ledger error: {0}")]
    Ledger(#[from] LedgerError),

    #[error("IotaSdk error: {0}")]
    Sdk(#[from] IotaSdkError),

    #[error("UserInput error: {0}")]
    UserInput(#[from] UserInputError),

    #[error("Failed to load object {object_id} for clear signing: {reason}")]
    ObjectLoad { object_id: ObjectId, reason: String },

    #[error("Expected {expected} objects for clear signing, but the RPC returned {actual}")]
    ObjectCount { expected: usize, actual: usize },
}
