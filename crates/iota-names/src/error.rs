// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::base_types::ObjectID;
use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum IotaNamesError {
    #[error("String length: {0} exceeds maximum allowed length: {1}")]
    ExceedsMaxLength(usize, usize),
    #[error("String length: {0} outside of valid range: [{1}, {2}]")]
    InvalidLength(usize, usize, usize),
    #[error("Hyphens are not allowed as the first or last character")]
    HyphensAsFirstOrLastChar,
    #[error("Only lowercase letters, numbers, and hyphens are allowed as label characters")]
    InvalidLabelChar,
    #[error("Domain must contain at least one label")]
    LabelsEmpty,
    #[error("Domain must include only one separator")]
    InvalidSeparator,
    #[error("Name has expired")]
    NameExpired,
    #[error("Malformed object for {0}")]
    MalformedObject(ObjectID),
}
