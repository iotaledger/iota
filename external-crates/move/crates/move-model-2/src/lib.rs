// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[macro_use(sp)]
extern crate move_ir_types;

pub mod compiled_model;
pub mod display;
pub mod model;
pub mod normalized;
pub mod source_model;

pub use normalized::{ModuleId, QualifiedMemberId, TModuleId};
