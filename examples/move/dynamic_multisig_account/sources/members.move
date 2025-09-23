// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module dynamic_multisig_account::members;

// --------------------------------------- Errors ---------------------------------------

#[error(code = 0)]
const EMembersComponentsHaveDifferentLengths: vector<u8> = b"The members components have different lengths.";
#[error(code = 1)]
const EMemberIsNotFound: vector<u8> = b"The member with the provided address is not found.";

// ----------------------------------- Data Structures -----------------------------------

/// Holds the information about a member.
public struct Member has drop, store {
    /// The member address.
    addr: address,
    /// The voting power of the member.
    weight: u64,
}

/// Holds the information about the account members.
public struct Members has drop, store {
    /// The members collection.
    list: vector<Member>,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a `Members` struct from the given vectors of addresses and weights.
/// The vectors must have the same length.
public(package) fun create(addresses: vector<address>, weights: vector<u64>): Members {
    // Check that the lengths of the provided vectors are equal.
    assert!(addresses.length() == weights.length(), EMembersComponentsHaveDifferentLengths);

    // Create a `Members` instance.
    let list = addresses.zip_map!(weights, |addr, weight| Member { addr, weight });

    Members{ list }
}

// --------------------------------------- Members ---------------------------------------

/// Checks if the account has a member with the provided address.
public(package) fun has_member(self: &Members, addr: address): bool {
    find_index(self, addr).is_some()
}

/// Immutably borrows the account member with the provided address.
public(package) fun borrow_member(self: &Members, addr: address): &Member {
    let index = find_index(self, addr);

    assert!(index.is_some(), EMemberIsNotFound);

    self.list.borrow(*index.borrow())
}

/// Mutably borrows the account member with the provided address.
public(package) fun borrow_member_mut(self: &mut Members, addr: address): &mut Member {
    let index = find_index(self, addr);

    assert!(index.is_some(), EMemberIsNotFound);

    self.list.borrow_mut(*index.borrow())
}

/// Returns the total weight of all the members.
public(package) fun total_weight(self: &Members): u64 {
    let mut total = 0;
    self.list.do_ref!(|m| total = total + m.weight);
    total
}

// --------------------------------------- Member ---------------------------------------

/// Borrows the address of the member.
public(package) fun borrow_address(self: &Member): &address {
    &self.addr
}

/// Returns the weight of the member.
public(package) fun weight(self: &Member): u64 {
    self.weight
}

// --------------------------------------- Utilities ---------------------------------------

/// Finds the index of the member with the provided address.
fun find_index(self: &Members, addr: address): Option<u64> {
    self.list.find_index!(|m| m.addr == addr)
}
