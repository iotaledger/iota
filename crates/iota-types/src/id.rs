// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};

use crate::IOTA_FRAMEWORK_ADDRESS;

pub const RESOLVED_IOTA_ID: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("object"),
    ident_str!("ID"),
);
