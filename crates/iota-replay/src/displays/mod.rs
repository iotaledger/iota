// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod gas_status_displays;
pub mod transaction_displays;

pub struct Pretty<'a, T>(pub &'a T);
