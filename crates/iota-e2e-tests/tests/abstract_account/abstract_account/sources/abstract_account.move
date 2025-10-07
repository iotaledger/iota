// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::dynamic_field;

// === Imports ===

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

// === Constants ===

// === Structs ===

/// Safely construct an AbstractAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
///
/// Account implementations are expected to call the builder in a single function call,
/// add the desired authenticator info and dynamic fields.
public struct AbstractAccountBuilder {
    account: AbstractAccount,
    cap: AbstractCap,
}

/// This struct represents an abstract account. It holds all the related data as dynamic fields
/// to simplify updates, migrations and extensions.
///
/// An `AbstractAccount` cannot be constructed directly. To create an `AbstractAccount` use `AbstractAccountBuilder`.
public struct AbstractAccount has key {
    id: UID,
}

/// This struct represents an admin capability that can be used to prove ownership of an account.
public struct AbstractCap has key, store {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Construct an AbstractAccountBuilder and set the Authenticator.
///
/// The `AuthenticatorInfo` will be attached as a dynamic field under key provided by:
/// `account::authenticator_df_name()`.
public fun builder(
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
): AbstractAccountBuilder {
    // Builder should be mutable, but that triggers a compiler warning and it works
    // without for some reason, so it has been removed.
    let builder = AbstractAccountBuilder {
        account: AbstractAccount { id: object::new(ctx) },
        cap: AbstractCap { id: object::new(ctx) },
    };
    builder.add_dynamic_field(account::authenticator_df_name(), authenticator)
}

/// Attach a `Value` as a regular dynamic field to the builder.
public fun add_dynamic_field<Name: copy + drop + store, Value: store>(
    mut self: AbstractAccountBuilder,
    name: Name,
    value: Value,
): AbstractAccountBuilder {
    dynamic_field::add(&mut self.account.id, name, value);

    self
}

/// Finish building the `AbstractAccount` and share the object.
public fun finish(self: AbstractAccountBuilder): (AbstractAccount, AbstractCap) {
    let AbstractAccountBuilder { account, cap } = self;
    (account, cap)
}

/// Share AbstractAccount.
public fun share(self: AbstractAccount) {
    iota::transfer::share_object(self);
}

public fun uid(self: &AbstractAccount): &UID {
    &self.id
}

// === Admin Functions ===

public fun create_abstract_cap(self: &mut AbstractAccount, ctx: &mut TxContext): AbstractCap {
    ensure_tx_sender_is_account(self, ctx);
    AbstractCap { id: object::new(ctx) }
}

public fun uid_mut(self: &mut AbstractAccount, _: &AbstractCap): &mut UID {
    &mut self.id
}

/// Check that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===
