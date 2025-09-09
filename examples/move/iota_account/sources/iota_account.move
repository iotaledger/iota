module iota_account::iota_account;

use iota::account;
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use std::ascii::{Self, String};

/// A dynamic field name for the account owner public key.
const IOTACCOUNT_OWNER_PUBKEY: vector<u8> = b"IOTACCOUNT_OWNER_PUBKEY";
/// A constant contains the `iota_account` module name.
const IOTACCOUNT_MODULE_NAME: vector<u8> = b"iota_account";

/// This struct represents an IOTA account on-chain.
/// It holds all the related data as dynamic fields to simplify updates and migrations.
public struct IOTAccount has key {
    id: UID,
}

/// The signature schemes supported by the IOTA account.
public enum SignatureScheme {
    ED25519,
    Secp256k1,
    Secp256r1,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a new `IOTAccount`  as a shared object with the given public key and signature scheme.
/// `package_id` is a `Storage ID` of the `iota_account` package published on-chain.
public fun create(
    pubkey: vector<u8>,
    package_id: address,
    scheme: SignatureScheme,
    ctx: &mut TxContext
) {
    let mut account = IOTAccount { id: object::new(ctx) };

    // Check the flag in `pubkey` is the same as the input scheme.
    //assert!(check_scheme(pubkey, scheme));

    let account_id = &mut account.id;

    // Add the account owner public key as a dynamic field.
    dynamic_field::add(account_id, IOTACCOUNT_OWNER_PUBKEY, pubkey);

    // Create `AuthenticatorInfoV1` instance.
    let authenticator_info_v1 = account::create_auth_info_v1(
        package_id,
        ascii::string(IOTACCOUNT_MODULE_NAME),
        signature_scheme_to_authenticator_name(scheme));

    // Add the authenticator info as a dynamic field.
    dynamic_field::add(account_id, account::authenticator_df_name(), authenticator_info_v1);

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
    let id = &self.id;
    // Check that the sender of this TX is the account.
    assert!(id.uid_to_address() == ctx.sender());

    dynamic_field::borrow(&self.id, name)
}

public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    let id = &self.id;
    // Check that the sender of this TX is the account.
    assert!(id.uid_to_address() == ctx.sender());

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

// --------------------------------------- Utility ---------------------------------------

/// Returns the authenticator function name for the given signature scheme.
fun signature_scheme_to_authenticator_name(scheme: SignatureScheme): String {
    match (scheme) {
        SignatureScheme::ED25519 => ascii::string(b"authenticate_ed25519"),
        SignatureScheme::Secp256k1 => ascii::string(b"authenticate_secp256k1"),
        SignatureScheme::Secp256r1 => ascii::string(b"authenticate_secp256r1"),
    }
}
