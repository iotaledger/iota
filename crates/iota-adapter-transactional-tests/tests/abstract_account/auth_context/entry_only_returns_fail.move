// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Failure: function with non-empty returns (policy check 2: void returns).
// `entry_returns_u64` is an entry function but returns `u64`. The enriched
// command records `returns = [TypeName("u64")]`; the authenticator aborts
// with code 2 (E_MUST_BE_VOID).

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::entry_only;

use simple_abstract_account::abstract_account::AbstractAccount;

/// Entry function that returns a value. `returns` will be `["u64"]`.
public entry fun entry_returns_u64(_ctx: &mut TxContext): u64 { 42 }

const E_NON_ENTRY: u64 = 1;
const E_MUST_BE_VOID: u64 = 2;

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
            assert!(call.returns().is_empty(), E_MUST_BE_VOID);
        };
        i = i + 1;
    }
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "entry_only" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

// `entry_returns_u64` is entry (check 1 passes) but returns u64 → `returns`
// is non-empty → authenticator aborts with code 2 (E_MUST_BE_VOID).
//# abstract --account immshared(4,2)
//> 0: test::entry_only::entry_returns_u64();
