// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dynamic_multisig_account::account;

use dynamic_multisig_account::members::{Self, Members};
use dynamic_multisig_account::transactions::{Self, Transactions};

use iota_account::iota_account::{Self, IOTAccount};

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;

// --------------------------------------- Errors ---------------------------------------

#[error(code = 0)]
const ETotalMembersWeightLessThenThreshold: vector<u8> = b"The members weight is less then the threshold.";
#[error(code = 1)]
const EThresholdIsZero: vector<u8> = b"The threshold can not be equal to 0.";
#[error(code = 2)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"The user who signed the transaction is not the account.";
#[error(code = 3)]
const ETransactionDoesNotHaveSufficientApprovals: vector<u8> = b"The transaction does not have sufficient approvals.";

// -------------------------------- Dynamic Field Names --------------------------------

/// A dynamic field key for the account members.
public struct MembersKey has copy, drop, store {}
/// A dynamic field key for the threshold.
public struct ThresholdKey has copy, drop, store {}
/// A dynamic field key for the transactions.
public struct TransactionsKey has copy, drop, store {}

// -------------------------------------- Creation --------------------------------------

/// Creates a new `DynamicMultisigAccount` instance as a shared object with the given
/// members, threshold and authenticator.
public fun create(
    members_addresses: vector<address>,
    members_weights: vector<u64>,
    threshold: u64,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext
) {
    // Create a `Members` instance.
    let members = members::create(members_addresses, members_weights);

    // Verify the provided data consistency.
    verify_threshold(&members, threshold);

    iota_account::builder(ctx)
        .add_reserved_dynamic_field(members_key(), members)
        .add_reserved_dynamic_field(threshold_key(), threshold)
        .add_reserved_dynamic_field(transactions_key(), transactions::create(ctx))
        .add_authenticator(authenticator)
        .share();
}

public fun clear(self: &mut IOTAccount, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    self.remove_field<_, Members>(members_key(), ctx);
    self.remove_field<_, u64>(threshold_key(), ctx);
    self.remove_field<_, Transactions>(transactions_key(), ctx).destroy();
}

// --------------------------------------- Transactions ---------------------------------------

/// Proposes a new transaction to be approved by the account members.
/// The member who proposes the transaction is added as the first approver.
public fun propose_transaction(
    self: &mut IOTAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext
) {
    // Get the member who proposed the transaction.
    let member_address = *members(self).member(ctx.sender()).addr();

    // Store the transaction.
    transactions_mut(self, ctx).add_transaction(transaction_digest, member_address);
}

/// Approves a proposed transaction.
public fun approve_transaction(self: &mut IOTAccount, transaction_digest: vector<u8>, ctx: &TxContext) {
    // Get the member who proposed the transaction.
    let member_address = *members(self).member(ctx.sender()).addr();

    // Get the transaction.
    let transaction = transactions_mut(self, ctx).transaction_mut(transaction_digest);

    // Approve the transaction.
    transaction.add_approval(member_address);
}

// --------------------------------------- Authentication ---------------------------------------

/// Updates the account data: members information, threshold and authenticator.
public fun update_account_data(
    self: &mut IOTAccount,
    members_addresses: vector<address>,
    members_weights: vector<u64>,
    threshold: u64,
    authenticator: AuthenticatorInfoV1,
    ctx: &TxContext
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Create a `Members` instance.
    let members = members::create(members_addresses, members_weights);

    // Verify the provided data consistency.
    verify_threshold(&members, threshold);

    // Update the dynamic fields. It is expected that the fields already exist.
    self.rotate_reserved_field(members_key(), members, ctx);
    self.rotate_reserved_field(threshold_key(), threshold, ctx);
    self.rotate_reserved_field(account::authenticator_df_name(), authenticator, ctx);
}

/// A transaction authenticator.
/// Checks that the sender of this transaction is the account and that the transaction is approved.
public fun authenticate(self: &IOTAccount, _: &AuthContext, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check that the transaction is approved.
    ensure_tx_is_approved(self, ctx);
}

// --------------------------------------- Utilities ---------------------------------------

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.addr() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Checks that the transaction is approved.
/// The total weight of the members who approved the transaction must be greater than or equal to the threshold.
/// If the members list is changed after the transaction proposal, only the members who are still in the list
/// are considered for the approval. Their weights are taken from the current members list.
fun ensure_tx_is_approved(self: &IOTAccount, ctx: &TxContext) {
    let members = members(self);
    let transaction = transactions(self).transaction(*ctx.digest());
    let threshold = threshold(self);

    let mut total_approves = 0;

    transaction.approves().do_ref!(|addr| {
        if (members.has_member(*addr)) {
            total_approves = total_approves + members.member(*addr).weight();
        }
    });

    assert!(total_approves >= threshold, ETransactionDoesNotHaveSufficientApprovals);
}

/// Returns the dynamic field name used to store the members information.
fun members_key(): MembersKey {
    MembersKey{}
}

/// Returns the dynamic field name used to store the threshold.
fun threshold_key(): ThresholdKey {
    ThresholdKey{}
}

/// Returns the dynamic field name used to store the transactions.
fun transactions_key(): TransactionsKey {
    TransactionsKey{}
}

/// Immutably borrows the account members.
fun members(self: &IOTAccount): &Members {
    self.borrow_field(members_key())
}

/// Immutably borrows the account transactions.
fun transactions(self: &IOTAccount): &Transactions {
    self.borrow_field(transactions_key())
}

/// Mutably borrows the account transactions.
fun transactions_mut(self: &mut IOTAccount, ctx: &TxContext): &mut Transactions {
    self.borrow_field_mut(transactions_key(), ctx)
}

/// Borrows the account threshold.
fun threshold(self: &IOTAccount): u64 {
    *self.borrow_field(threshold_key())
}

/// Verifies the threshold.
fun verify_threshold(members: &Members, threshold: u64 ) {
    // Check that the threshold is not zero.
    assert!(threshold != 0, EThresholdIsZero);
    // Check that the total members weight is greater than or equal to the threshold.  
    assert!(members.total_weight() >= threshold, ETotalMembersWeightLessThenThreshold);
}
