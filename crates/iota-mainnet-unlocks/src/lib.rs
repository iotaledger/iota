// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod aggregated_data;
/// Provides functionality for loading and querying token unlock data over time.
///
/// This module defines a `MainnetUnlocksStore` which maintains an in-memory
/// mapping of timestamps to the amount of still-locked tokens at that point in
/// time.
///
/// The unlock data is used to answer questions like:
/// - "How many tokens are still locked at a specific timestamp?"
pub mod store;

pub use store::{MainnetUnlocksStore, StillLockedEntry};
