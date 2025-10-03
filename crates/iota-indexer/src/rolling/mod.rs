// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rolling version of the library.

pub(crate) mod error;
pub(crate) mod extract;
pub(crate) mod persist;
pub(crate) mod transform;
pub use transform::CheckpointObjectChanges;
