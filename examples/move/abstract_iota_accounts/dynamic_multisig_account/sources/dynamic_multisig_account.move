// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The DynamicMultisigAccount module defines a generic account struct that can be used to handle a dynamic multisig
/// account. The account data, such as the members information, the threshold and the proposed transactions, are
/// stored as dynamic fields of the account object. The module provides functions to create a new
/// DynamicMultisigAccount, update the account data and propose and approve transactions.
///
/// The module also defines an authenticator that checks that the sender of the transaction is the account and that
/// the total weight of the members who approved the transaction is greater than or equal to the threshold.
module dynamic_multisig_account::dynamic_multisig_account;

use dynamic_multisig_account::members::{Self, Members};
use dynamic_multisig_account::transactions::{Self, Transactions};
use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field as df;

// === Errors ===

#[error(code = 0)]
const ETotalMembersWeightLessThanThreshold: vector<u8> =
    b"The members weight is less than the threshold.";
#[error(code = 1)]
const EThresholdIsZero: vector<u8> = b"The threshold can not be equal to 0.";
#[error(code = 2)]
const ETransactionSenderIsNotTheAccount: vector<u8> =
    b"The user who signed the transaction is not the account.";
#[error(code = 3)]
const ETransactionDoesNotHaveSufficientApprovals: vector<u8> =
    b"The transaction does not have sufficient approvals.";

// === Structs ===

/// A dynamic field name for the account members.
public struct MembersFieldName has copy, drop, store {}

/// A dynamic field name for the threshold.
public struct ThresholdFieldName has copy, drop, store {}

/// A dynamic field name for the transactions.
public struct TransactionsFieldName has copy, drop, store {}

/// This struct represents a dynamic multisig account.
public struct DynamicMultisigAccount has key {
    id: UID,
}

// === DynamicMultisigAccount Handling ===

/// Creates a new `DynamicMultisigAccount` instance as a shared object with the given
/// members, threshold and authenticator.
public fun create(
    members_addresses: vector<address>,
    members_weights: vector<u64>,
    threshold: u64,
    authenticator: AuthenticatorFunctionRefV1<DynamicMultisigAccount>,
    ctx: &mut TxContext,
) {
    // Create a `Members` instance.
    let members = members::create(members_addresses, members_weights);

    // Verify the provided data consistency.
    verify_threshold(&members, threshold);

    // Create a UID for an account object.
    let mut id = object::new(ctx);

    // Add all the data as dynamic fields.
    df::add(&mut id, members_field_name(), members);
    df::add(&mut id, threshold_field_name(), threshold);
    df::add(&mut id, transactions_field_name(), transactions::create(ctx));

    let account = DynamicMultisigAccount { id };

    // Create a mutable shared account object.
    account::create_account_v1(account, authenticator);
}

/// Updates the account data: members information, threshold and authenticator.
/// Can be called only by the account itself, that means that this call must be approved by the account members.
/// The transactions that are proposed but not yet executed can have approves from members
/// who are not in the new members list. These approves will be ignored when checking if the transaction is approved.
public fun update_account_data(
    self: &mut DynamicMultisigAccount,
    members_addresses: vector<address>,
    members_weights: vector<u64>,
    threshold: u64,
    authenticator: AuthenticatorFunctionRefV1<DynamicMultisigAccount>,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Create a `Members` instance.
    let members = members::create(members_addresses, members_weights);

    // Verify the provided data consistency.
    verify_threshold(&members, threshold);

    let account_id = &mut self.id;

    // Update the dynamic fields. It is expected that the fields already exist.
    update_df(account_id, members_field_name(), members);
    update_df(account_id, threshold_field_name(), threshold);

    account::rotate_auth_function_ref_v1(self, authenticator);
}

/// Proposes a new transaction to be approved by the account members.
/// The member who proposes the transaction is added as the first approver.
public fun propose_transaction(
    self: &mut DynamicMultisigAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext,
) {
    // Get the member who proposed the transaction.
    let member_address = *self.members().borrow(ctx.sender()).addr();

    // Store the transaction.
    self.transactions_mut().add(transaction_digest, member_address);
}

/// Approves a proposed transaction.
public fun approve_transaction(
    self: &mut DynamicMultisigAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext,
) {
    // Get the member who approved the transaction.
    let member_address = *self.members().borrow(ctx.sender()).addr();

    // Get the transaction.
    let transaction = self.transactions_mut().borrow_mut(transaction_digest);

    // Approve the transaction.
    transaction.add_approval(member_address);
}

/// Removes a transaction.
/// It can be removed ether it was executed or not.
/// Can be removed only by the account itself, that means that this call must be approved by the account members.
public fun remove_transaction(
    self: &mut DynamicMultisigAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Remove the transaction.
    self.transactions_mut().remove(transaction_digest);
}

// === Authenticators ===

/// A DynamicMultisigAccount authenticator.
///
/// Checks that the sender of this transaction is the account.
/// The total weight of the members who approved the transaction must be greater than or equal to the threshold.
/// If the members list is changed after the transaction proposal, only the members who are still in the list
/// are considered for the approval. Their weights are taken from the current members list.
#[authenticator]
public fun approval_authenticator(self: &DynamicMultisigAccount, _: &AuthContext, ctx: &TxContext) {
    assert!(
        self.total_approves(*ctx.digest()) >= self.threshold(),
        ETransactionDoesNotHaveSufficientApprovals,
    );
}

// === View Functions ===

/// Returns the account address.
public fun get_address(self: &DynamicMultisigAccount): address {
    self.id.to_address()
}

/// Borrows the account threshold.
public fun threshold(self: &DynamicMultisigAccount): u64 {
    *df::borrow(&self.id, threshold_field_name())
}

/// Immutably borrows the account members.
public fun members(self: &DynamicMultisigAccount): &Members {
    df::borrow(&self.id, members_field_name())
}

/// Immutably borrows the account transactions.
public fun transactions(self: &DynamicMultisigAccount): &Transactions {
    df::borrow(&self.id, transactions_field_name())
}

/// Returns the total weight of the members who approved the transaction with the provided digest.
public fun total_approves(self: &DynamicMultisigAccount, transaction_digest: vector<u8>): u64 {
    // If the transaction does not exist, the total approves is zero.
    if (!self.transactions().contains(transaction_digest)) {
        return 0
    };

    let members = self.members();
    let transaction = self.transactions().borrow(transaction_digest);

    // Calculate the total weight of the members who approved the transaction.
    let mut total_approves = 0;
    transaction.approves().do_ref!(|addr| {
        if (members.contains(*addr)) {
            total_approves = total_approves + members.borrow(*addr).weight();
        }
    });
    total_approves
}

/// Immutably borrows the account authenticator.
public fun borrow_auth_function_ref_v1(
    self: &DynamicMultisigAccount,
): &AuthenticatorFunctionRefV1<DynamicMultisigAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

// === Private Functions ===

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &DynamicMultisigAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Returns the dynamic field name used to store the members information.
fun members_field_name(): MembersFieldName {
    MembersFieldName {}
}

/// Returns the dynamic field name used to store the threshold.
fun threshold_field_name(): ThresholdFieldName {
    ThresholdFieldName {}
}

/// Returns the dynamic field name used to store the transactions.
fun transactions_field_name(): TransactionsFieldName {
    TransactionsFieldName {}
}

/// Mutably borrows the account transactions.
fun transactions_mut(self: &mut DynamicMultisigAccount): &mut Transactions {
    df::borrow_mut(&mut self.id, transactions_field_name())
}

/// Verifies the threshold.
fun verify_threshold(members: &Members, threshold: u64) {
    // Check that the threshold is not zero.
    assert!(threshold != 0, EThresholdIsZero);
    // Check that the total members weight is greater than or equal to the threshold.
    assert!(members.total_weight() >= threshold, ETotalMembersWeightLessThanThreshold);
}

/// Updates a dynamic field value and returns the previous one.
/// It is supposed that the dynamic field with the given name already exists.
fun update_df<Name: copy + drop + store, Value: store>(
    account_id: &mut UID,
    name: Name,
    value: Value,
): Value {
    let previous_value = df::remove(account_id, name);
    df::add(account_id, name, value);
    previous_value
}
