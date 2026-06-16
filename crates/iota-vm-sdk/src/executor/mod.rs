// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`LocalVm`] — the single store -> execute -> introspect executor.
//!
//! `LocalVm` owns a [`Store`](crate::Store) and a Move execution engine
//! configured for the chain described by a [`ChainContext`]. Each
//! [`LocalVm::execute`] / [`LocalVm::execute_signed`] call runs a transaction
//! in one of three [`ExecutionMode`]s:
//!
//! - [`ExecutionMode::DevInspect`] — relaxed Move VM checks, no commit.
//! - [`ExecutionMode::DryRun`] — full sign-time checks, no commit.
//! - [`ExecutionMode::Execute`] — full checks; on success the effects (writes
//!   *and* deletions) are applied back into the store and
//!   [`ExecutionResult::committed`] is `true`.
//!
//! `DevInspect`/`DryRun` leave the store untouched.
//!
//! The module is split into:
//! - [`types`] — the public input/output types.
//! - [`local_vm`] — the [`LocalVm`] executor and its public API.
//! - [`env`] — the per-run execution environment and engine/profile wiring.
//! - [`prepare`] — shared transaction preparation, execution, and event decode.

mod env;
mod local_vm;
mod prepare;
mod types;

pub use local_vm::LocalVm;
pub use types::{
    ChainContext, DecodedEvent, ExecuteOptions, ExecutionMode, ExecutionResult, SignatureStatus,
};
