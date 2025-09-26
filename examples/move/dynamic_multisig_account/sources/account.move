// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dynamic_multisig_account::account;

use dynamic_multisig_account::members::{Self, Members};
use dynamic_multisig_account::transactions::{Self, Transactions};

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;

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

// ---------------------------------- Data Structures ----------------------------------

/// This struct represents a dynamic multisig account.
public struct DynamicMultisigAccount has key {
    id: UID,
}

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

    // Create a UID for an account object.
    let mut id = object::new(ctx);

    // Add all the data as dynamic fields.
    dynamic_field::add(&mut id, members_key(), members);
    dynamic_field::add(&mut id, threshold_key(), threshold);
    dynamic_field::add(&mut id, account::authenticator_df_name(), authenticator);
    dynamic_field::add(&mut id, transactions_key(), transactions::create(ctx));

    // Create a mutable shared account object.
    iota::transfer::share_object(DynamicMultisigAccount { id });
}

// --------------------------------------- Transactions ---------------------------------------

/// Proposes a new transaction to be approved by the account members.
/// The member who proposes the transaction is added as the first approver.
public fun propose_transaction(
    self: &mut DynamicMultisigAccount,
    transaction_digest: vector<u8>,
    ctx: &TxContext
) {
    // Get the member who proposed the transaction.
    let member_address = *self.members().member(ctx.sender()).addr();

    // Store the transaction.
    self.transactions_mut().add_transaction(transaction_digest, member_address);
}

/// Approves a proposed transaction.
public fun approve_transaction(self: &mut DynamicMultisigAccount, transaction_digest: vector<u8>, ctx: &TxContext) {
    // Get the member who proposed the transaction.
    let member_address = *self.members().member(ctx.sender()).addr();

    // Get the transaction.
    let transaction = self.transactions_mut().transaction_mut(transaction_digest);

    // Approve the transaction.
    transaction.add_approval(member_address);
}

/// Removes a transaction.
/// It can be removed ether it was executed or not.
/// Can be removed only by the account itself, that means that this call must be approved by the account members.
public fun remove_transaction(self: &mut DynamicMultisigAccount, transaction_digest: vector<u8>, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Get the transaction.
    self.transactions_mut().remove_transaction(transaction_digest);
}

// --------------------------------------- Authentication ---------------------------------------

/// Updates the account data: members information, threshold and authenticator.
/// Can be called only by the account itself, that means that this call must be approved by the account members.
public fun update_account_data(
    self: &mut DynamicMultisigAccount,
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

    let account_id = &mut self.id;

    // Update the dynamic fields. It is expected that the fields already exist.
    update_dynamic_field(account_id, members_key(), members);
    update_dynamic_field(account_id, threshold_key(), threshold);
    update_dynamic_field(account_id, account::authenticator_df_name(), authenticator);
}

/// A transaction authenticator.
/// Checks that the sender of this transaction is the account and that the transaction is approved.
public fun authenticate(self: &DynamicMultisigAccount, _: &AuthContext, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check that the transaction is approved.
    ensure_tx_is_approved(self, ctx);
}

// --------------------------------------- Utilities ---------------------------------------

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &DynamicMultisigAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Checks that the transaction is approved.
/// The total weight of the members who approved the transaction must be greater than or equal to the threshold.
/// If the members list is changed after the transaction proposal, only the members who are still in the list
/// are considered for the approval. Their weights are taken from the current members list.
fun ensure_tx_is_approved(self: &DynamicMultisigAccount, ctx: &TxContext) {
    let members = self.members();
    let transaction = self.transactions().transaction(*ctx.digest());
    let threshold = self.threshold();

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
fun members(self: &DynamicMultisigAccount): &Members {
    dynamic_field::borrow(&self.id, members_key())
}

/// Immutably borrows the account transactions.
fun transactions(self: &DynamicMultisigAccount): &Transactions {
    dynamic_field::borrow(&self.id, transactions_key())
}

/// Mutably borrows the account transactions.
fun transactions_mut(self: &mut DynamicMultisigAccount): &mut Transactions {
    dynamic_field::borrow_mut(&mut self.id, transactions_key())
}

/// Borrows the account threshold.
fun threshold(self: &DynamicMultisigAccount): u64 {
    *dynamic_field::borrow(&self.id, threshold_key())
}

/// Verifies the threshold.
fun verify_threshold(members: &Members, threshold: u64 ) {
    // Check that the threshold is not zero.
    assert!(threshold != 0, EThresholdIsZero);
    // Check that the total members weight is greater than or equal to the threshold.  
    assert!(members.total_weight() >= threshold, ETotalMembersWeightLessThenThreshold);
}

/// Updates a dynamic field value and returns the previous one.
/// It is supposed that the dynamic field with the given name already exists.
fun update_dynamic_field<Name: copy + drop + store, Value: store>(
    account_id: &mut UID,
    name: Name,
    value: Value
): Value {
    let previous_value = dynamic_field::remove(account_id, name);
    dynamic_field::add(account_id, name, value);
    previous_value
}
