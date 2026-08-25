// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use move_core_types::account_address::AccountAddress;
use serde::{Deserialize, Serialize};

use super::Publication;
use crate::{
    flavor::MoveFlavor,
    schema::{EnvironmentID, EnvironmentName},
};

/// Publish information for a package
#[derive(Debug, Serialize, Deserialize)]
pub struct PublishInformation<F: MoveFlavor> {
    /// This is usually the `chain_id`. We need to decide if we really want to
    /// abstract the concept of "environments".
    pub environment: EnvironmentID,
    /// The IDs (original, published_at) for the package.
    pub publication: Publication<F>,
}
