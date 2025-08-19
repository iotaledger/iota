module iota_account::iota_account;

use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use std::ascii::{Self, String};

const IOTACCOUNT_OWNER: vector<u8> = b"IOTACCOUNT_OWNER";
const IOTA_AUTHENTICATION: vector<u8> = b"IOTA_AUTHENTICATION";

public struct IOTAccount has key {
    id: UID,
}

public enum SignatureScheme {
    ED25519,
    Secp256k1,
    Secp256r1,
}

fun select_authenticate(scheme: SignatureScheme): String {
    match (scheme) {
        SignatureScheme::ED25519 => ascii::string(b"authenticate_ed25519"),
        SignatureScheme::Secp256k1 => ascii::string(b"authenticate_secp256k1"),
        SignatureScheme::Secp256r1 => ascii::string(b"authenticate_secp256r1"),
    }
}

// --------------------------------------- Creation ---------------------------------------

public fun create(pubkey: vector<u8>, scheme: SignatureScheme, ctx: &mut TxContext) {
    let mut iota_account = IOTAccount { id: object::new(ctx) };

    // check the flag in pubkey is the same as the input scheme
    //assert!(check_scheme(pubkey, scheme));

    // add the owner field
    dynamic_field::add(&mut iota_account.id, IOTACCOUNT_OWNER, pubkey);

    //TODO: turn it on
    let auth_info = account::create_auth_info_v1_self(select_authenticate(scheme));
    dynamic_field::add(&mut iota_account.id, IOTA_AUTHENTICATION, auth_info);

    iota::transfer::share_object(iota_account);
}

// --------------------------------------- Field Operation ---------------------------------------

public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    let id = &self.id;
    // Check that the sender of this TX is the account
    assert!(id.uid_to_address() == ctx.sender());

    // add a new field
    dynamic_field::add(&mut self.id, name, value);
}

public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
) {
    let id = &self.id;
    // Check that the sender of this TX is the account
    assert!(id.uid_to_address() == ctx.sender());

    // remove a new field
    let _value: Value = dynamic_field::remove(&mut self.id, name);
}

public fun modify_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): &mut Value {
    let id = &self.id;
    // Check that the sender of this TX is the account
    assert!(id.uid_to_address() == ctx.sender());

    let inner: &mut Value = dynamic_field::borrow_mut(&mut self.id, name);
    inner
}

// --------------------------------------- Authentication ---------------------------------------

public fun authenticate_ed25519(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let pk: &vector<u8> = dynamic_field::borrow(&self.id, IOTACCOUNT_OWNER);
    assert!(ed25519::ed25519_verify(&signature, pk, ctx.digest()));
}

public fun authenticate_secp256k1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let pk: &vector<u8> = dynamic_field::borrow(&self.id, IOTACCOUNT_OWNER);
    assert!(ecdsa_k1::secp256k1_verify(&signature, pk, ctx.digest(), 0));
}

public fun authenticate_secp256r1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    let pk: &vector<u8> = dynamic_field::borrow(&self.id, IOTACCOUNT_OWNER);
    assert!(ecdsa_r1::secp256r1_verify(&signature, pk, ctx.digest(), 0));
}
