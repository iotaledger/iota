// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Failure: mutable object input (policy check 3: no mutable objects).
// `entry_takes_mut` is an entry, void function that accepts `&mut
// AbstractAccount`. When the account is passed as Input(0) the engine records
// `mutable: true` on the enriched input. The authenticator aborts with code 3
// (E_NO_MUTABLE_INPUTS).

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::entry_only;

use simple_abstract_account::abstract_account::AbstractAccount;

/// Entry, void, mutable ref. The enriched input for the object passed here
/// will have `mutable: true`.
public entry fun entry_takes_mut(_: &mut AbstractAccount, _ctx: &mut TxContext) {}

const E_NON_ENTRY: u64 = 1;
const E_MUST_BE_VOID: u64 = 2;
const E_NO_MUTABLE_INPUTS: u64 = 3;

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
            let call = command.as_move_call().destroy_some();

            // Field 1: `is_entry` — true when the called function
            // is declared `entry` in its module.
            assert!(call.is_entry(), E_NON_ENTRY);

            // Field 2: `returns` — list of canonical return types.
            // An empty vector means the function is void.
            assert!(call.returns().is_empty(), E_MUST_BE_VOID);
        };
        i = i + 1;
    };

    // Check 3: no object input (ImmOrOwned or Shared) may be mutable.
    // Abstract accounts are shared objects, so we must check SharedObject
    // mutability here in addition to ImmOrOwned.
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

// `entry_takes_mut` is entry+void (checks 1 & 2 pass), but Input(0) is
// passed as `&mut AbstractAccount` → `mutable: true` → check 3 aborts
// with code 3 (E_NO_MUTABLE_INPUTS).
//# abstract --account immshared(4,2) --ptb-inputs object(4,2)
//> 0: test::entry_only::entry_takes_mut(Input(0));
