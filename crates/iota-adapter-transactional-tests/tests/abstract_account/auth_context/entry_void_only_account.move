// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Transactional-test adaptation of the `entry_only_account` example.
// Enforced policy (three enriched AuthContext fields):
//   1. is_entry  - every MoveCall must target an `entry` function
//   2. returns   - every MoveCall must be void (no return values)
//   3. mutable   - no ImmOrOwned object input may be passed as `&mut T`
// This file covers the SUCCESS path. Failure cases are in the
// `entry_only_*_fail.move` files alongside this one.

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::entry_only;

use simple_abstract_account::abstract_account::AbstractAccount;

// -- Helper functions called from PTBs ---------------------------------------

/// Entry, void - satisfies policy checks 1 and 2.
public entry fun allowed(_ctx: &mut TxContext) {}

/// Entry, void, immutable ref - satisfies all three checks when the object
/// is passed immutably.
public entry fun allowed_with_ref(_account: &AbstractAccount, _ctx: &mut TxContext) {}

/// Non-entry, void - used in `entry_only_non_entry_fail.move`.
public fun non_entry_func() {}

/// Entry, returns u64 - used in `entry_only_returns_fail.move`.
public entry fun entry_returns_u64(_ctx: &mut TxContext): u64 { 42 }

/// Entry, `&mut AbstractAccount` - used in `entry_only_mutable_fail.move`.
public entry fun entry_takes_mut(_: &mut AbstractAccount, _ctx: &mut TxContext) {}

// -- Authenticator ------------------------------------------------------------

const E_NON_ENTRY: u64 = 1;
const E_MUST_BE_VOID: u64 = 2;
const E_NO_MUTABLE_INPUTS: u64 = 3;

/// Enforces the entry-only, void, no-mutable policy via three enriched fields:
/// `is_entry`, `returns`, and `ImmOrOwnedObjectArg::mutable`.
#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    // Check 1 & 2: every MoveCall must be `entry` and void.
    let cmds = auth_ctx.enriched_tx_commands();
    let mut i = 0;
    while (i < cmds.length()) {
        let command = &cmds[i];
        if (command.is_move_call()) {
            let call = command.as_move_call().destroy_some();
            // `is_entry` is true only when the called function is declared
            // with the `entry` modifier.
            assert!(call.is_entry(), E_NON_ENTRY);
            // `returns` is the list of resolved return-type names.
            // Empty means the function is void.
            assert!(call.returns().is_empty(), E_MUST_BE_VOID);
        };
        i = i + 1;
    };

    // Check 3: no object input (ImmOrOwned or Shared) may be mutable.
    // `is_mutable_object` covers both variants so that shared objects
    // passed as `&mut T` are also rejected.
    let inputs = auth_ctx.enriched_tx_inputs();
    let mut j = 0;
    while (j < inputs.length()) {
        let input = &inputs[j];
        let mutable_opt = input.is_mutable_object();
        if (mutable_opt.is_some()) {
            assert!(!mutable_opt.destroy_some(), E_NO_MUTABLE_INPUTS);
        };
        j = j + 1;
    }
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "entry_only" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

// -- Scenario 1: entry+void, only pure inputs ---------------------------------
//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: test::entry_only::allowed();
//> 1: SplitCoins(Gas, [Input(0)]);
//> 2: TransferObjects([Result(1)], Input(1));

// -- Scenario 2: entry+void, immutable shared object ref ----------------------
// Use immshared() so the SharedObjectArg has mutable:false; the policy check
// must allow immutable shared references.
//# abstract --account immshared(4,2) --ptb-inputs immshared(4,2) 100 @A
//> 0: test::entry_only::allowed_with_ref(Input(0));
//> 1: SplitCoins(Gas, [Input(1)]);
//> 2: TransferObjects([Result(1)], Input(2));
