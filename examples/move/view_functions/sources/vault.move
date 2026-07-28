// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::vault;

use iota::clock::Clock;

#[error(code = 0)]
const EStillLocked: vector<u8> = b"The vault is still time-locked.";
#[error(code = 1)]
const ENotAllowed: vector<u8> = b"Sender is not allowed to unlock this vault.";

/// A shared, generic vault holding a single value of type `T`.
///
/// The contents stay locked until `unlock_at`, after which only `beneficiary`
/// may release them.
public struct Vault<T: store> has key {
    id: UID,
    item: T,
    /// Unix timestamp (ms) before which the vault cannot be unlocked.
    unlock_at: u64,
    /// The only address permitted to unlock the vault.
    beneficiary: address,
}

/// Create and share a vault wrapping `item`, locked until `unlock_at` and
/// releasable only by `beneficiary`.
public fun create<T: store>(item: T, unlock_at: u64, beneficiary: address, ctx: &mut TxContext) {
    transfer::share_object(Vault { id: object::new(ctx), item, unlock_at, beneficiary });
}

/// Unlock the vault and return the stored item.
///
/// Aborts if the time lock has not elapsed, or if the sender is not the
/// `beneficiary`.
public fun unlock<T: store>(vault: Vault<T>, clock: &Clock, ctx: &TxContext): T {
    assert!(clock.timestamp_ms() >= vault.unlock_at, EStillLocked);
    assert!(ctx.sender() == vault.beneficiary, ENotAllowed);
    let Vault { id, item, unlock_at: _, beneficiary: _ } = vault;
    id.delete();
    item
}

/// Returns an immutable reference to the stored item.
///
/// The view itself places no constraint on `T`: a type parameter used only
/// behind a reference may be unconstrained.
#[view]
public fun item<T: store>(vault: &Vault<T>): &T {
    &vault.item
}

/// Returns the timestamp (ms) before which the vault stays locked.
#[view]
public fun unlock_at<T: store>(vault: &Vault<T>): u64 {
    vault.unlock_at
}

/// Returns the only address allowed to unlock the vault.
#[view]
public fun beneficiary<T: store>(vault: &Vault<T>): address {
    vault.beneficiary
}

/// Returns true if the time lock has elapsed as of `clock`.
#[view]
public fun is_unlockable<T: store>(vault: &Vault<T>, clock: &Clock): bool {
    clock.timestamp_ms() >= vault.unlock_at
}
