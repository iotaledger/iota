// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "pg_integration")]
#[path = "../common/mod.rs"]
mod common;

#[cfg(feature = "pg_integration")]
mod extended_api;

#[cfg(feature = "pg_integration")]
mod indexer_api;

#[cfg(feature = "pg_integration")]
mod read_api;
