// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Core data types for blocks in the tangle.

#[macro_use]
mod r#macro;
mod convert;
mod error;
mod ids;

/// A module that provides types and syntactic validations of addresses.
pub mod address;
/// A module that provides types and syntactic validations of outputs.
pub mod output;

pub use ids::{BlockId, TransactionId};
pub(crate) use r#macro::create_bitflags;

pub use self::{convert::ConvertTo, error::Error};
