// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rolling version of the library.
//!
//! This is to phase in breaking changes that may or may not
//! include refactoring of the existing API.
//!
//! This will remain private until the versioning scheme is stabilized.
//!
//! In cases where the [`rolling`] module is used in other modules above, it
//! MUST be used only in private APIs, or in inner implementations.

pub(crate) mod error;
pub(crate) mod extract;
pub(crate) mod persist;
pub(crate) mod transform;
