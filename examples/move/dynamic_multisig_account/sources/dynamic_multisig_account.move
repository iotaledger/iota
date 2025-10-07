// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dynamic_multisig_account::dynamic_multisig_account;

use dynamic_multisig_account::members::{Self, Members};
use dynamic_multisig_account::transactions::{Self, Transactions};
use iota::account::AuthenticatorInfoV1;
use iota::auth_context::AuthContext;
use iotaccount::iotaccount::{Self, IOTAccount};

// --------------------------------------- Errors ---------------------------------------

#[error(code = 0)]
const ETotalMembersWeightLessThanThreshold: vector<u8> =
    b"The members weight is less than the threshold.";
#[error(code = 1)]
const EThresholdIsZero: vector<u8> = b"The threshold can not be equal to 0.";
#[error(code = 2)]
const ETransactionDoesNotHaveSufficientApprovals: vector<u8> =
    b"The transaction does not have sufficient approvals.";

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
    ctx: &mut TxContext,
) {
    // Create a `Members` instance.
    let members = members::create(members_addresses, members_weights);

    // Verify the provided data consistency.
    verify_threshold(&members, threshold);

    // Create an account object.
    let account = iotaccount::builder(authenticator, ctx)
        .add_dynamic_field(members_key(), members)
        .add_dynamic_field(threshold_key(), threshold)
        .add_dynamic_field(transactions_key(), transactions::create(ctx))
        .finish();

    // Share the account object.
    account.share();
}

// --------------------------------------- View Functions ---------------------------------------

/// Borrows the account threshold.
public fun threshold(self: &IOTAccount): u64 {
    *self.borrow_field(threshold_key())
}

/// Immutably borrows the account members.
public fun members(self: &IOTAccount): &Members {
    self.borrow_field(members_key())
}

/// Immutably borrows the account transactions.
public fun transactions(self: &IOTAccount): &Transactions {
    self.borrow_field(transactions_key())
}

/// Returns the total weight of the members who approved the transaction with the provided digest.
public fun total_approves(self: &IOTAccount, transaction_digest: vector<u8>): u64 {
    // If the transaction does not exist, the total approves is zero.
    if (!transactions(self).contains(transaction_digest)) {
        return 0
    };

    let members = members(self);
    let transaction = transactions(self).borrow(transaction_digest);

    // Calculate the total weight of the members who approved the transaction.
    let mut total_approves = 0;
    transaction.approves().do_ref!(|addr| {
        if (members.contains(*addr)) {
            total_approves = total_approves + members.borrow(*addr).weight();
        }
    });
    total_approves
}

// --------------------------------------- Transactions ---------------------------------------

/// Proposes a new transaction to be approved by the account members.
/// The member who proposes the transaction is added as the first approver.
public fun propose_transaction(
    self: &mut IOTAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext,
) {
    // Get the member who proposed the transaction.
    let member_address = *members(self).borrow(ctx.sender()).addr();

    // Store the transaction.
    transactions_mut(self, ctx).add(transaction_digest, member_address);
}

/// Approves a proposed transaction.
public fun approve_transaction(
    self: &mut IOTAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext,
) {
    // Get the member who approved the transaction.
    let member_address = *members(self).borrow(ctx.sender()).addr();

    // Get the transaction.
    let transaction = transactions_mut(self, ctx).borrow_mut(transaction_digest);

    // Approve the transaction.
    transaction.add_approval(member_address);
}

/// Removes a transaction.
/// It can be removed ether it was executed or not.
/// Can be removed only by the account itself, that means that this call must be approved by the account members.
public fun remove_transaction(
    self: &mut IOTAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    self.ensure_tx_sender_is_account(ctx);

    // Remove the transaction.
    transactions_mut(self, ctx).remove(transaction_digest);
}

// --------------------------------------- Authentication ---------------------------------------

/// Updates the account data: members information, threshold and authenticator.
/// Can be called only by the account itself, that means that this call must be approved by the account members.
/// The transactions that are proposed but not yet executed can have approves from members
/// who are not in the new members list. These approves will be ignored when checking if the transaction is approved.
public fun update_account_data(
    self: &mut IOTAccount,
    members_addresses: vector<address>,
    members_weights: vector<u64>,
    threshold: u64,
    authenticator: AuthenticatorInfoV1,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    self.ensure_tx_sender_is_account(ctx);

    // Create a `Members` instance.
    let members = members::create(members_addresses, members_weights);

    // Verify the provided data consistency.
    verify_threshold(&members, threshold);

    // Update the dynamic fields. It is expected that the fields already exist.
    self.rotate_field(members_key(), members, ctx);
    self.rotate_field(threshold_key(), threshold, ctx);
    self.rotate_auth_info_v1(authenticator, ctx);
}

/// A transaction authenticator.
///
/// Checks that the sender of this transaction is the account.
/// The total weight of the members who approved the transaction must be greater than or equal to the threshold.
/// If the members list is changed after the transaction proposal, only the members who are still in the list
/// are considered for the approval. Their weights are taken from the current members list.
public fun authenticate(self: &IOTAccount, _: &AuthContext, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    self.ensure_tx_sender_is_account(ctx);

    // Check that the transaction is approved.
    assert!(
        total_approves(self, *ctx.digest()) >= threshold(self),
        ETransactionDoesNotHaveSufficientApprovals,
    );
}

// --------------------------------------- Utilities ---------------------------------------

/// Returns the dynamic field name used to store the members information.
fun members_key(): MembersKey {
    MembersKey {}
}

/// Returns the dynamic field name used to store the threshold.
fun threshold_key(): ThresholdKey {
    ThresholdKey {}
}

/// Returns the dynamic field name used to store the transactions.
fun transactions_key(): TransactionsKey {
    TransactionsKey {}
}

/// Mutably borrows the account transactions.
fun transactions_mut(self: &mut IOTAccount, ctx: &TxContext): &mut Transactions {
    self.borrow_field_mut(transactions_key(), ctx)
}

/// Verifies the threshold.
fun verify_threshold(members: &Members, threshold: u64) {
    // Check that the threshold is not zero.
    assert!(threshold != 0, EThresholdIsZero);
    // Check that the total members weight is greater than or equal to the threshold.
    assert!(members.total_weight() >= threshold, ETotalMembersWeightLessThanThreshold);
}
