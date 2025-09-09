module iota_account2::iota_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;

/// A dynamic field name for the account owner public key.
const IOTACCOUNT_OWNER_PUBKEY: vector<u8> = b"IOTACCOUNT_OWNER_PUBKEY";

/// This struct represents an IOTA account on-chain.
/// It holds all the related data as dynamic fields to simplify updates and migrations.
public struct IOTAccount has key {
    id: UID,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a new `IOTAccount`  as a shared object with the given authenticator.
/// 
/// `authenticator` is expect to have the following signature:
///
/// public fun authenticate(self: &IOTAccount, signature: vector<u8>, _: &AuthContext, _: &TxContext) { ... }
/// 
/// And it is expected to verify the `signature` against the public key stored in the account.
/// 
/// There are several ready-made authenticators available in this module:
/// - `authenticate_ed25519`
/// - `authenticate_secp256k1`
/// - `authenticate_secp256r1`
public fun create(pubkey: vector<u8>, authenticator: AuthenticatorInfoV1, ctx: &mut TxContext) {
    // Create an account object.
    let mut account = IOTAccount { id: object::new(ctx) };

    let account_id = &mut account.id;

    // Add the account owner public key as a dynamic field.
    dynamic_field::add(account_id, IOTACCOUNT_OWNER_PUBKEY, pubkey);

    // Add the authenticator info as a dynamic field.
    dynamic_field::add(account_id, account::authenticator_df_name(), authenticator);

    // Turn the account object into a mutable shared object.
    iota::transfer::share_object(account);
}

// --------------------------------------- Field Operations ---------------------------------------

/// Adds a new dynamic field to the account.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this TX is the account.
    assert!(self.id.uid_to_address() == ctx.sender());

    // Add a new field.
    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this TX is the account.
    assert!(self.id.uid_to_address() == ctx.sender());

    // Remove a new field.
    dynamic_field::remove(&mut self.id, name)
}

public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &IOTAccount,
    name: Name,
    ctx: &TxContext,
): &Value {
    // Check that the sender of this TX is the account.
    assert!(self.id.uid_to_address() == ctx.sender());

    dynamic_field::borrow(&self.id, name)
}

public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this TX is the account.
    assert!(self.id.uid_to_address() == ctx.sender());

    dynamic_field::borrow_mut(&mut self.id, name)
}

// --------------------------------------- Authentication ---------------------------------------

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let pubkey: &vector<u8> = borrow_field(self, IOTACCOUNT_OWNER_PUBKEY, ctx);
    assert!(ed25519::ed25519_verify(&signature, pubkey, ctx.digest()));
}

/// Secp256k1 signature authenticator.
public fun authenticate_secp256k1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let pubkey: &vector<u8> = borrow_field(self, IOTACCOUNT_OWNER_PUBKEY, ctx);
    assert!(ecdsa_k1::secp256k1_verify(&signature, pubkey, ctx.digest(), 0));
}

/// Secp256r1 signature authenticator.
public fun authenticate_secp256r1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let pubkey: &vector<u8> = borrow_field(self, IOTACCOUNT_OWNER_PUBKEY, ctx);
    assert!(ecdsa_r1::secp256r1_verify(&signature, pubkey, ctx.digest(), 0));
}
