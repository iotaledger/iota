// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The [`stardust`] module incorporates all the logic necessary for
//! parsing Stardust UTXOs from a full-snapshot file, and converting
//! them to the appropriate genesis objects.
pub mod error;
pub mod migration;
#[cfg(test)]
mod migration_tests;
pub mod native_token;
pub mod parse;
pub mod types;
