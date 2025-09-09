module iota_account::iota_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;

/// A dynamic field name for the account owner public key.
const IOTACCOUNT_OWNER_PUBLIC_KEY_DF_NAME: vector<u8> = b"IOTACCOUNT_OWNER_PUBLIC_KEY";

/// This struct represents an IOTA account on-chain.
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
public struct IOTAccount has key {
    id: UID,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a new `IOTAccount`  as a shared object with the given authenticator.
/// 
/// `authenticator` is expected to have a signature like the following:
///
/// public fun authenticate(self: &IOTAccount, signature: vector<u8>, _: &AuthContext, _: &TxContext) { ... }
/// 
/// to allow to verify the `signature` parameter against the public key stored in the account.
/// 
/// There are several ready-made authenticators available in this module:
/// - `authenticate_ed25519`
/// - `authenticate_secp256k1`
/// - `authenticate_secp256r1`
public fun create(public_key: vector<u8>, authenticator: AuthenticatorInfoV1, ctx: &mut TxContext) {
    // Create an account object.
    let mut account = IOTAccount { id: object::new(ctx) };

    let account_id = &mut account.id;

    // Add the account owner public key as a dynamic field.
    dynamic_field::add(account_id, IOTACCOUNT_OWNER_PUBLIC_KEY_DF_NAME, public_key);

    // Add the authenticator info as a dynamic field.
    dynamic_field::add(account_id, account::authenticator_df_name(), authenticator);

    // Turn the account object into a mutable shared object.
    iota::transfer::share_object(account);
}

// --------------------------------------- Field Operations ---------------------------------------

/// Adds a new dynamic field to the account.
/// Only the account itself can call this function.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Add a new field.
    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
/// Only the account itself can call this function.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Remove a new field and return it.
    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a reference to a dynamic field from the account.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &IOTAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
/// Only the account itself can call this function.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

// --------------------------------------- Authentication ---------------------------------------

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    self: &mut IOTAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &TxContext
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    let account_id = &mut self.id;

    // Update the account owner public key dynamic field. It is expected that the field already exists.
    dynamic_field::remove<_, vector<u8>>(account_id, IOTACCOUNT_OWNER_PUBLIC_KEY_DF_NAME);
    dynamic_field::add(account_id, IOTACCOUNT_OWNER_PUBLIC_KEY_DF_NAME, public_key);

    // Update the account owner public key dynamic field. It is expected that the field already exists.
    let authenticator_df_name = account::authenticator_df_name();

    let prev_authenticator = dynamic_field::remove(account_id, authenticator_df_name);
    account::drop_auth_info_v1(prev_authenticator);

    dynamic_field::add(account_id, authenticator_df_name, authenticator);
}

// --------------------------------------- Authenticators ---------------------------------------

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(ed25519::ed25519_verify(&signature, self.borrow_public_key(), ctx.digest()));
}

/// Secp256k1 signature authenticator.
public fun authenticate_secp256k1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(ecdsa_k1::secp256k1_verify(&signature, self.borrow_public_key(), ctx.digest(), 0));
}

/// Secp256r1 signature authenticator.
public fun authenticate_secp256r1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(ecdsa_r1::secp256r1_verify(&signature, self.borrow_public_key(), ctx.digest(), 0));
}

// --------------------------------------- Utilities ---------------------------------------

/// An utility function to borrow the account-related public key.
fun borrow_public_key(self: &IOTAccount): &vector<u8> {
    dynamic_field::borrow(&self.id, IOTACCOUNT_OWNER_PUBLIC_KEY_DF_NAME)
}

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender());
}
