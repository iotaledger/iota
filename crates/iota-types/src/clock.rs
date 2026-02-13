// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::Identifier;
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::{IOTA_FRAMEWORK_ADDRESS, id::UID};

pub const RESOLVED_IOTA_CLOCK: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("clock"),
    ident_str!("Clock"),
);
pub const CONSENSUS_COMMIT_PROLOGUE_FUNCTION_NAME: Identifier =
    Identifier::from_static("consensus_commit_prologue");

#[derive(Debug, Serialize, Deserialize)]
pub struct Clock {
    pub id: UID,
    pub timestamp_ms: u64,
}

impl Clock {
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    /// Detects a `&mut iota::clock::Clock` or `iota::clock::Clock` in the
    /// signature.
    pub fn is_mutable(view: &CompiledModule, s: &SignatureToken) -> bool {
        use SignatureToken as S;
        match s {
            S::MutableReference(inner) => Self::is_mutable(view, inner),
            S::Datatype(idx) => resolve_struct(view, *idx) == RESOLVED_IOTA_CLOCK,
            _ => false,
        }
    }
}
