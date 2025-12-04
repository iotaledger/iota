// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module aa::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {
    let mut account = AbstractAccount { id: object::new(ctx) };
    let authenticator_compatibility_proof = account::check_auth_info_v1_compatibility(
        &account,
        authenticator,
    );
    account::attach_auth_info_v1(account.uid_mut(), authenticator_compatibility_proof);
    let account_address = object::id_address(&account);
    iota::transfer::share_object(account);
    account_address
}

public fun uid_mut(self: &mut AbstractAccount): &mut UID {
    &mut self.id
}
