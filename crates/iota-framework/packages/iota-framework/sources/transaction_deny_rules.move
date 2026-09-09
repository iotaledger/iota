// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module holds the consensus-governed transaction deny rules: the
/// stake-weighted aggregate of the validators' deny rule proposals, mirrored
/// on-chain by `TransactionDenyRulesUpdate` system transactions carrying
/// add/remove deltas.
module iota::transaction_deny_rules;

use iota::event;
use iota::linked_table::{Self, LinkedTable};
use iota::versioned::{Self, Versioned};

/// Sender is not @0x0 the system address.
const ENotSystemAddress: u64 = 0;
const EWrongInnerVersion: u64 = 1;

const CURRENT_VERSION: u64 = 1;

/// Singleton shared object which stores the active transaction deny rules.
/// The actual state is stored in a versioned inner field.
public struct TransactionDenyRules has key {
    id: UID,
    inner: Versioned,
}

/// The deny lists are `LinkedTable` membership sets (the `bool` value is
/// always `true` and never read): each entry is its own child object, so
/// capacity is not bounded by the size of a single Move object, and updates
/// add and remove entries without touching the rest. The linked keys let any
/// reader enumerate the full set by walking `front` → `next` with plain
/// child-object reads, without a dynamic-field index — a plain `Table` cannot
/// be read back whole by a party that does not already know the keys.
public struct TransactionDenyRulesInnerV1 has store {
    version: u64,
    /// Addresses denied as transaction sender or gas sponsor.
    denied_addresses: LinkedTable<address, bool>,
    /// Objects denied as transaction inputs or receiving objects.
    denied_objects: LinkedTable<ID, bool>,
    /// Packages denied as a (transitive) dependency of any command.
    denied_packages: LinkedTable<ID, bool>,
    /// Denies all package publishing.
    package_publish_disabled: bool,
    /// Denies all package upgrades.
    package_upgrade_disabled: bool,
    /// Denies transactions that use shared objects as inputs.
    shared_object_disabled: bool,
    /// Denies all user transactions (kill switch).
    user_transaction_disabled: bool,
    /// Denies transactions that contain receiving objects.
    receiving_objects_disabled: bool,
    /// Denies transactions signed with a Move authenticator.
    move_authenticator_disabled: bool,
}

/// Emitted on every update; the event stream is the audit history of the
/// network's deny rules. Carries the delta the update transaction requested
/// (tolerated no-ops included), the resulting switch states, and the
/// post-update size of each deny list.
public struct TransactionDenyRulesUpdated has copy, drop {
    /// The epoch in which the update was executed.
    epoch: u64,
    /// Addresses added to / removed from the sender-or-sponsor deny list.
    added_addresses: vector<address>,
    removed_addresses: vector<address>,
    /// Objects added to / removed from the input-or-receiving deny list.
    added_objects: vector<ID>,
    removed_objects: vector<ID>,
    /// Packages added to / removed from the dependency deny list.
    added_packages: vector<ID>,
    removed_packages: vector<ID>,
    /// Denies all package publishing.
    package_publish_disabled: bool,
    /// Denies all package upgrades.
    package_upgrade_disabled: bool,
    /// Denies transactions that use shared objects as inputs.
    shared_object_disabled: bool,
    /// Denies all user transactions (kill switch).
    user_transaction_disabled: bool,
    /// Denies transactions that contain receiving objects.
    receiving_objects_disabled: bool,
    /// Denies transactions signed with a Move authenticator.
    move_authenticator_disabled: bool,
    /// Deny list sizes after applying the delta.
    denied_addresses_len: u64,
    denied_objects_len: u64,
    denied_packages_len: u64,
}

#[allow(unused_function)]
/// Create and share the TransactionDenyRules object with no rules active.
/// This function is called exactly once, by the `TransactionDenyRulesCreate`
/// end-of-epoch transaction that first creates the object.
fun create(ctx: &mut TxContext) {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    let version = CURRENT_VERSION;

    let inner = TransactionDenyRulesInnerV1 {
        version,
        denied_addresses: linked_table::new(ctx),
        denied_objects: linked_table::new(ctx),
        denied_packages: linked_table::new(ctx),
        package_publish_disabled: false,
        package_upgrade_disabled: false,
        shared_object_disabled: false,
        user_transaction_disabled: false,
        receiving_objects_disabled: false,
        move_authenticator_disabled: false,
    };

    let self = TransactionDenyRules {
        id: object::transaction_deny_rules(),
        inner: versioned::create(version, inner, ctx),
    };
    transfer::share_object(self);
}

#[test_only]
public fun create_for_testing(ctx: &mut TxContext) {
    create(ctx);
}

fun load_inner_mut(self: &mut TransactionDenyRules): &mut TransactionDenyRulesInnerV1 {
    let version = self.inner.version();

    // Replace this with a lazy update function when we add a new version of the inner object.
    assert!(version == CURRENT_VERSION, EWrongInnerVersion);
    let inner: &mut TransactionDenyRulesInnerV1 = self.inner.load_value_mut();
    assert!(inner.version == version, EWrongInnerVersion);
    inner
}

#[test_only]
fun load_inner(self: &TransactionDenyRules): &TransactionDenyRulesInnerV1 {
    let version = self.inner.version();

    // Replace this with a lazy update function when we add a new version of the inner object.
    assert!(version == CURRENT_VERSION, EWrongInnerVersion);
    let inner: &TransactionDenyRulesInnerV1 = self.inner.load_value();
    assert!(inner.version == version, EWrongInnerVersion);
    inner
}

#[allow(unused_function)]
/// Apply an add/remove delta to the deny lists and set the switch states.
/// Called when executing a `TransactionDenyRulesUpdate` system transaction;
/// a large delta arrives split across several such transactions.
fun update(
    self: &mut TransactionDenyRules,
    added_addresses: vector<address>,
    removed_addresses: vector<address>,
    added_objects: vector<ID>,
    removed_objects: vector<ID>,
    added_packages: vector<ID>,
    removed_packages: vector<ID>,
    package_publish_disabled: bool,
    package_upgrade_disabled: bool,
    shared_object_disabled: bool,
    user_transaction_disabled: bool,
    receiving_objects_disabled: bool,
    move_authenticator_disabled: bool,
    ctx: &TxContext,
) {
    // Validator will make a special system call with sender set as 0x0.
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    let inner = self.load_inner_mut();
    apply_delta(&mut inner.denied_addresses, &added_addresses, &removed_addresses);
    apply_delta(&mut inner.denied_objects, &added_objects, &removed_objects);
    apply_delta(&mut inner.denied_packages, &added_packages, &removed_packages);
    inner.package_publish_disabled = package_publish_disabled;
    inner.package_upgrade_disabled = package_upgrade_disabled;
    inner.shared_object_disabled = shared_object_disabled;
    inner.user_transaction_disabled = user_transaction_disabled;
    inner.receiving_objects_disabled = receiving_objects_disabled;
    inner.move_authenticator_disabled = move_authenticator_disabled;

    event::emit(TransactionDenyRulesUpdated {
        epoch: ctx.epoch(),
        added_addresses,
        removed_addresses,
        added_objects,
        removed_objects,
        added_packages,
        removed_packages,
        package_publish_disabled,
        package_upgrade_disabled,
        shared_object_disabled,
        user_transaction_disabled,
        receiving_objects_disabled,
        move_authenticator_disabled,
        denied_addresses_len: inner.denied_addresses.length(),
        denied_objects_len: inner.denied_objects.length(),
        denied_packages_len: inner.denied_packages.length(),
    });
}

/// Remove `removed` from `list`, then add `added`. Both are tolerant — keys
/// already absent or already present are skipped — so a system transaction
/// can never abort here and re-applying a delta is a no-op.
fun apply_delta<Key: copy + drop + store>(
    list: &mut LinkedTable<Key, bool>,
    added: &vector<Key>,
    removed: &vector<Key>,
) {
    let mut i = 0;
    while (i < removed.length()) {
        let key = removed[i];
        if (list.contains(key)) {
            list.remove(key);
        };
        i = i + 1;
    };
    let mut i = 0;
    while (i < added.length()) {
        let key = added[i];
        if (!list.contains(key)) {
            list.push_back(key, true);
        };
        i = i + 1;
    };
}

#[test_only]
public fun update_for_testing(
    self: &mut TransactionDenyRules,
    added_addresses: vector<address>,
    removed_addresses: vector<address>,
    added_objects: vector<ID>,
    removed_objects: vector<ID>,
    added_packages: vector<ID>,
    removed_packages: vector<ID>,
    package_publish_disabled: bool,
    package_upgrade_disabled: bool,
    shared_object_disabled: bool,
    user_transaction_disabled: bool,
    receiving_objects_disabled: bool,
    move_authenticator_disabled: bool,
    ctx: &TxContext,
) {
    self.update(
        added_addresses,
        removed_addresses,
        added_objects,
        removed_objects,
        added_packages,
        removed_packages,
        package_publish_disabled,
        package_upgrade_disabled,
        shared_object_disabled,
        user_transaction_disabled,
        receiving_objects_disabled,
        move_authenticator_disabled,
        ctx,
    );
}

#[test_only]
/// Assert that the stored state equals the expected full state: exact deny
/// list membership (the expected vectors must be duplicate-free) and every
/// switch value.
public fun assert_state_for_testing(
    self: &TransactionDenyRules,
    denied_addresses: vector<address>,
    denied_objects: vector<ID>,
    denied_packages: vector<ID>,
    package_publish_disabled: bool,
    package_upgrade_disabled: bool,
    shared_object_disabled: bool,
    user_transaction_disabled: bool,
    receiving_objects_disabled: bool,
    move_authenticator_disabled: bool,
) {
    let inner = self.load_inner();
    assert_members(&inner.denied_addresses, &denied_addresses);
    assert_members(&inner.denied_objects, &denied_objects);
    assert_members(&inner.denied_packages, &denied_packages);
    assert!(inner.package_publish_disabled == package_publish_disabled);
    assert!(inner.package_upgrade_disabled == package_upgrade_disabled);
    assert!(inner.shared_object_disabled == shared_object_disabled);
    assert!(inner.user_transaction_disabled == user_transaction_disabled);
    assert!(inner.receiving_objects_disabled == receiving_objects_disabled);
    assert!(inner.move_authenticator_disabled == move_authenticator_disabled);
}

#[test_only]
/// Returns (added_addresses, removed_addresses, denied_addresses_len) of an
/// update event.
public fun event_addresses_for_testing(
    event: &TransactionDenyRulesUpdated,
): (vector<address>, vector<address>, u64) {
    (event.added_addresses, event.removed_addresses, event.denied_addresses_len)
}

#[test_only]
fun assert_members<Key: copy + drop + store>(
    list: &LinkedTable<Key, bool>,
    expected: &vector<Key>,
) {
    assert!(list.length() == expected.length());
    let mut i = 0;
    while (i < expected.length()) {
        assert!(list.contains(expected[i]));
        i = i + 1;
    };
}
