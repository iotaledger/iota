// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "rpc_tests")]
#[path = "../common/mod.rs"]
mod common;

#[cfg(feature = "pg_integration")]
mod extended_api;

#[cfg(feature = "rpc_tests")]
mod read_api;
