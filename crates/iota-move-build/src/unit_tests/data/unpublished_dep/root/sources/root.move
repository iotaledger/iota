// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module root::root;

use dep::dep::Account;

public fun account_id(account: &Account): ID {
    object::id(account)
}
