// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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

// #[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize,
// EnumVariantOrder)] pub enum ExecutionFailureStatus {}
