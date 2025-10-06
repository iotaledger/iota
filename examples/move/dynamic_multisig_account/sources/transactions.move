// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dynamic_multisig_account::transactions;

use iota::table::{Self, Table};

// --------------------------------------- Errors ---------------------------------------

#[error(code = 0)]
const ETransactionIsAlreadyApprovedByTheMember: vector<u8> =
    b"The transaction is already approved by the member.";
#[error(code = 1)]
const ETransactionAlreadyExists: vector<u8> =
    b"A transaction with the provided digest already exists.";
#[error(code = 2)]
const ETransactionDoesNotExist: vector<u8> =
    b"A transaction with the provided digest does not exist.";

// ----------------------------------- Data Structures -----------------------------------

/// Holds the information about a transaction.
public struct Transaction has store {
    /// The transaction digest.
    digest: vector<u8>,
    /// The members who approved the transaction.
    approves: vector<address>,
}

/// Holds the information about the account transactions.
public struct Transactions has store {
    /// The transactions collection.
    table: Table<vector<u8>, Transaction>,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a `Transactions` instance.
public(package) fun create(ctx: &mut TxContext): Transactions {
    Transactions { table: table::new(ctx) }
}

// ------------------------------------- Transactions -------------------------------------

/// Checks if the account has a transaction with the provided digest.
public fun contains(self: &Transactions, digest: vector<u8>): bool {
    self.table.contains(digest)
}

/// Immutably borrows the account transaction with the provided digest.
public fun borrow(self: &Transactions, digest: vector<u8>): &Transaction {
    self.table.borrow(digest)
}

/// Mutably borrows the account transaction with the provided digest.
public(package) fun borrow_mut(self: &mut Transactions, digest: vector<u8>): &mut Transaction {
    self.table.borrow_mut(digest)
}

/// Adds a new transaction to the account.
public(package) fun add(self: &mut Transactions, digest: vector<u8>, member: address) {
    // Ensure that the transaction does not already exist.
    assert!(!self.table.contains(digest), ETransactionAlreadyExists);

    // Add the transaction.
    self.table.add(digest, Transaction { digest, approves: vector[member] });
}

/// Removes a transaction from the account.
/// Returns the digest and the addresses of the members who approved the transaction.
public(package) fun remove(
    self: &mut Transactions,
    digest: vector<u8>,
): (vector<u8>, vector<address>) {
    // Ensure that the transaction exists.
    assert!(self.table.contains(digest), ETransactionDoesNotExist);

    // Remove the transaction and unpack it.
    unpack(self.table.remove(digest))
}

// ------------------------------------- Transaction -------------------------------------

/// Returns the digest of the transaction.
public fun digest(self: &Transaction): vector<u8> {
    self.digest
}

/// Returns the addresses of the members who approved the transaction.
public fun approves(self: &Transaction): &vector<address> {
    &self.approves
}

/// Adds the approval of the member to the transaction.
public(package) fun add_approval(self: &mut Transaction, member: address) {
    assert!(!self.approves.contains(&member), ETransactionIsAlreadyApprovedByTheMember);

    self.approves.push_back(member);
}

/// Unpacks the transaction into its components and deletes it.
fun unpack(self: Transaction): (vector<u8>, vector<address>) {
    let Transaction { digest, approves } = self;

    (digest, approves)
}
