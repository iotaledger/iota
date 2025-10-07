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
}

/// This struct represents an abstract account.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// It distinguishes between two classes of dynamic fields.
/// Reserved ones, used for managing the account's internal state, such as unlock times and public keys
/// and regular ones which can be used for general data storage.
///
/// The list of reserved fields is stored as a dynamic field under `ReservedDynamicFields`.
///
/// As regular data, dynamic fields may be added and removed as necessary, but reserved ones cannot.
/// Reserved fields are part of the authentication logic so they should not be removed only rotated.
///
/// An `AbstractAccount` cannot be constructed directly. To create an `AbstractAccount` use `AbstractAccountBuilder`.
public struct AbstractAccount has key {
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
public fun finish(self: AbstractAccountBuilder): AbstractAccount {
    let AbstractAccountBuilder { account } = self;
    account
}

/// Share AbstractAccount.
public fun share(self: AbstractAccount) {
    iota::transfer::share_object(self);
}

public fun uid(self: &AbstractAccount): &UID {
    &self.id
}

public fun uid_mut(self: &mut AbstractAccount, ctx: &TxContext): &mut UID {
    ensure_tx_sender_is_account(self, ctx);
    &mut self.id
}

// === Admin Functions ===

/// Check that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===
