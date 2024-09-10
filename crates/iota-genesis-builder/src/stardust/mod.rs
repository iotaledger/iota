// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The `stardust` module incorporates all the logic necessary for
//! parsing Stardust UTXOs from a full-snapshot file, and converting
//! them to the appropriate genesis objects.

pub mod migration;
pub mod native_token;
pub mod parse;
#[cfg(feature = "test-outputs")]
pub mod test_outputs;
pub mod types;
