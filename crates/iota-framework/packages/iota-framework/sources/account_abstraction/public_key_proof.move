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
/// The message names the account and the address of the public key attached to it now
/// (`public_key::to_iota_address`), or `none` if it has none, so a proof works only on that
/// account and only while that key is attached:
///
/// ```
/// 0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey
/// public key: <hex of flag || raw bytes>
/// account: 0x<hex>
/// current key address: 0x<hex>, or none
/// valid until epoch: <decimal>
/// ```
///
/// A proof works again if the account gets the same key back before the proof expires, for
/// example after rotating away and back. Only code that can change the account's key can use it
/// then, and it still attaches only the key whose holder signed.
///
/// # New accounts
/// A new account's address can't be known when the key signs, since it comes from the digest of
/// the transaction that carries the signature. The message names an `AccountFactory` instead,
/// the sender of the creating transaction, a nonce key of the factory and that nonce key's next
/// sequence number, which the creation consumes. Naming the sender keeps anyone else from using
/// the proof:
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
const EFactoryIsShared: vector<u8> =
    b"A shared factory can't be destroyed and its sequence numbers can't be skipped.";
#[error(code = 4)]
const EInvalidNonceKeyCount: vector<u8> = b"A shared factory has from 1 to 1024 nonce keys.";
#[error(code = 5)]
const ENonceKeyOutOfRange: vector<u8> = b"The factory has no such nonce key.";

// === Constants ===

const MAX_SHARED_FACTORY_NONCE_KEYS: u64 = 1024;

// === Structs ===

/// Proof that the holder of `public_key` approved attaching it.
///
/// `account` is the account the proof was made for, with the address of the key it had attached
/// then, or `none` for a new account. It has no abilities: it must be consumed by
/// `builtin_authenticator_functions` in the same transaction.
public struct VerifiedPublicKey {
    public_key: PublicKey,
    account: Option<address>,
    current_key_address: Option<address>,
}

/// Makes proofs for new accounts single-use.
///
/// Holds the next sequence number of each of its nonce keys, and a creation consumes the one its
/// proof names. Their number is fixed when the factory is created, so the factory doesn't grow
/// with use.
///
/// An owned factory has one nonce key, `0`, is used only by its owner and never waits for
/// consensus. A shared factory can be used by anyone, and every transaction that uses it is
/// ordered by consensus. Its senders share the sequence numbers, so a proof stops working if
/// another sender creates an account under the same nonce key first, and has to be signed again.
public struct AccountFactory has key {
    id: UID,
    shared: bool,
    sequences: vector<u64>,
}

// === Public Functions ===

/// Creates an `AccountFactory` owned by the sender, with one nonce key.
public fun new_owned_factory(ctx: &mut TxContext) {
    let factory = AccountFactory { id: object::new(ctx), shared: false, sequences: vector[0] };
    transfer::transfer(factory, ctx.sender());
}

/// Creates a shared `AccountFactory` with `nonce_keys` nonce keys. More nonce keys let more
/// proofs stay open at the same time, and make every transaction that uses the factory write a
/// larger object.
///
/// Aborts if `nonce_keys` is 0 or more than 1024.
public fun new_shared_factory(nonce_keys: u64, ctx: &mut TxContext) {
    assert!(
        nonce_keys > 0 && nonce_keys <= MAX_SHARED_FACTORY_NONCE_KEYS,
        EInvalidNonceKeyCount,
    );
    transfer::share_object(AccountFactory {
        id: object::new(ctx),
        shared: true,
        sequences: vector::tabulate!(nonce_keys, |_| 0),
    });
}

/// Destroys an owned `AccountFactory`. Proofs that name it can no longer be used.
///
/// Aborts if the factory is shared.
public fun destroy_owned_factory(factory: AccountFactory) {
    assert!(!factory.shared, EFactoryIsShared);

    let AccountFactory { id, shared: _, sequences: _ } = factory;
    id.delete();
}

/// Moves the sequence number of `nonce_key` one step on, so the proof made for the current one
/// can no longer be used.
///
/// Aborts if the factory is shared, or has no such nonce key.
public fun skip_sequence(factory: &mut AccountFactory, nonce_key: u64) {
    assert!(!factory.shared, EFactoryIsShared);
    factory.consume_sequence(nonce_key);
}

/// Checks that the holder of `public_key` signed the proof message for `account` while the key
/// with address `current_key_address` is attached to it (`none` if no key is), valid until
/// `valid_until_epoch`.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if `signature` is not a valid signature of the message by `public_key`.
public fun verify_for_account(
    public_key: PublicKey,
    account: address,
    current_key_address: Option<address>,
    valid_until_epoch: u64,
    signature: vector<u8>,
    ctx: &TxContext,
): VerifiedPublicKey {
    let message = account_proof_message(
        &public_key,
        account,
        current_key_address,
        valid_until_epoch,
    );
    verify_signature(&public_key, &message, &signature, valid_until_epoch, ctx);

    VerifiedPublicKey { public_key, account: option::some(account), current_key_address }
}

/// Checks that the holder of `public_key` signed the proof message for a new account created by
/// the transaction sender under `factory` and `nonce_key`, at that nonce key's next sequence
/// number, valid until `valid_until_epoch`, then consumes that sequence number.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the factory has no such nonce key.
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
    let message = creation_proof_message(
        &public_key,
        object::id(factory),
        ctx.sender(),
        nonce_key,
        factory.next_sequence(nonce_key),
        valid_until_epoch,
    );
    verify_signature(&public_key, &message, &signature, valid_until_epoch, ctx);
    factory.consume_sequence(nonce_key);

    VerifiedPublicKey {
        public_key,
        account: option::none(),
        current_key_address: option::none(),
    }
}

/// Returns the message the key holder signs to attach `public_key` to the existing `account`
/// while the key with address `current_key_address` is attached to it.
public fun account_proof_message(
    public_key: &PublicKey,
    account: address,
    current_key_address: Option<address>,
    valid_until_epoch: u64,
): vector<u8> {
    let mut message = message_head(public_key);
    message.append(b"\naccount: 0x");
    message.append(address::to_string(account).into_bytes());
    message.append(b"\ncurrent key address: ");
    if (current_key_address.is_some()) {
        message.append(b"0x");
        message.append(address::to_string(current_key_address.destroy_some()).into_bytes());
    } else {
        message.append(b"none");
    };
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

/// Returns the number of nonce keys of the factory. They are numbered from 0.
public fun nonce_keys(self: &AccountFactory): u64 {
    self.sequences.length()
}

/// Returns the sequence number the next proof under `nonce_key` must name.
///
/// Aborts if the factory has no such nonce key.
public fun next_sequence(self: &AccountFactory, nonce_key: u64): u64 {
    assert!(nonce_key < self.sequences.length(), ENonceKeyOutOfRange);
    self.sequences[nonce_key]
}

/// Borrows the verified public key.
public fun public_key(self: &VerifiedPublicKey): &PublicKey {
    &self.public_key
}

// === Package Functions ===

/// Returns a `VerifiedPublicKey` for a new account claimed through `ClaimAccount`, where
/// `claim::claim_address` has already proved that the sender holds `public_key`.
public(package) fun verified_by_claim(public_key: PublicKey): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::none(), current_key_address: option::none() }
}

/// Unpacks the proof into its public key, the account it was made for (`none` for a new
/// account) and the address of the key that account had attached then.
public(package) fun unpack(self: VerifiedPublicKey): (PublicKey, Option<address>, Option<address>) {
    let VerifiedPublicKey { public_key, account, current_key_address } = self;
    (public_key, account, current_key_address)
}

// === Private Functions ===

/// Moves the sequence number of `nonce_key` one step on.
fun consume_sequence(self: &mut AccountFactory, nonce_key: u64) {
    assert!(nonce_key < self.sequences.length(), ENonceKeyOutOfRange);
    let sequence = &mut self.sequences[nonce_key];
    *sequence = *sequence + 1;
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

/// A proof for `account` while the key with address `current_key_address` is attached to it,
/// without a signature.
#[test_only]
public fun verified_for_account_for_testing(
    public_key: PublicKey,
    account: address,
    current_key_address: Option<address>,
): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::some(account), current_key_address }
}

/// A proof for a new account, without a signature.
#[test_only]
public fun verified_for_new_account_for_testing(public_key: PublicKey): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::none(), current_key_address: option::none() }
}
