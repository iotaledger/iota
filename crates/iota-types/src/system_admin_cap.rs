// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {
    use serde::{Deserialize, Serialize};

    /// Rust version of the IotaSystemAdminCap type.
    #[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
    pub struct IotaSystemAdminCap {
        // This field is required to make a Rust struct compatible with an empty Move one.
        // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
        // allowed in the bytecode.
        dummy_field: bool,
    }
}
