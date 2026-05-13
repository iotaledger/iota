// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticator_inputs_cryptographic_protection::blacklist;

#[error(code = 0)]
const EAccountIsAlreadyBlacklisted: vector<u8> = b"Account is already blacklisted.";

/// A shared object that maintains a blacklist of accounts.
public struct Blacklist has key {
    id: UID,
    accounts: vector<address>,
}

/// Creates a new `Blacklist` shared object.
public fun create(ctx: &mut TxContext) {
    let blacklist = Blacklist { id: object::new(ctx), accounts: vector::empty() };
    transfer::share_object(blacklist);
}

/// Adds an account address to the blacklist.
public fun add(blacklist: &mut Blacklist, account: address) {
    assert!(!blacklist.accounts.contains(&account), EAccountIsAlreadyBlacklisted);
    blacklist.accounts.push_back(account);
}

/// Returns `true` if the given account address is blacklisted.
public fun is_blacklisted(blacklist: &Blacklist, account: address): bool {
    blacklist.accounts.contains(&account)
}
