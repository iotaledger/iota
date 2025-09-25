// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dynamic_multisig_account::transactions;

use iota::bag::{Self, Bag};

// --------------------------------------- Errors ---------------------------------------

#[error(code = 0)]
const ETransactionIsAlreadyApprovedByTheMember: vector<u8> = b"The transaction is already approved by the member.";

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
    /// The members collection.
    bag: Bag,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a `Transactions` struct.
public(package) fun create(ctx: &mut TxContext): Transactions {
    Transactions{ bag: bag::new(ctx) }
}

public(package) fun destroy(self: Transactions) {
    let Transactions { bag } = self;

    bag.destroy_empty();
}

// ------------------------------------- Transactions -------------------------------------

/// Checks if the account has a transaction with the provided digest.
public(package) fun has_transaction(self: &Transactions, digest: vector<u8>): bool {
    self.bag.contains(digest)
}

/// Immutably borrows the account transaction with the provided digest.
public(package) fun transaction(self: &Transactions, digest: vector<u8>): &Transaction {
    self.bag.borrow(digest)
}

/// Mutably borrows the account transaction with the provided digest.
public(package) fun transaction_mut(self: &mut Transactions, digest: vector<u8>): &mut Transaction {
    self.bag.borrow_mut(digest)
}

/// Adds a new transaction to the account.
public(package) fun add_transaction(self: &mut Transactions, digest: vector<u8>, member: address) {
    self.bag.add(digest, Transaction{digest, approves: vector[ member ]});
}

// ------------------------------------- Transaction -------------------------------------

/// Returns the digest of the transaction.
public(package) fun digest(self: &Transaction): vector<u8> {
    self.digest
}

/// Returns the addresses of the members who approved the transaction.
public(package) fun approves(self: &Transaction): &vector<address> {
    &self.approves
}

/// Adds the approval of the member to the transaction.
public(package) fun add_approval(self: &mut Transaction, member: address) {
    assert!(
        !self.approves.contains(&member),
        ETransactionIsAlreadyApprovedByTheMember
    );

    self.approves.push_back(member);
}
