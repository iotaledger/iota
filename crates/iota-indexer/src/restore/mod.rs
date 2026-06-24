// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Restore the Indexer database through formal snapshots.

mod orchestration;
mod persist;
mod setup;
mod verify;

pub use orchestration::start;
pub use setup::Network;
