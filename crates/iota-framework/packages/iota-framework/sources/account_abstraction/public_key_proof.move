// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Proof that the holder of a public key approved attaching it to an account.
///
/// The key holder signs a plain-text personal message, which wallets produce with
/// `signPersonalMessage`, and the functions here check that signature and return a
/// `VerifiedPublicKey`. `builtin_authenticator_functions` only attaches or rotates a key given a
/// `VerifiedPublicKey`, for every account type.
///
/// Every message names a nonce, a value that is accepted only once, and where it comes from.
///
/// # Existing accounts
/// The nonce is the next value of the account's counter for these proofs (`account_nonce`), which
/// `builtin_authenticator_functions` moves on when it attaches or rotates the key. The message names
/// the account, so a proof works only on that account:
///
/// ```
/// 0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey
/// public key: <hex of flag || raw bytes>
/// account: 0x<hex>
/// nonce source: account 0x<hex>
/// nonce: <decimal>
/// valid until epoch: <decimal>
/// ```
///
/// # New accounts
/// A new account's address can't be known when the key signs, since it comes from the digest of
/// the transaction that carries the signature. The message names the sender of the creating
/// transaction instead, and a nonce from a `NonceFactory` the sender owns, which the check takes:
///
/// ```
/// 0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey
/// public key: <hex of flag || raw bytes>
/// sender: 0x<hex>
/// nonce source: factory 0x<hex>
/// nonce key: <decimal>
/// nonce: <decimal>
/// valid until epoch: <decimal>
/// ```
///
/// Lines are separated by `\n`, hex is lowercase, and there is no trailing newline.
module iota::public_key_proof;

use iota::account_nonce;
use iota::address;
use iota::hex;
use iota::nonce_factory::NonceFactory;
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

// === Structs ===

/// Proof that the holder of `public_key` approved attaching it.
///
/// `account` is the account the proof was made for, with the nonce of its counter the proof
/// names, or `none` for a new account. It has no abilities: it must be consumed by
/// `builtin_authenticator_functions` in the same transaction.
public struct VerifiedPublicKey {
    public_key: PublicKey,
    account: Option<address>,
    account_nonce: Option<u64>,
}

// === Public Functions ===

/// Checks that the holder of `public_key` signed the proof message for `account` with the nonce
/// `nonce` of the account's counter, valid until `valid_until_epoch`.
///
/// The counter moves on when the proof attaches or rotates the key.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if `signature` is not a valid signature of the message by `public_key`.
public fun verify_for_account(
    public_key: PublicKey,
    account: address,
    nonce: u64,
    valid_until_epoch: u64,
    signature: vector<u8>,
    ctx: &TxContext,
): VerifiedPublicKey {
    check_builtin_authenticators_enabled();

    let message = account_proof_message(&public_key, account, nonce, valid_until_epoch);
    verify_signature(&public_key, &message, &signature, valid_until_epoch, ctx);

    VerifiedPublicKey {
        public_key,
        account: option::some(account),
        account_nonce: option::some(nonce),
    }
}

/// Checks that the holder of `public_key` signed the proof message for a new account created by
/// the transaction sender, with the nonce `nonce` under `nonce_key` of `factory`, valid until
/// `valid_until_epoch`, and takes that nonce.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
/// Aborts if `nonce` is not the next value of `nonce_key` in `factory`.
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if `signature` is not a valid signature of the message by `public_key`.
public fun verify_for_new_account(
    public_key: PublicKey,
    factory: &mut NonceFactory,
    nonce_key: u64,
    nonce: u64,
    valid_until_epoch: u64,
    signature: vector<u8>,
    ctx: &TxContext,
): VerifiedPublicKey {
    check_builtin_authenticators_enabled();

    factory.use_nonce(nonce_key, nonce);
    let message = creation_proof_message(
        &public_key,
        ctx.sender(),
        object::id(factory),
        nonce_key,
        nonce,
        valid_until_epoch,
    );
    verify_signature(&public_key, &message, &signature, valid_until_epoch, ctx);

    VerifiedPublicKey { public_key, account: option::none(), account_nonce: option::none() }
}

/// Returns the message the key holder signs to attach `public_key` to the existing `account`,
/// with the nonce `nonce` of the account's counter.
public fun account_proof_message(
    public_key: &PublicKey,
    account: address,
    nonce: u64,
    valid_until_epoch: u64,
): vector<u8> {
    let mut message = message_head(public_key);
    message.append(b"\naccount: 0x");
    message.append(address::to_string(account).into_bytes());
    message.append(b"\nnonce source: account 0x");
    message.append(address::to_string(account).into_bytes());
    message.append(b"\nnonce: ");
    message.append(nonce.to_string().into_bytes());
    message.append(b"\nvalid until epoch: ");
    message.append(valid_until_epoch.to_string().into_bytes());
    message
}

/// Returns the message the key holder signs to attach `public_key` to a new account created by
/// `sender`, with the nonce `nonce` under `nonce_key` of `factory`.
public fun creation_proof_message(
    public_key: &PublicKey,
    sender: address,
    factory: ID,
    nonce_key: u64,
    nonce: u64,
    valid_until_epoch: u64,
): vector<u8> {
    let mut message = message_head(public_key);
    message.append(b"\nsender: 0x");
    message.append(address::to_string(sender).into_bytes());
    message.append(b"\nnonce source: factory 0x");
    message.append(address::to_string(object::id_to_address(&factory)).into_bytes());
    message.append(b"\nnonce key: ");
    message.append(nonce_key.to_string().into_bytes());
    message.append(b"\nnonce: ");
    message.append(nonce.to_string().into_bytes());
    message.append(b"\nvalid until epoch: ");
    message.append(valid_until_epoch.to_string().into_bytes());
    message
}

// === View Functions ===

/// Returns the nonce the next proof for the account with `account_id` must name.
public fun next_account_nonce(account_id: &UID): u64 {
    account_nonce::next_value<VerifiedPublicKey>(account_id)
}

/// Borrows the verified public key.
public fun public_key(self: &VerifiedPublicKey): &PublicKey {
    &self.public_key
}

// === Package Functions ===

/// Returns a `VerifiedPublicKey` for a new account claimed through `ClaimAccount`, where
/// `claim::claim_address` has already proved that the sender holds `public_key`.
public(package) fun verified_by_claim(public_key: PublicKey): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::none(), account_nonce: option::none() }
}

/// Unpacks the proof into its public key, the account it was made for (`none` for a new
/// account) and the nonce of that account's counter it names.
public(package) fun unpack(self: VerifiedPublicKey): (PublicKey, Option<address>, Option<u64>) {
    let VerifiedPublicKey { public_key, account, account_nonce } = self;
    (public_key, account, account_nonce)
}

// === Private Functions ===

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

fun check_builtin_authenticators_enabled() {
    assert!(
        protocol_config::is_feature_enabled(b"enable_builtin_move_authenticators"),
        EBuiltinAuthenticatorsNotEnabled,
    );
}

fun verify_signature(
    public_key: &PublicKey,
    message: &vector<u8>,
    signature: &vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
) {
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

/// A proof for `account` with the nonce `nonce` of its counter, without a signature.
#[test_only]
public fun verified_for_account_for_testing(
    public_key: PublicKey,
    account: address,
    nonce: u64,
): VerifiedPublicKey {
    VerifiedPublicKey {
        public_key,
        account: option::some(account),
        account_nonce: option::some(nonce),
    }
}

/// A proof for a new account, without a signature.
#[test_only]
public fun verified_for_new_account_for_testing(public_key: PublicKey): VerifiedPublicKey {
    VerifiedPublicKey { public_key, account: option::none(), account_nonce: option::none() }
}
