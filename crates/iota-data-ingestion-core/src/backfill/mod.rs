// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Backfill of a bounded checkpoint range.

mod executor;
mod options;
mod order;
pub(crate) mod source;

pub use executor::BackfillExecutor;
pub use options::BackfillOptions;
pub use order::IngestionOrder;
