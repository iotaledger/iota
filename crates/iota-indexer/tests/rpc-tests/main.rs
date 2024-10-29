// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(dead_code)]
#[path = "../common/mod.rs"]
mod common;
#[cfg(feature = "shared_test_runtime")]
mod extended_api;
#[cfg(feature = "shared_test_runtime")]
mod governance_api;
#[cfg(feature = "shared_test_runtime")]
mod indexer_api;
#[cfg(feature = "shared_test_runtime")]
mod move_utils;

#[cfg(feature = "shared_test_runtime")]
mod read_api;

#[cfg(feature = "shared_test_runtime")]
mod write_api;
