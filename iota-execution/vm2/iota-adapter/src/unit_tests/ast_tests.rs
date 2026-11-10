// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::static_programmable_transactions::loading::ast::Type;

#[test]
fn enum_size() {
    assert_eq!(std::mem::size_of::<Type>(), 16);
}
