// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A permissionless registry of third-party attestors for explicit
/// transaction attestations. Anyone can register a dedicated attestor
/// signing key by locking a bond; registrations, deregistrations and key
/// rotations take effect at epoch boundaries.
///
/// The escrow is held in two parts: an at-stake bond capped at the joining
/// bond — the only part slashing draws from and the eviction check reads —
/// and the excess above it. Top-ups join the excess and are folded into the
/// at-stake bond only at the boundary rebalance, so they cannot rescue an
/// attestor whose at-stake bond was slashed below the low-bond threshold in
/// the current epoch. An attestor below the threshold at the boundary is
/// evicted and its entire escrow (at-stake and excess) burned.
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
use iota::dynamic_field;
use iota::event;
use iota::iota::IOTA;
use iota::url::{Self, Url};
use iota_system::protocol_config;
use std::string::String;

// Protocol config parameter names, read via `protocol_config::get_attr`.
// The bond levels are rates (basis points) applied to
// `min_validator_joining_stake`, so they stay consistent as the stake is
// tuned.
const MIN_VALIDATOR_JOINING_STAKE_PARAM: vector<u8> = b"min_validator_joining_stake";
const ATTESTOR_JOINING_BOND_RATE_PARAM: vector<u8> = b"attestor_joining_bond_rate";
const ATTESTOR_LOW_BOND_THRESHOLD_RATE_PARAM: vector<u8> = b"attestor_low_bond_threshold_rate";
const MAX_ATTESTOR_COUNT_PARAM: vector<u8> = b"max_attestor_count";
const ATTESTOR_MAX_INACTIVITY_EPOCHS_PARAM: vector<u8> = b"attestor_max_inactivity_epochs";
const ATTESTOR_INACTIVITY_PENALTY_PARAM: vector<u8> = b"attestor_inactivity_penalty";

const BASIS_POINT_DENOMINATOR: u128 = 10000;

// Exit reasons for advance_epoch's combined exit pass, in precedence
// order: eviction > inactivity > voluntary removal.
const EXIT_EVICTION: u8 = 0;
const EXIT_INACTIVITY: u8 = 1;
const EXIT_REMOVAL: u8 = 2;

const MAX_ATTESTOR_METADATA_LENGTH: u64 = 256;

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
// Returned by the validate_attestor_pubkey native, not asserted in Move.
#[allow(unused_const)]
const EInvalidProofOfPossession: u64 = 8;
const EDuplicatePubkey: u64 = 9;
const EAttestorMetadataExceedingLengthLimit: u64 = 10;
const ENoMetadataEntry: u64 = 11;

/// Key for the attestor registry dynamic field on the IotaSystemState UID.
public struct AttestorRegistryKey has copy, drop, store {}

/// Key for one attestor's metadata dynamic field on the IotaSystemState UID.
public struct AttestorMetadataKey has copy, drop, store {
    attestor_address: address,
}

/// Display metadata, validated like the validator metadata. Lives as a
/// per-attestor dynamic field, not inline in the registry: the registry is
/// one object and 1000 permissionless entries of display strings would
/// breach the object-size limit.
public struct AttestorMetadataV1 has drop, store {
    name: String,
    description: String,
    url: Url,
    logo: Url,
}

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

/// Departures from the active set at an epoch boundary. Has no abilities,
/// so the caller of `advance_epoch` cannot drop or store it — it must be
/// consumed by `remove_departed_metadata`, guaranteeing the departed
/// attestors' metadata fields are cleaned up.
public struct DepartedAttestors {
    addresses: vector<address>,
}

public struct AttestorV1 has store {
    /// Identity address (= ctx.sender() at registration).
    attestor_address: address,
    /// Dedicated signing key: flag byte || raw pubkey bytes.
    attestor_pubkey: vector<u8>,
    /// Staged replacement key, applied in place at the next epoch boundary.
    next_epoch_attestor_pubkey: Option<vector<u8>>,
    /// At-stake part of the escrow: what slashing draws from and the
    /// eviction check reads. Rebalanced to min(total, joining bond) at
    /// each epoch boundary.
    bond: Balance<IOTA>,
    /// Escrow above the joining bond; top-ups land here and fold into
    /// `bond` only at the boundary rebalance.
    excess_bond: Balance<IOTA>,
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

public struct AttestorDeregisterRequestedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
}

public struct AttestorRemovedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    refunded_amount: u64,
}

public struct AttestorBondDepositedEvent has copy, drop {
    epoch: u64,
    attestor_address: address,
    deposited_amount: u64,
    new_bond_amount: u64,
}

/// One epoch boundary's worth of activations, batched into a single event
/// so a full registry can't exceed the per-tx event count cap.
public struct AttestorsActivatedEvent has copy, drop {
    epoch: u64,
    attestors: vector<address>,
}

/// One departed attestor; `reason` is EXIT_EVICTION / EXIT_INACTIVITY /
/// EXIT_REMOVAL. Eviction burns the whole escrow, excess included
/// (refunded=0); inactivity burns the penalty and refunds the rest;
/// removal refunds the whole escrow (burned=0).
public struct AttestorExitInfo has copy, drop, store {
    attestor_address: address,
    reason: u8,
    refunded_amount: u64,
    burned_amount: u64,
}

/// One epoch boundary's worth of exits, batched into a single event so a
/// full registry can't exceed the per-tx event count cap.
public struct AttestorsExitedEvent has copy, drop {
    epoch: u64,
    exited: vector<AttestorExitInfo>,
}

/// Whether the external-attestation protocol feature is enabled on this
/// chain.
public(package) fun is_feature_enabled(): bool {
    protocol_config::is_feature_enabled(b"enable_external_attestation")
}

/// Aborts unless the feature is enabled. Gates all user-facing registry entry
/// points.
public(package) fun assert_feature_enabled() {
    assert!(is_feature_enabled(), EFeatureNotEnabled);
}

/// Minimum bond to register as an attestor.
public(package) fun min_joining_bond(): u64 {
    stake_fraction(ATTESTOR_JOINING_BOND_RATE_PARAM)
}

/// Bond level below which an active attestor is evicted at the epoch boundary.
public(package) fun low_bond_threshold(): u64 {
    stake_fraction(ATTESTOR_LOW_BOND_THRESHOLD_RATE_PARAM)
}

/// `min_validator_joining_stake` scaled by the rate (basis points) named by
/// `rate_param`.
fun stake_fraction(rate_param: vector<u8>): u64 {
    let stake = protocol_config::get_attr<u64>(MIN_VALIDATOR_JOINING_STAKE_PARAM);
    let rate = protocol_config::get_attr<u64>(rate_param);
    ((stake as u128) * (rate as u128) / BASIS_POINT_DENOMINATOR) as u64
}

// === Construction ===

public(package) fun registry_key(): AttestorRegistryKey { AttestorRegistryKey {} }

public(package) fun metadata_key(attestor_address: address): AttestorMetadataKey {
    AttestorMetadataKey { attestor_address }
}

public(package) fun new(): AttestorRegistryV1 {
    AttestorRegistryV1 {
        active_attestors: vector[],
        pending_active: vector[],
        pending_removals: vector[],
    }
}

// === Pubkey validation ===

/// Validate a `flag || raw_key` attestor signing key (plain schemes only)
/// and its proof of possession: the raw signature by that key over
/// `bcs(IntentMessage(ProofOfPossession, pubkey || sender))`. Aborts with
/// `EInvalidPubkey` / `EInvalidProofOfPossession`. Implemented as a native.
native fun validate_attestor_pubkey(
    pubkey: vector<u8>,
    proof_of_possession: vector<u8>,
    sender: address,
);

// === Lookup helpers ===

fun find_active(self: &AttestorRegistryV1, addr: address): Option<u64> {
    self.active_attestors.find_index!(|a| a.attestor_address == addr)
}

fun find_pending(self: &AttestorRegistryV1, addr: address): Option<u64> {
    self.pending_active.find_index!(|a| a.attestor_address == addr)
}

/// Whether `pubkey` is already held by an active attestor (current or
/// staged) or a pending registration.
fun pubkey_in_use(self: &AttestorRegistryV1, pubkey: &vector<u8>): bool {
    self.active_attestors.any!(|a| {
        a.attestor_pubkey == *pubkey
            || a.next_epoch_attestor_pubkey.contains(pubkey)
    }) || self.pending_active.any!(|a| a.attestor_pubkey == *pubkey)
}

// === Registration ===

/// Register `sender` as an attestor with the given dedicated signing key,
/// locking `bond`. Takes effect at the next epoch boundary.
public(package) fun register(
    self: &mut AttestorRegistryV1,
    mut bond: Balance<IOTA>,
    attestor_pubkey: vector<u8>,
    proof_of_possession: vector<u8>,
    sender: address,
    current_epoch: u64,
) {
    let min_joining_bond = min_joining_bond();
    assert!(bond.value() >= min_joining_bond, EBondTooLow);
    assert!(
        self.active_attestors.length() + self.pending_active.length()
            < protocol_config::get_attr(MAX_ATTESTOR_COUNT_PARAM),
        ETooManyAttestors,
    );
    validate_attestor_pubkey(attestor_pubkey, proof_of_possession, sender);
    // An entry scheduled for removal is still in `active_attestors` until
    // the boundary, so this also blocks re-registering while exiting.
    assert!(find_active(self, sender).is_none(), EAlreadyRegistered);
    assert!(find_pending(self, sender).is_none(), EAlreadyRegistered);
    assert!(!pubkey_in_use(self, &attestor_pubkey), EDuplicatePubkey);

    let activation_epoch = current_epoch + 1;
    let bond_amount = bond.value();
    let excess_bond = bond.split(bond_amount - min_joining_bond);
    let pubkey_for_event = attestor_pubkey;
    self
        .pending_active
        .push_back(AttestorV1 {
            attestor_address: sender,
            attestor_pubkey: pubkey_for_event,
            next_epoch_attestor_pubkey: option::none(),
            bond,
            excess_bond,
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
            mut bond,
            excess_bond,
            activation_epoch: _,
            last_active_epoch: _,
        } = self.pending_active.remove(pending_idx.destroy_some());
        next_epoch_attestor_pubkey.destroy_none();
        bond.join(excess_bond);
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

/// Add `additional` to the sender's excess (active or pending entry). It
/// folds into the at-stake bond only at the boundary rebalance, so the
/// eviction check at the very next boundary does not see it.
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
    entry.excess_bond.join(additional);
    event::emit(AttestorBondDepositedEvent {
        epoch: current_epoch,
        attestor_address: sender,
        deposited_amount,
        new_bond_amount: entry.bond.value() + entry.excess_bond.value(),
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
    proof_of_possession: vector<u8>,
) {
    let active_idx = find_active(self, sender);
    assert!(active_idx.is_some(), ENotActiveAttestor);
    let idx = active_idx.destroy_some();
    assert!(!self.pending_removals.contains(&idx), EAlreadyDeregistering);
    validate_attestor_pubkey(new_pubkey, proof_of_possession, sender);
    assert!(!pubkey_in_use(self, &new_pubkey), EDuplicatePubkey);
    let entry = &mut self.active_attestors[idx];
    entry.next_epoch_attestor_pubkey = option::some(new_pubkey);
}

// === Epoch boundary processing ===

/// Process the epoch boundary for the registry. Order:
/// 0. (Reserved) slashing executes before exits — see the design doc.
/// 1. Combined exits, one pass so the stored indices stay valid; per-entry
///    reason precedence: low-bond eviction (whole escrow burned, excess
///    included) > inactivity drop (penalty burned, rest refunded) >
///    requested removal (escrow refunded). The eviction check reads the
///    at-stake bond as slashing left it — before the rebalance below — so
///    an in-epoch top-up cannot rescue a threshold-crossing slash.
///    Inactivity beating a pending removal means an inactive attestor
///    cannot escape the penalty by deregistering in the same epoch.
/// 2. Staged key rotations and the bond rebalance (at-stake =
///    min(total, current joining bond)), in place.
/// 3. Pending activations appended in registration order; an entry whose
///    total escrow is below the current joining bond is refused and
///    refunded like a voluntary removal, the rest rebalanced like actives.
/// Emits at most one `AttestorsExitedEvent` and one `AttestorsActivatedEvent`
/// for the whole boundary, batching every departed/activated attestor into
/// them — a per-attestor event here would risk exceeding the per-tx event
/// count cap with a full registry.
/// Returns the evicted bonds and penalties (the caller burns them via the
/// treasury cap) and the addresses that left the active set, for the
/// caller to drop their metadata via `remove_departed_metadata`.
public(package) fun advance_epoch(
    self: &mut AttestorRegistryV1,
    new_epoch: u64,
    feature_enabled: bool,
    ctx: &mut TxContext,
): (Balance<IOTA>, DepartedAttestors) {
    let mut evicted_bonds = balance::zero<IOTA>();
    let mut departed = vector<address>[];
    let mut exited = vector<AttestorExitInfo>[];

    // --- 1. Combined exits ---
    // Only active attestors can ever exit, and only active attestors can
    // populate `pending_removals` (deregister requires an active entry), so
    // a chain with the feature flag on but the exit-threshold params unset
    // is safe here as long as the active set is empty (register also reads
    // params and would already abort, so it can't be populated otherwise).
    //
    // With the feature disabled only voluntary removals are processed: they
    // read no params, so escrowed bonds can always exit, while eviction and
    // inactivity stay suspended.
    if (!self.active_attestors.is_empty()) {
        let mut inactivity_penalty: u64 = 0;
        let mut exit_indices = vector<u64>[];
        let mut exit_reasons = vector<u8>[];
        if (feature_enabled) {
            let low_bond_threshold = low_bond_threshold();
            let max_inactivity_epochs: u64 = protocol_config::get_attr(
                ATTESTOR_MAX_INACTIVITY_EPOCHS_PARAM,
            );
            inactivity_penalty = protocol_config::get_attr(ATTESTOR_INACTIVITY_PENALTY_PARAM);
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
        };
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
                excess_bond,
                activation_epoch: _,
                last_active_epoch: _,
            } = self.active_attestors.remove(idx);
            next_epoch_attestor_pubkey.destroy!(|_| ());
            bond.join(excess_bond);
            departed.push_back(attestor_address);
            if (reason == EXIT_EVICTION) {
                let burned_amount = bond.value();
                exited.push_back(AttestorExitInfo {
                    attestor_address,
                    reason,
                    refunded_amount: 0,
                    burned_amount,
                });
                evicted_bonds.join(bond);
            } else if (reason == EXIT_INACTIVITY) {
                let penalty_amount = inactivity_penalty.min(bond.value());
                evicted_bonds.join(bond.split(penalty_amount));
                exited.push_back(AttestorExitInfo {
                    attestor_address,
                    reason,
                    refunded_amount: bond.value(),
                    burned_amount: penalty_amount,
                });
                transfer::public_transfer(coin::from_balance(bond, ctx), attestor_address);
            } else {
                exited.push_back(AttestorExitInfo {
                    attestor_address,
                    reason,
                    refunded_amount: bond.value(),
                    burned_amount: 0,
                });
                transfer::public_transfer(coin::from_balance(bond, ctx), attestor_address);
            }
        };
    };

    // --- 2. Staged key rotations and bond rebalance, in place ---
    // The param read is guarded like the exit pass: an active entry exists
    // only if register() read the same params.
    let len = self.active_attestors.length();
    if (feature_enabled && len > 0) {
        let min_joining_bond = min_joining_bond();
        let mut k = 0;
        while (k < len) {
            let entry = &mut self.active_attestors[k];
            if (entry.next_epoch_attestor_pubkey.is_some()) {
                entry.attestor_pubkey = entry.next_epoch_attestor_pubkey.extract();
            };
            rebalance(entry, min_joining_bond);
            k = k + 1;
        };
    };

    // --- 3. Activations, in registration order ---
    // The total escrow is re-checked against the current joining bond: a
    // raise between registration and activation that the escrow (including
    // top-ups) no longer covers refuses the entry, refunding it like a
    // voluntary removal. The param read is guarded like the exit pass
    // above: a pending entry exists only if register() read the same
    // params, so the read cannot abort here.
    let mut activated = vector<address>[];
    if (feature_enabled && !self.pending_active.is_empty()) {
        let min_joining_bond = min_joining_bond();
        self.pending_active.reverse();
        while (!self.pending_active.is_empty()) {
            let mut entry = self.pending_active.pop_back();
            if (entry.bond.value() + entry.excess_bond.value() < min_joining_bond) {
                let AttestorV1 {
                    attestor_address,
                    attestor_pubkey: _,
                    next_epoch_attestor_pubkey,
                    mut bond,
                    excess_bond,
                    activation_epoch: _,
                    last_active_epoch: _,
                } = entry;
                next_epoch_attestor_pubkey.destroy!(|_| ());
                bond.join(excess_bond);
                departed.push_back(attestor_address);
                exited.push_back(AttestorExitInfo {
                    attestor_address,
                    reason: EXIT_REMOVAL,
                    refunded_amount: bond.value(),
                    burned_amount: 0,
                });
                transfer::public_transfer(coin::from_balance(bond, ctx), attestor_address);
            } else {
                rebalance(&mut entry, min_joining_bond);
                activated.push_back(entry.attestor_address);
                self.active_attestors.push_back(entry);
            }
        };
    };

    if (!activated.is_empty()) {
        event::emit(AttestorsActivatedEvent { epoch: new_epoch, attestors: activated });
    };
    if (!exited.is_empty()) {
        event::emit(AttestorsExitedEvent { epoch: new_epoch, exited });
    };

    (evicted_bonds, DepartedAttestors { addresses: departed })
}

/// Restore the boundary invariant: at-stake = min(total escrow, the
/// current joining bond), the rest held as excess.
fun rebalance(entry: &mut AttestorV1, min_joining_bond: u64) {
    let at_stake = entry.bond.value();
    let target = (at_stake + entry.excess_bond.value()).min(min_joining_bond);
    if (at_stake < target) {
        entry.bond.join(entry.excess_bond.split(target - at_stake));
    } else if (at_stake > target) {
        entry.excess_bond.join(entry.bond.split(at_stake - target));
    };
}

/// Consume the departure list, removing each departed attestor's metadata
/// dynamic field. The only way to dispose of a `DepartedAttestors`.
public(package) fun remove_departed_metadata(departed: DepartedAttestors, uid: &mut UID) {
    let DepartedAttestors { addresses } = departed;
    addresses.do!(|addr| remove_metadata(uid, addr));
}

// === Metadata ===

fun validated_string(bytes: vector<u8>): String {
    assert!(
        bytes.length() <= MAX_ATTESTOR_METADATA_LENGTH,
        EAttestorMetadataExceedingLengthLimit,
    );
    bytes.to_ascii_string().to_string()
}

fun validated_url(bytes: vector<u8>): Url {
    assert!(
        bytes.length() <= MAX_ATTESTOR_METADATA_LENGTH,
        EAttestorMetadataExceedingLengthLimit,
    );
    url::new_unsafe_from_bytes(bytes)
}

public(package) fun add_metadata(
    uid: &mut UID,
    attestor_address: address,
    name: vector<u8>,
    description: vector<u8>,
    url: vector<u8>,
    logo: vector<u8>,
) {
    dynamic_field::add(
        uid,
        metadata_key(attestor_address),
        AttestorMetadataV1 {
            name: validated_string(name),
            description: validated_string(description),
            url: validated_url(url),
            logo: validated_url(logo),
        },
    );
}

/// Tolerates a missing field: it runs inside the epoch-change transaction,
/// which a broken registry/metadata pairing must not abort.
public(package) fun remove_metadata(uid: &mut UID, attestor_address: address) {
    if (dynamic_field::exists_(uid, metadata_key(attestor_address))) {
        let _: AttestorMetadataV1 = dynamic_field::remove(uid, metadata_key(attestor_address));
    }
}

fun borrow_metadata_mut(uid: &mut UID, sender: address): &mut AttestorMetadataV1 {
    assert!(dynamic_field::exists_(uid, metadata_key(sender)), ENoMetadataEntry);
    dynamic_field::borrow_mut(uid, metadata_key(sender))
}

public(package) fun update_metadata_name(uid: &mut UID, sender: address, name: vector<u8>) {
    borrow_metadata_mut(uid, sender).name = validated_string(name);
}

public(package) fun update_metadata_description(
    uid: &mut UID,
    sender: address,
    description: vector<u8>,
) {
    borrow_metadata_mut(uid, sender).description = validated_string(description);
}

public(package) fun update_metadata_url(uid: &mut UID, sender: address, url: vector<u8>) {
    borrow_metadata_mut(uid, sender).url = validated_url(url);
}

public(package) fun update_metadata_logo(uid: &mut UID, sender: address, logo: vector<u8>) {
    borrow_metadata_mut(uid, sender).logo = validated_url(logo);
}

// === Slash hook (no public trigger yet) ===

/// Deduct up to `amount` from an active attestor's at-stake bond and
/// return it; the excess is untouched until the boundary rebalance.
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

public(package) fun excess_bond_value(attestor: &AttestorV1): u64 {
    attestor.excess_bond.value()
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
/// Unpack for assertions; event fields are private to this module.
public fun unpack_activated_event_for_testing(
    event: AttestorsActivatedEvent,
): (u64, vector<address>) {
    let AttestorsActivatedEvent { epoch, attestors } = event;
    (epoch, attestors)
}

#[test_only]
/// Unpack for assertions; event fields are private to this module.
public fun unpack_exited_event_for_testing(
    event: AttestorsExitedEvent,
): (u64, vector<AttestorExitInfo>) {
    let AttestorsExitedEvent { epoch, exited } = event;
    (epoch, exited)
}

#[test_only]
/// Unpack for assertions; struct fields are private to this module.
public fun unpack_exit_info_for_testing(info: AttestorExitInfo): (address, u8, u64, u64) {
    let AttestorExitInfo { attestor_address, reason, refunded_amount, burned_amount } = info;
    (attestor_address, reason, refunded_amount, burned_amount)
}

#[test_only]
public fun destroy_for_testing(self: AttestorRegistryV1) {
    let AttestorRegistryV1 { active_attestors, pending_active, pending_removals: _ } = self;
    active_attestors.destroy!(|a| destroy_attestor_for_testing(a));
    pending_active.destroy!(|a| destroy_attestor_for_testing(a));
}

#[test_only]
/// Unpack without removing metadata; returns the addresses for assertions.
public fun unpack_for_testing(self: DepartedAttestors): vector<address> {
    let DepartedAttestors { addresses } = self;
    addresses
}

#[test_only]
fun destroy_attestor_for_testing(attestor: AttestorV1) {
    let AttestorV1 {
        attestor_address: _,
        attestor_pubkey: _,
        next_epoch_attestor_pubkey,
        bond,
        excess_bond,
        activation_epoch: _,
        last_active_epoch: _,
    } = attestor;
    next_epoch_attestor_pubkey.destroy!(|_| ());
    bond.destroy_for_testing();
    excess_bond.destroy_for_testing();
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
            excess_bond: balance::zero(),
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
