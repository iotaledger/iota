// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::Identifier;
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};

use crate::IOTA_FRAMEWORK_ADDRESS;

pub const RESOLVED_IOTA_CLOCK: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("clock"),
    ident_str!("Clock"),
);
pub const CONSENSUS_COMMIT_PROLOGUE_FUNCTION_NAME: Identifier =
    Identifier::from_static("consensus_commit_prologue");

/// Detects a `&mut iota::clock::Clock` or `iota::clock::Clock` in the
/// signature.
pub fn is_mutable_clock(view: &CompiledModule, s: &SignatureToken) -> bool {
    use SignatureToken as S;
    match s {
        S::MutableReference(inner) => is_mutable_clock(view, inner),
        S::Datatype(idx) => resolve_struct(view, *idx) == RESOLVED_IOTA_CLOCK,
        _ => false,
    }
}
