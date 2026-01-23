// Copyright (c) 2026 IOTA Stiftung
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
/// Payload types.
pub mod payload;
/// Protocol parameters.
pub mod protocol;

pub use ids::{BlockId, MilestoneId, MilestoneIndex, TransactionId};

pub use self::{
    convert::ConvertTo,
    error::Error,
    payload::{MilestoneOption, ParametersMilestoneOption},
    protocol::{PROTOCOL_VERSION, ProtocolParameters},
};
