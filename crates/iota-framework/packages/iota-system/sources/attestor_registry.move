// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A permissionless registry of third-party attestors for explicit
/// transaction attestations. Anyone can register a dedicated attestor
/// signing key by locking a bond; registrations, deregistrations and key
/// rotations take effect at epoch boundaries. An active attestor whose bond
/// falls below the low-bond threshold at an epoch boundary has its
/// remaining bond burned and is evicted.
///
/// An active attestor that goes unreported by `refresh_activity` for more
/// than the configured number of epochs is dropped at the boundary: a
/// fixed penalty is burned from its bond and the remainder refunded.
///
/// The registry is stored as a dynamic field on the `IotaSystemState`
/// wrapper object under `AttestorRegistryKey`, and follows the
/// `ValidatorSet` design: the active set is an ordered vector, an
/// attestor's per-epoch index is its position in that vector at the start
/// of the epoch.
module iota_system::attestor_registry;

use iota::balance::{Self, Balance};
use iota::coin;
use iota::event;
use iota::iota::IOTA;
use iota_system::protocol_config;

// Protocol config parameter names, read via `protocol_config::get_attr`.
const MIN_ATTESTOR_JOINING_BOND_PARAM: vector<u8> = b"min_attestor_joining_bond";
const ATTESTOR_LOW_BOND_THRESHOLD_PARAM: vector<u8> = b"attestor_low_bond_threshold";
const MAX_ATTESTOR_COUNT_PARAM: vector<u8> = b"max_attestor_count";
const ATTESTOR_MAX_INACTIVITY_EPOCHS_PARAM: vector<u8> = b"attestor_max_inactivity_epochs";
const ATTESTOR_INACTIVITY_PENALTY_PARAM: vector<u8> = b"attestor_inactivity_penalty";

// Exit reasons for advance_epoch's combined exit pass, in precedence
// order: eviction > inactivity > voluntary removal.
const EXIT_EVICTION: u8 = 0;
const EXIT_INACTIVITY: u8 = 1;
const EXIT_REMOVAL: u8 = 2;

const EFeatureNotEnabled: u64 = 0;
const EBondTooLow: u64 = 1;
const ETooManyAttestors: u64 = 2;
// Returned by the validate_attestor_pubkey native, not asserted in Move.
#[allow(unused_const)]
const EInvalidPubkey: u64 = 3;
const EAlreadyRegistered: u64 = 4;
const ENotAnAttestor: u64 = 5;
const EAlreadyDeregistering: u64 = 6;
const ENotActiveAttestor: u64 = 7;

/// Key for the attestor registry dynamic field on the IotaSystemState UID.
public struct AttestorRegistryKey has copy, drop, store {}

public struct AttestorRegistryV1 has store {
    /// Active attestors, ordered. An attestor's per-epoch dense index is
    /// its position in this vector at the start of the epoch. Mutated only
    /// during advance_epoch.
    active_attestors: vector<AttestorV1>,
    /// Registrations awaiting the next epoch boundary, in registration order.
    pending_active: vector<AttestorV1>,
    /// Indices into `active_attestors` marked for removal at the next
    /// epoch boundary.
    pending_removals: vector<u64>,
}

public struct AttestorV1 has store {
    /// Identity address (= ctx.sender() at registration).
    attestor_address: address,
    /// Dedicated signing key: flag byte || raw pubkey bytes.
    attestor_pubkey: vector<u8>,
    /// Staged replacement key, applied in place at the next epoch boundary.
    next_epoch_attestor_pubkey: Option<vector<u8>>,
    /// Escrowed bond, held until removal (refund), eviction (burn), or a
    /// future slash.
    bond: Balance<IOTA>,
    /// Epoch from which this attestor is considered active.
    activation_epoch: u64,
    /// Last epoch in which this attestor was reported active via
    /// `refresh_activity`; starts at `activation_epoch`.
    last_active_epoch: u64,
}

// === Events ===

public struct AttestorRegisteredEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    attestor_pubkey: vector<u8>,
    bond_amount: u64,
    activation_epoch: u64,
}

public struct AttestorActivatedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
}

public struct AttestorDeregisterRequestedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
}

public struct AttestorRemovedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    refunded_amount: u64,
}

public struct AttestorEvictedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    burned_amount: u64,
}

public struct AttestorDroppedForInactivityEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    penalty_amount: u64,
    refunded_amount: u64,
}

public struct AttestorBondDepositedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    deposited_amount: u64,
    new_bond_amount: u64,
}

public struct AttestorKeyRotationStagedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    new_pubkey: vector<u8>,
}

public struct AttestorKeyRotatedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    new_pubkey: vector<u8>,
}

/// Whether the validator-attestation protocol feature is enabled on this chain.
public(package) fun is_feature_enabled(): bool {
    protocol_config::is_feature_enabled(b"enable_validator_attestation")
}

/// Aborts unless the feature is enabled. Gates all user-facing registry entry
/// points.
public(package) fun assert_feature_enabled() {
    assert!(is_feature_enabled(), EFeatureNotEnabled);
}

// === Construction ===

public(package) fun registry_key(): AttestorRegistryKey { AttestorRegistryKey {} }

public(package) fun new(): AttestorRegistryV1 {
    AttestorRegistryV1 {
        active_attestors: vector[],
        pending_active: vector[],
        pending_removals: vector[],
    }
}

// === Pubkey validation ===

/// Validate a `flag || raw_key` attestor signing key (plain schemes only).
/// Aborts with `EInvalidPubkey` otherwise. Implemented as a native.
native fun validate_attestor_pubkey(pubkey: vector<u8>);

// === Lookup helpers ===

fun find_active(self: &AttestorRegistryV1, addr: address): Option<u64> {
    self.active_attestors.find_index!(|a| a.attestor_address == addr)
}

fun find_pending(self: &AttestorRegistryV1, addr: address): Option<u64> {
    self.pending_active.find_index!(|a| a.attestor_address == addr)
}

// === Registration ===

/// Register `sender` as an attestor with the given dedicated signing key,
/// locking `bond`. Takes effect at the next epoch boundary.
public(package) fun register(
    self: &mut AttestorRegistryV1,
    bond: Balance<IOTA>,
    attestor_pubkey: vector<u8>,
    sender: address,
    current_epoch: u64,
) {
    assert!(
        bond.value() >= protocol_config::get_attr(MIN_ATTESTOR_JOINING_BOND_PARAM),
        EBondTooLow,
    );
    assert!(
        self.active_attestors.length() + self.pending_active.length()
            < protocol_config::get_attr(MAX_ATTESTOR_COUNT_PARAM),
        ETooManyAttestors,
    );
    validate_attestor_pubkey(attestor_pubkey);
    // An entry scheduled for removal is still in `active_attestors` until
    // the boundary, so this also blocks re-registering while exiting.
    assert!(find_active(self, sender).is_none(), EAlreadyRegistered);
    assert!(find_pending(self, sender).is_none(), EAlreadyRegistered);

    let activation_epoch = current_epoch + 1;
    let bond_amount = bond.value();
    let pubkey_for_event = attestor_pubkey;
    self
        .pending_active
        .push_back(AttestorV1 {
            attestor_address: sender,
            attestor_pubkey: pubkey_for_event,
            next_epoch_attestor_pubkey: option::none(),
            bond,
            activation_epoch,
            last_active_epoch: activation_epoch,
        });
    event::emit(AttestorRegisteredEvent {
        epoch: current_epoch,
        attestor_address: sender,
        attestor_pubkey: pubkey_for_event,
        bond_amount,
        activation_epoch,
    });
}

// === Deregistration ===

/// Deregister the sender. If the sender is still pending (never activated)
/// the entry is removed and the bond returned immediately (`Some`). If the
/// sender is active, removal is scheduled for the next epoch boundary and
/// `None` is returned; any staged key rotation is cancelled.
public(package) fun deregister(
    self: &mut AttestorRegistryV1,
    sender: address,
    current_epoch: u64,
): Option<Balance<IOTA>> {
    let pending_idx = find_pending(self, sender);
    if (pending_idx.is_some()) {
        let AttestorV1 {
            attestor_address: _,
            attestor_pubkey: _,
            next_epoch_attestor_pubkey,
            bond,
            activation_epoch: _,
            last_active_epoch: _,
        } = self.pending_active.remove(pending_idx.destroy_some());
        next_epoch_attestor_pubkey.destroy_none();
        event::emit(AttestorRemovedEvent {
            epoch: current_epoch,
            attestor_address: sender,
            refunded_amount: bond.value(),
        });
        return option::some(bond)
    };
    pending_idx.destroy_none();

    let active_idx = find_active(self, sender);
    assert!(active_idx.is_some(), ENotAnAttestor);
    let idx = active_idx.destroy_some();
    assert!(!self.pending_removals.contains(&idx), EAlreadyDeregistering);
    self.pending_removals.push_back(idx);
    // Cancel any staged rotation; the entry is leaving anyway.
    let entry = &mut self.active_attestors[idx];
    if (entry.next_epoch_attestor_pubkey.is_some()) {
        entry.next_epoch_attestor_pubkey = option::none();
    };
    event::emit(AttestorDeregisterRequestedEvent {
        epoch: current_epoch,
        attestor_address: sender,
    });
    option::none()
}

// === Bond top-up ===

/// Add `additional` to the sender's bond (active or pending entry).
/// Effective immediately; the boundary low-bond check sees the topped-up
/// balance.
public(package) fun deposit(
    self: &mut AttestorRegistryV1,
    sender: address,
    additional: Balance<IOTA>,
    current_epoch: u64,
) {
    let deposited_amount = additional.value();
    let active_idx = find_active(self, sender);
    let entry = if (active_idx.is_some()) {
        &mut self.active_attestors[active_idx.destroy_some()]
    } else {
        active_idx.destroy_none();
        let pending_idx = find_pending(self, sender);
        assert!(pending_idx.is_some(), ENotAnAttestor);
        &mut self.pending_active[pending_idx.destroy_some()]
    };
    entry.bond.join(additional);
    event::emit(AttestorBondDepositedEvent {
        epoch: current_epoch,
        attestor_address: sender,
        deposited_amount,
        new_bond_amount: entry.bond.value(),
    });
}

// === Activity tracking ===

/// Record that the attestors at `active_indices` — per-epoch dense
/// indices, i.e. positions in `active_attestors` at the start of
/// `ending_epoch` — were active during `ending_epoch`. Must run before
/// `advance_epoch` mutates the active set. Never aborts: out-of-range
/// indices are skipped, duplicates are idempotent (aborting would poison
/// the epoch-change transaction).
public(package) fun refresh_activity(
    self: &mut AttestorRegistryV1,
    active_indices: vector<u64>,
    ending_epoch: u64,
) {
    let len = self.active_attestors.length();
    active_indices.do!(|idx| {
        if (idx < len) {
            self.active_attestors[idx].last_active_epoch = ending_epoch;
        }
    });
}

// === Key rotation ===

/// Stage a replacement signing key for the sender's active entry; the key
/// is swapped in place at the next epoch boundary. Staging again before the
/// boundary overwrites the previously staged key.
public(package) fun rotate_key(
    self: &mut AttestorRegistryV1,
    sender: address,
    new_pubkey: vector<u8>,
    current_epoch: u64,
) {
    let active_idx = find_active(self, sender);
    assert!(active_idx.is_some(), ENotActiveAttestor);
    let idx = active_idx.destroy_some();
    assert!(!self.pending_removals.contains(&idx), EAlreadyDeregistering);
    validate_attestor_pubkey(new_pubkey);
    let new_pubkey_for_event = new_pubkey;
    let entry = &mut self.active_attestors[idx];
    entry.next_epoch_attestor_pubkey = option::some(new_pubkey_for_event);
    event::emit(AttestorKeyRotationStagedEvent {
        epoch: current_epoch,
        attestor_address: sender,
        new_pubkey: new_pubkey_for_event,
    });
}

// === Epoch boundary processing ===

/// Process the epoch boundary for the registry. Order:
/// 0. (Reserved) slashing executes before exits — see the design doc.
/// 1. Combined exits, one pass so the stored indices stay valid; per-entry
///    reason precedence: low-bond eviction (bond burned) > inactivity drop
///    (penalty burned, rest refunded) > requested removal (bond refunded).
///    Inactivity beating a pending removal means an inactive attestor
///    cannot escape the penalty by deregistering in the same epoch.
/// 2. Staged key rotations applied in place.
/// 3. Pending activations appended in registration order.
/// Returns the evicted bonds and penalties; the caller burns them via the
/// treasury cap.
public(package) fun advance_epoch(
    self: &mut AttestorRegistryV1,
    new_epoch: u64,
    ctx: &mut TxContext,
): Balance<IOTA> {
    let mut evicted_bonds = balance::zero<IOTA>();

    // --- 1. Combined exits ---
    let low_bond_threshold: u64 = protocol_config::get_attr(ATTESTOR_LOW_BOND_THRESHOLD_PARAM);
    let max_inactivity_epochs: u64 = protocol_config::get_attr(
        ATTESTOR_MAX_INACTIVITY_EPOCHS_PARAM,
    );
    let inactivity_penalty: u64 = protocol_config::get_attr(ATTESTOR_INACTIVITY_PENALTY_PARAM);
    let mut exit_indices = vector<u64>[];
    let mut exit_reasons = vector<u8>[];
    self.active_attestors.length().do!(|i| {
        let entry = &self.active_attestors[i];
        if (entry.bond.value() < low_bond_threshold) {
            exit_indices.push_back(i);
            exit_reasons.push_back(EXIT_EVICTION);
        } else if (new_epoch - entry.last_active_epoch > max_inactivity_epochs) {
            exit_indices.push_back(i);
            exit_reasons.push_back(EXIT_INACTIVITY);
        }
    });
    // Add voluntary removals not already exiting for a stronger reason.
    while (!self.pending_removals.is_empty()) {
        let idx = self.pending_removals.pop_back();
        if (!exit_indices.contains(&idx)) {
            exit_indices.push_back(idx);
            exit_reasons.push_back(EXIT_REMOVAL);
        }
    };
    // Sort ascending, then remove from the back so indices stay valid.
    let mut i = 1;
    while (i < exit_indices.length()) {
        let mut j = i;
        while (j > 0 && exit_indices[j - 1] > exit_indices[j]) {
            exit_indices.swap(j - 1, j);
            exit_reasons.swap(j - 1, j);
            j = j - 1;
        };
        i = i + 1;
    };
    while (!exit_indices.is_empty()) {
        let idx = exit_indices.pop_back();
        let reason = exit_reasons.pop_back();
        let AttestorV1 {
            attestor_address,
            attestor_pubkey: _,
            next_epoch_attestor_pubkey,
            mut bond,
            activation_epoch: _,
            last_active_epoch: _,
        } = self.active_attestors.remove(idx);
        next_epoch_attestor_pubkey.destroy!(|_| ());
        if (reason == EXIT_EVICTION) {
            event::emit(AttestorEvictedEvent {
                epoch: new_epoch,
                attestor_address,
                burned_amount: bond.value(),
            });
            evicted_bonds.join(bond);
        } else if (reason == EXIT_INACTIVITY) {
            let penalty_amount = inactivity_penalty.min(bond.value());
            evicted_bonds.join(bond.split(penalty_amount));
            event::emit(AttestorDroppedForInactivityEvent {
                epoch: new_epoch,
                attestor_address,
                penalty_amount,
                refunded_amount: bond.value(),
            });
            transfer::public_transfer(coin::from_balance(bond, ctx), attestor_address);
        } else {
            event::emit(AttestorRemovedEvent {
                epoch: new_epoch,
                attestor_address,
                refunded_amount: bond.value(),
            });
            transfer::public_transfer(coin::from_balance(bond, ctx), attestor_address);
        }
    };

    // --- 2. Staged key rotations, in place ---
    let len = self.active_attestors.length();
    let mut k = 0;
    while (k < len) {
        let entry = &mut self.active_attestors[k];
        if (entry.next_epoch_attestor_pubkey.is_some()) {
            entry.attestor_pubkey = entry.next_epoch_attestor_pubkey.extract();
            event::emit(AttestorKeyRotatedEvent {
                epoch: new_epoch,
                attestor_address: entry.attestor_address,
                new_pubkey: entry.attestor_pubkey,
            });
        };
        k = k + 1;
    };

    // --- 3. Activations, in registration order ---
    self.pending_active.reverse();
    while (!self.pending_active.is_empty()) {
        let entry = self.pending_active.pop_back();
        event::emit(AttestorActivatedEvent {
            epoch: new_epoch,
            attestor_address: entry.attestor_address,
        });
        self.active_attestors.push_back(entry);
    };

    evicted_bonds
}

// === Slash hook (no public trigger yet) ===

/// Deduct up to `amount` from an active attestor's bond and return it.
/// The trigger (evidence model, adjudication, destination of the returned
/// balance) is a future slashing design; today only tests call this.
public(package) fun slash(
    self: &mut AttestorRegistryV1,
    attestor_address: address,
    amount: u64,
): Balance<IOTA> {
    let active_idx = find_active(self, attestor_address);
    assert!(active_idx.is_some(), ENotAnAttestor);
    let entry = &mut self.active_attestors[active_idx.destroy_some()];
    let to_take = amount.min(entry.bond.value());
    entry.bond.split(to_take)
}

// === Accessors ===

public(package) fun active_count(self: &AttestorRegistryV1): u64 {
    self.active_attestors.length()
}

public(package) fun pending_count(self: &AttestorRegistryV1): u64 {
    self.pending_active.length()
}

public(package) fun attestor_address(attestor: &AttestorV1): address {
    attestor.attestor_address
}

public(package) fun attestor_pubkey(attestor: &AttestorV1): &vector<u8> {
    &attestor.attestor_pubkey
}

public(package) fun bond_value(attestor: &AttestorV1): u64 {
    attestor.bond.value()
}

public(package) fun activation_epoch(attestor: &AttestorV1): u64 {
    attestor.activation_epoch
}

public(package) fun last_active_epoch(attestor: &AttestorV1): u64 {
    attestor.last_active_epoch
}

public(package) fun active_attestors(self: &AttestorRegistryV1): &vector<AttestorV1> {
    &self.active_attestors
}

#[test_only]
public fun destroy_for_testing(self: AttestorRegistryV1) {
    let AttestorRegistryV1 { active_attestors, pending_active, pending_removals: _ } = self;
    active_attestors.destroy!(|a| destroy_attestor_for_testing(a));
    pending_active.destroy!(|a| destroy_attestor_for_testing(a));
}

#[test_only]
fun destroy_attestor_for_testing(attestor: AttestorV1) {
    let AttestorV1 {
        attestor_address: _,
        attestor_pubkey: _,
        next_epoch_attestor_pubkey,
        bond,
        activation_epoch: _,
        last_active_epoch: _,
    } = attestor;
    next_epoch_attestor_pubkey.destroy!(|_| ());
    bond.destroy_for_testing();
}

#[test_only]
/// Cheaply append a pending entry, bypassing the O(n) duplicate scan in
/// `register`. Used to fill the registry to capacity in tests without the
/// O(n^2) gas cost of 1000 real registrations.
public fun push_pending_for_testing(
    self: &mut AttestorRegistryV1,
    addr: address,
    bond_amount: u64,
) {
    self
        .pending_active
        .push_back(AttestorV1 {
            attestor_address: addr,
            attestor_pubkey: vector[],
            next_epoch_attestor_pubkey: option::none(),
            bond: balance::create_for_testing(bond_amount),
            activation_epoch: 0,
            last_active_epoch: 0,
        });
}

#[test_only]
/// Move all pending entries straight into the active set (test shortcut for
/// epoch activation; real processing lives in advance_epoch).
public fun activate_for_testing(self: &mut AttestorRegistryV1) {
    self.pending_active.reverse();
    while (!self.pending_active.is_empty()) {
        self.active_attestors.push_back(self.pending_active.pop_back());
    };
}
