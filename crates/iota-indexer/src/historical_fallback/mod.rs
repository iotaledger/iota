// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Provides the necessary types and client implementation for querying
//! historical fallback data from KV Store.
//!
//! Its primary use is to support fetching historical data to compensate for
//! missing data from the Postgres database when the pruning feature is enabled
//! for the JSON-RPC API.

pub(crate) mod client;
pub(crate) mod convert;
pub(crate) mod metrics;
pub(crate) mod reader;
