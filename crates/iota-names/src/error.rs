// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum IotaNamesError {
    #[error("Name length {0} exceeds maximum length {1}")]
    NameLengthExceeded(usize, usize),
    #[error("Label length {0} outside of valid range [{1}, {2}]")]
    InvalidLabelLength(usize, usize, usize),
    #[error("Hyphens are not allowed as first or last character of a label")]
    HyphensAsFirstOrLastLabelChar,
    #[error(
        "Only lowercase letters, numbers, and hyphens are allowed as label characters. Got \'{0}\' at position {1}"
    )]
    InvalidLabelChar(char, usize),
    #[error("Name must contain at least two labels, TLN and SLN")]
    NotEnoughLabels,
    #[error("Name must include only one separator")]
    InvalidSeparator,
    #[error("Name has expired")]
    NameExpired,
    #[error("Malformed object for {0}")]
    MalformedObject(ObjectId),
    #[error("Invalid TLN {0}")]
    InvalidTln(String),
}
