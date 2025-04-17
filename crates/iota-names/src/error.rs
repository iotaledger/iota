// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::base_types::ObjectID;
use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum IotaNamesError {
    #[error("Name Service: String length: {0} exceeds maximum allowed length: {1}")]
    ExceedsMaxLength(usize, usize),
    #[error("Name Service: String length: {0} outside of valid range: [{1}, {2}]")]
    InvalidLength(usize, usize, usize),
    #[error("Name Service: Hyphens are not allowed as the first or last character")]
    InvalidHyphens,
    #[error("Name Service: Only lowercase letters, numbers, and hyphens are allowed")]
    InvalidUnderscore,
    #[error("Name Service: Domain must contain at least one label")]
    LabelsEmpty,
    #[error("Name Service: Domain must include only one separator")]
    InvalidSeparator,
    #[error("Name Service: Name has expired")]
    NameExpired,
    #[error("Name Service: Malformed object for {0}")]
    MalformedObject(ObjectID),
}
