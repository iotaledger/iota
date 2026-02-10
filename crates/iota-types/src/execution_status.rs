// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{self};

use serde::{Deserialize, Serialize};

use crate::ObjectID;

pub type ExecutionStatus = iota_sdk_types::ExecutionStatus;
pub type ExecutionFailureStatus = iota_sdk_types::ExecutionError;
pub type PackageUpgradeError = iota_sdk_types::PackageUpgradeError;
pub type CommandArgumentError = iota_sdk_types::CommandArgumentError;
pub type TypeArgumentError = iota_sdk_types::TypeArgumentError;
pub type MoveLocation = iota_sdk_types::MoveLocation;
pub type MoveLocationOpt = iota_sdk_types::MoveLocationOpt;

#[cfg(test)]
#[path = "unit_tests/execution_status_tests.rs"]
mod execution_status_tests;

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CongestedObjects(pub Vec<ObjectID>);

impl fmt::Display for CongestedObjects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for obj in &self.0 {
            write!(f, "{obj}, ")?;
        }
        Ok(())
    }
}

// #[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, EnumVariantOrder)]
// pub enum ExecutionFailureStatus {}
