// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Proof that the holder of a public key approved attaching it to an account.
///
/// The key holder signs a plain-text personal message, which wallets produce with
/// `signPersonalMessage`, and the functions here check that signature and return a
/// `VerifiedPublicKey`. `builtin_authenticator_functions` only attaches or rotates a key given a
/// `VerifiedPublicKey`, for every account type.
///
/// # Existing accounts
/// The message names the account and its key change count, which goes up on every attach,
/// rotate and detach, so a proof works on one account and only once:
///
/// ```
/// 0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey
/// public key: <hex of flag || raw bytes>
/// account: 0x<hex>
/// key changes on this account: <decimal>
/// valid until epoch: <decimal>
/// ```
///
/// # New accounts
/// A new account's address can't be known when the key signs, since it comes from the digest of
/// the transaction that carries the signature. The message names an `AccountFactory` instead,
/// the sender of the creating transaction, one of that sender's nonce keys and its next sequence
/// number, which the creation consumes. Naming the sender keeps anyone else from using the proof
/// through a shared factory, and each sender's sequence numbers are its own, so nobody else can
/// move them:
///
/// ```
/// 0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey
/// public key: <hex of flag || raw bytes>
/// factory: 0x<hex>
/// sender: 0x<hex>
/// nonce key: <decimal>
/// sequence: <decimal>
/// valid until epoch: <decimal>
/// ```
///
/// Lines are separated by `\n`, hex is lowercase, and there is no trailing newline.
module iota::public_key_proof;

use iota::address;
use iota::dynamic_field;
use iota::hex;
use iota::protocol_config;
use iota::public_key::PublicKey;
use std::type_name;

// === Errors ===

#[error(code = 0)]
const EBuiltinAuthenticatorsNotEnabled: vector<u8> = b"Built-in Move authenticators not enabled.";
#[error(code = 1)]
const EInvalidProofSignature: vector<u8> =
    b"The signature is not a valid signature of the proof message by the public key.";
#[error(code = 2)]
const EProofExpired: vector<u8> = b"The proof is valid only until an earlier epoch.";
#[error(code = 3)]
const EFactoryIsShared: vector<u8> = b"A shared factory can't be destroyed.";

// === Structs ===

/// Proof that the holder of `public_key` approved attaching it.
///
/// `account` is the account the proof was made for, with its key change count at that time, or
/// `none` for a new account. It has no abilities: it must be consumed by
/// `builtin_authenticator_functions` in the same transaction.
public struct VerifiedPublicKey {
    public_key: PublicKey,
    account: Option<address>,
    key_change_count: u64,
}

/// Makes proofs for new accounts single-use.
///
/// Every sender has its own sequence numbers here, one per nonce key it picks, and a creation
/// consumes the one its proof names. Nonce keys only let a sender keep several proofs open that
/// don't depend on each other's order.
///
/// An owned factory is used only by its owner and never waits for consensus. A shared factory
/// can be used by anyone, but every transaction that uses it is ordered by consensus.
public struct AccountFactory has key {
    id: UID,
    shared: bool,
}

/// Dynamic field key of the next sequence number of `sender` under `nonce_key`.
public struct NonceKey has copy, drop, store {
    sender: address,
    nonce_key: u64,
}

// === Public Functions ===

/// Creates an `AccountFactory` owned by the sender.
public fun new_owned_factory(ctx: &mut TxContext) {
    let factory = AccountFactory { id: object::new(ctx), shared: false };
    transfer::transfer(factory, ctx.sender());
}

/// Creates a shared `AccountFactory`.
public fun new_shared_factory(ctx: &mut TxContext) {
    transfer::share_object(AccountFactory { id: object::new(ctx), shared: true });
}

/// Destroys an owned `AccountFactory`, after removing the sequence numbers of its owner under
/// `nonce_keys`. Sequence numbers under other nonce keys stay in storage, unreachable.
///
/// Proofs that name the factory can no longer be used.
///
/// Aborts if the factory is shared.
public fun destroy_owned_factory(factory: AccountFactory, nonce_keys: vector<u64>, ctx: &TxContext) {
    assert!(!factory.shared, EFactoryIsShared);

    let AccountFactory { mut id, shared: _ } = factory;
    nonce_keys.do!(|nonce_key| {
        let name = NonceKey { sender: ctx.sender(), nonce_key };
        if (dynamic_field::exists_(&id, name)) {
            dynamic_field::remove<NonceKey, u64>(&mut id, name);
        }
    });
    id.delete();
}

/// Moves the sender's sequence number under `nonce_key` one step on, so the proof made for the
/// current one can no longer be used. Other senders' sequence numbers don't change.
public fun skip_sequence(factory: &mut AccountFactory, nonce_key: u64, ctx: &TxContext) {
    factory.consume_sequence(ctx.sender(), nonce_key);
}

/// Checks that the holder of `public_key` signed the proof message for `account` at
/// `key_change_count`, valid until `valid_until_epoch`.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if `signature` is not a valid signature of the message by `public_key`.
public fun verify_for_account(
    public_key: PublicKey,
    account: address,
    key_change_count: u64,
    valid_until_epoch: u64,
    signature: vector<u8>,
    ctx: &TxContext,
): VerifiedPublicKey {
    let message = account_proof_message(&public_key, account, key_change_count, valid_until_epoch);
    verify_signature(&public_key, &message, &signature, valid_until_epoch, ctx);

    VerifiedPublicKey { public_key, account: option::some(account), key_change_count }
}

/// Checks that the holder of `public_key` signed the proof message for a new account created by
/// the transaction sender under `factory` and `nonce_key`, at the sender's next sequence number
/// there, valid until `valid_until_epoch`, then consumes that sequence number.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if `signature` is not a valid signature of the message by `public_key`.
public fun verify_for_new_account(
    public_key: PublicKey,
    factory: &mut AccountFactory,
    nonce_key: u64,
    valid_until_epoch: u64,
    signature: vector<u8>,
    ctx: &TxContext,
): VerifiedPublicKey {
    let sender = ctx.sender();
    let sequence = factory.next_sequence(sender, nonce_key);
    let message = creation_proof_message(
        &public_key,
        object::id(factory),
        sender,
        nonce_key,
        sequence,
        valid_until_epoch,
    );
    verify_signature(&public_key, &message, &signature, valid_until_epoch, ctx);
    factory.consume_sequence(sender, nonce_key);

    VerifiedPublicKey { public_key, account: option::none(), key_change_count: 0 }
}

/// Returns the message the key holder signs to attach `public_key` to the existing `account`
/// at `key_change_count`.
public fun account_proof_message(
    public_key: &PublicKey,
    account: address,
    key_change_count: u64,
    valid_until_epoch: u64,
): vector<u8> {
    let mut message = message_head(public_key);
    message.append(b"\naccount: 0x");
    message.append(address::to_string(account).into_bytes());
    message.append(b"\nkey changes on this account: ");
    message.append(key_change_count.to_string().into_bytes());
    message.append(b"\nvalid until epoch: ");
    message.append(valid_until_epoch.to_string().into_bytes());
    message
}

/// Returns the message the key holder signs to attach `public_key` to a new account created by
/// `sender` under `factory`, `nonce_key` and `sequence`.
public fun creation_proof_message(
    public_key: &PublicKey,
    factory: ID,
    sender: address,
    nonce_key: u64,
    sequence: u64,
    valid_until_epoch: u64,
): vector<u8> {
    let mut message = message_head(public_key);
    message.append(b"\nfactory: 0x");
    message.append(address::to_string(object::id_to_address(&factory)).into_bytes());
    message.append(b"\nsender: 0x");
    message.append(address::to_string(sender).into_bytes());
    message.append(b"\nnonce key: ");
    message.append(nonce_key.to_string().into_bytes());
    message.append(b"\nsequence: ");
    message.append(sequence.to_string().into_bytes());
    message.append(b"\nvalid until epoch: ");
    message.append(valid_until_epoch.to_string().into_bytes());
    message
}

// === View Functions ===

/// Returns `true` if and only if the factory is shared.
public fun is_shared(self: &AccountFactory): bool {
    self.shared
}

/// Returns the sequence number the next proof of `sender` under `nonce_key` must name.
public fun next_sequence(self: &AccountFactory, sender: address, nonce_key: u64): u64 {
    let name = NonceKey { sender, nonce_key };
    if (dynamic_field::exists_(&self.id, name)) {
        *dynamic_field::borrow(&self.id, name)
    } else {
        0
    }
}

/// Borrows the verified public key.
public fun public_key(self: &VerifiedPublicKey): &PublicKey {
    &self.public_key
}

// === Package Functions ===

/// Returns a `VerifiedPublicKey` for a new account claimed through `ClaimAccount`, where
/// `claim::claim_address` has already proved that the sender holds `public_key`.
public(package) fun verified_by_claim(public_key: PublicKey): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::none(), key_change_count: 0 }
}

/// Unpacks the proof into its public key, the account it was made for (`none` for a new
/// account) and that account's key change count.
public(package) fun unpack(self: VerifiedPublicKey): (PublicKey, Option<address>, u64) {
    let VerifiedPublicKey { public_key, account, key_change_count } = self;
    (public_key, account, key_change_count)
}

// === Private Functions ===

/// Moves the sequence number of `sender` under `nonce_key` one step on.
fun consume_sequence(self: &mut AccountFactory, sender: address, nonce_key: u64) {
    let name = NonceKey { sender, nonce_key };
    if (dynamic_field::exists_(&self.id, name)) {
        let sequence: &mut u64 = dynamic_field::borrow_mut(&mut self.id, name);
        *sequence = *sequence + 1;
    } else {
        dynamic_field::add(&mut self.id, name, 1u64);
    }
}

/// Returns the first two lines of a proof message: the type name of `PublicKey`, which keeps
/// these messages apart from any other signed text, and the key itself.
fun message_head(public_key: &PublicKey): vector<u8> {
    let mut message = type_name::get_with_original_ids<PublicKey>().into_string().into_bytes();
    message.append(b"\npublic key: ");

    let scheme = public_key.scheme();
    let mut prefixed_key = vector[scheme.flag()];
    prefixed_key.append(*public_key.raw_bytes());
    message.append(hex::encode(prefixed_key));
    message
}

fun verify_signature(
    public_key: &PublicKey,
    message: &vector<u8>,
    signature: &vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
) {
    assert!(
        protocol_config::is_feature_enabled(b"enable_builtin_move_authenticators"),
        EBuiltinAuthenticatorsNotEnabled,
    );
    assert!(ctx.epoch() <= valid_until_epoch, EProofExpired);
    assert!(verify_personal_message(public_key, message, signature), EInvalidProofSignature);
}

// === Native Functions ===

/// Returns true if `signature` is a valid signature, by `public_key`, of the personal message
/// `message`, using the signature scheme of `public_key`.
native fun verify_personal_message(
    public_key: &PublicKey,
    message: &vector<u8>,
    signature: &vector<u8>,
): bool;

// === Test Functions ===

/// A proof for `account` at `key_change_count`, without a signature.
#[test_only]
public fun verified_for_account_for_testing(
    public_key: PublicKey,
    account: address,
    key_change_count: u64,
): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::some(account), key_change_count }
}

/// A proof for a new account, without a signature.
#[test_only]
public fun verified_for_new_account_for_testing(public_key: PublicKey): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::none(), key_change_count: 0 }
}
