// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::account;

// Test account struct
public struct Account has key {
    id: UID,
}

public fun destroy(account: Account) {
    let Account { id } = account;
    object::delete(id);
}
