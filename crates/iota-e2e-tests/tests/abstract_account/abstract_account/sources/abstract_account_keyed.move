// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::abstract_account_keyed;

use abstract_account::abstract_account::{Self, AbstractAccount};
use abstract_account::basic_keyed_aa;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::dynamic_object_field;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;

// === Errors ===

// === Constants ===

// === Structs ===

/// A simple shared counter used to exercise a Move authenticator that takes a
/// *mutable* shared object (see `authenticate_ed25519_and_increment`).
public struct Counter has key {
    id: UID,
    value: u64,
}

/// Dynamic-field key for a per-account authentication counter, mutated by
/// `authenticate_ed25519_and_mutate_account`.
public struct AuthCount has copy, drop, store {}

/// A fresh top-level object minted by `authenticate_ed25519_and_create_object`,
/// and also stored as a dynamic object field on a `Counter` for the extraction
/// test (`authenticate_ed25519_and_extract_from_counter`).
public struct Marker has key, store {
    id: UID,
}

/// Dynamic-object-field key under which a `Marker` is stored on a `Counter`.
public struct MarkerKey has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Creates a new `AbstractAccount`  as a shared object with the given authenticator.
///
/// `authenticator` is expected to have a signature like the following:
///
/// public fun authenticate(self: &AbstractAccount, signature: vector<u8>, _: &AuthContext, _: &TxContext) { ... }
///
/// to allow to verify the `signature` parameter against the public key stored in the account.
///
/// There are several ready-made authenticators available in this module:
/// - `authenticate_ed25519`
/// - `authenticate_secp256k1`
/// - `authenticate_secp256r1`
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
    ctx: &mut TxContext,
) {
    abstract_account::builder(authenticator, ctx)
        .add_dynamic_field(basic_keyed_aa::owner_public_key(), public_key)
        .build();
}

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    account: &mut AbstractAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
    ctx: &TxContext,
) {
    // Update the account owner public key dynamic field. It is expected that the field already exists.
    account.replace_field(basic_keyed_aa::owner_public_key(), public_key, ctx);

    // Update the account authenticator dynamic field. It is expected that the field already exists.
    account.rotate_auth_function_ref_v1(authenticator, ctx);
}

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519(
    account: &AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Secp256k1 signature authenticator.
#[authenticator]
public fun authenticate_secp256k1(
    account: &AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    basic_keyed_aa::authenticate_secp256k1(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Secp256r1 signature authenticator.
#[authenticator]
public fun authenticate_secp256r1(
    account: &AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    basic_keyed_aa::authenticate_secp256r1(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Ed25519 signature authenticator that uses `auth_ctx.signing_digest()`
/// to verify the signature, and checks the structural invariants of the
/// new AuthContext byte fields (tx_data_bytes, intent_tx_data_bytes,
/// signing_digest).
#[authenticator]
public fun authenticate_ed25519_via_signing_digest(
    account: &AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    basic_keyed_aa::authenticate_ed25519_via_signing_digest(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Free access, do nothing.
#[authenticator]
public fun authenticate_free_access(_: &AbstractAccount, _: &AuthContext, _: &TxContext) {}

/// An authenticator that checks both the sender and sponsor of the transaction against the provided accounts.
#[authenticator]
public fun authenticate_with_sponsor_and_sender(
    sponsor: &AbstractAccount,
    sender: &AbstractAccount,
    _: &AuthContext,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == sender.account_address());
    assert!(ctx.sponsor().borrow() == sponsor.account_address());
}

/// Create and share a `Counter` initialised to zero.
public fun create_counter(ctx: &mut TxContext) {
    transfer::share_object(Counter { id: object::new(ctx), value: 0 });
}

/// Create and share a `Counter` that holds a `Marker` as a dynamic object
/// field, used by the `authenticate_ed25519_and_extract_from_counter` test.
public fun create_counter_with_marker(ctx: &mut TxContext) {
    let mut counter = Counter { id: object::new(ctx), value: 0 };
    dynamic_object_field::add(&mut counter.id, MarkerKey {}, Marker { id: object::new(ctx) });
    transfer::share_object(counter);
}

/// Ed25519 authenticator that *mutates* a shared `Counter` while authenticating.
///
/// This requires the `enable_mutable_shared_in_move_authenticator` protocol
/// feature flag to be enabled, both to publish (the verifier must accept the
/// `&mut Counter` parameter) and to execute (the mutable shared object is
/// allowed as an authenticator input).
#[authenticator]
public fun authenticate_ed25519_and_increment(
    account: &AbstractAccount,
    counter: &mut Counter,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Mutate the shared object as part of authentication.
    counter.value = counter.value + 1;

    // Verify the ed25519 signature against the account public key.
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Ed25519 authenticator that *mutates the authenticated account itself*: it
/// initialises or bumps an `AuthCount` dynamic field on the account.
///
/// Requires `enable_mutable_shared_in_move_authenticator`: the account must be
/// passed by mutable reference (`&mut AbstractAccount`) and as a mutable shared
/// authenticator input.
#[authenticator]
public fun authenticate_ed25519_and_mutate_account(
    account: &mut AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Verify the signature first (immutable borrow, released before mutation).
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );

    // Mutate the account itself: initialise or bump the auth counter.
    if (account.has_field(AuthCount {})) {
        let count: &mut u64 = account.borrow_field_mut(AuthCount {}, ctx);
        *count = *count + 1;
    } else {
        account.add_field(AuthCount {}, 1u64, ctx);
    };
}

/// Ed25519 authenticator that *rotates the account's own
/// `AuthenticatorFunctionRef`* to `authenticate_free_access` while
/// authenticating.
///
/// Requires `enable_mutable_shared_in_move_authenticator` (the account is taken
/// by mutable reference). After a transaction authenticated with this function,
/// the account is authenticated by `authenticate_free_access`. The package
/// metadata is passed as an immutable input so the new authenticator reference
/// can be constructed.
#[authenticator]
public fun authenticate_ed25519_and_rotate_to_free_access(
    account: &mut AbstractAccount,
    package_metadata: &PackageMetadataV1,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Verify the signature first (immutable borrow, released before mutation).
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );

    // Build a reference to the free-access authenticator and rotate to it.
    let new_ref = authenticator_function::create_auth_function_ref_v1<AbstractAccount>(
        package_metadata,
        ascii::string(b"abstract_account_keyed"),
        ascii::string(b"authenticate_free_access"),
    );
    let _prev: AuthenticatorFunctionRefV1<AbstractAccount> = account.rotate_auth_function_ref_v1(
        new_ref,
        ctx,
    );
}

/// Ed25519 authenticator that attempts to *delete an object* by removing a
/// dynamic field from the account. Object deletion during authenticator
/// execution is forbidden, so any transaction using this authenticator must be
/// rejected.
#[authenticator]
public fun authenticate_ed25519_and_delete_field(
    account: &mut AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );

    // Removing a dynamic field deletes its child object.
    let _: vector<u8> = account.remove_field(basic_keyed_aa::owner_public_key(), ctx);
}

/// Ed25519 authenticator that *creates a fresh top-level object* (a new UID via
/// `object::new`) and transfers it to the account. This requires a
/// `&mut TxContext`, which is only accepted when
/// `enable_mutable_shared_in_move_authenticator` is enabled.
#[authenticator]
public fun authenticate_ed25519_and_create_object(
    account: &AbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &mut TxContext,
) {
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );

    // Mint a brand-new object and transfer it to the account.
    transfer::transfer(Marker { id: object::new(ctx) }, account.account_address());
}

/// Ed25519 authenticator that *removes an object* — it extracts the `Marker`
/// stored as a dynamic object field on a mutable shared `Counter` and transfers
/// it to the account. Removing the dynamic object field deletes the internal
/// wrapper object, so this requires
/// `enable_mutable_shared_in_move_authenticator` (which permits deletion).
#[authenticator]
public fun authenticate_ed25519_and_extract_from_counter(
    account: &AbstractAccount,
    counter: &mut Counter,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );

    // Extract the stored object and transfer it to the account.
    let marker: Marker = dynamic_object_field::remove(&mut counter.id, MarkerKey {});
    transfer::public_transfer(marker, account.account_address());
}

// === View Functions ===

/// Read the current value of a `Counter`.
public fun counter_value(counter: &Counter): u64 {
    counter.value
}

/// An utility function to borrow the account-related public key.
public fun borrow_public_key(account: &AbstractAccount): &vector<u8> {
    account.borrow_field(basic_keyed_aa::owner_public_key())
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
