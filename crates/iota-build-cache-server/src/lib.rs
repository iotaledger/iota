// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IOTA Build Cache Server Library
//!
//! This crate provides both a standalone build cache server binary and a client
//! library for interacting with build cache servers.

pub mod cache;
pub mod client;
pub mod server;
pub mod types;

// Re-export commonly used types
pub use client::{BuildCacheClient, BuildCacheError};
pub use types::{BuildCacheResponse, BuildJob, BuildRequest, BuildStatus};
