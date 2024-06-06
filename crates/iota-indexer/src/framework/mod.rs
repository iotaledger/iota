// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod builder;
pub mod interface;

// TODO remove the pub(crater) once indexer.rs is renamed to lib.rs
pub(crate) mod fetcher;
pub(crate) mod runner;

pub use builder::IndexerBuilder;
pub use interface::Handler;
