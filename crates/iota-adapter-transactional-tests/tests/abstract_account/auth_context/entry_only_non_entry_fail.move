// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Failure: non-entry function (policy check 1: is_entry).
// `non_entry_func` is a plain `public` function. The enriched command records
// `is_entry = false`; the authenticator aborts with code 1 (E_NON_ENTRY).

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::entry_only;

use simple_abstract_account::abstract_account::AbstractAccount;

/// Regular public function - not entry. `is_entry` will be `false`.
public fun non_entry_func() {}

const E_NON_ENTRY: u64 = 1;

#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let cmds = auth_ctx.enriched_tx_commands();
    let mut i = 0;
    while (i < cmds.length()) {
        let command = &cmds[i];
        if (command.is_move_call()) {
            let call = command.as_move_call().extract();
            assert!(call.is_entry(), E_NON_ENTRY);
        };
        i = i + 1;
    }
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "entry_only" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

// `non_entry_func` has `is_entry = false` → authenticator aborts with code 1.
//# abstract --account immshared(4,2)
//> 0: test::entry_only::non_entry_func();
