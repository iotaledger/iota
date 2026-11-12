// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The message a key holder signs to approve an action, and the check of its signature.
///
/// The key holder signs readable lines as an IOTA personal message. The message is built in this
/// order, with lines joined by `\n` and no trailing newline:
///
/// ```
/// check: <full type name of Check, with original IDs>
/// network: <hex of the 32-byte chain identifier>
/// <lines written by the check>
/// nonce: <hex of the 32-byte nonce hash>
/// valid until epoch: <decimal>
/// ```
///
/// Hex is lowercase, without a `0x` prefix.
///
/// The `check:` line keeps a signature made for one check from being accepted by another, the
/// `network:` line from being accepted on another network, and the `nonce:` line from being
/// accepted twice.
module iota::approval_message;

use iota::bcs;
use iota::ed25519;
use iota::hash;
use iota::hex;
use iota::intent;
use iota::public_key::PublicKey;
use iota::signature_scheme;
use iota::verified_nonce::VerifiedNonce;
use std::type_name;

// === Errors ===

#[error(code = 0)]
const EApprovalExpired: vector<u8> = b"The approval has expired.";
#[error(code = 1)]
const EInvalidSignature: vector<u8> = b"Invalid approval signature.";

// === Constants ===

const NEWLINE: u8 = 10;

/// Length of an Ed25519 signature in wire format: the scheme flag, the signature and the key.
const ED25519_WIRE_SIGNATURE_LENGTH: u64 = 97;
const ED25519_SIGNATURE_LENGTH: u64 = 64;
const ED25519_PUBLIC_KEY_LENGTH: u64 = 32;

// === Package Functions ===

/// Checks that `signature` is a signature by `public_key` of the approval message for `Check`,
/// and destroys `nonce`.
///
/// `check_lines` are the lines the check writes about what it approves, joined by `\n`, with no
/// trailing newline. `signature` is in the wire format of `public_key`'s scheme (the scheme flag,
/// the signature and the key), over the message as an IOTA personal message.
///
/// The approval is valid up to and including the epoch `valid_until_epoch`.
///
/// Aborts if the current epoch is after `valid_until_epoch`.
/// Aborts if the signature is not valid.
public(package) fun verify_signature<Check>(
    nonce: VerifiedNonce<Check>,
    public_key: &PublicKey,
    check_lines: vector<u8>,
    signature: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
) {
    assert!(ctx.epoch() <= valid_until_epoch, EApprovalExpired);

    let message = message(nonce, check_lines, valid_until_epoch, ctx);
    assert!(
        verify_personal_message_signature(public_key, &signature, &message),
        EInvalidSignature,
    );
}

// === Private Functions ===

/// Returns the whole approval message for `Check` and destroys `nonce`.
fun message<Check>(
    nonce: VerifiedNonce<Check>,
    check_lines: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
): vector<u8> {
    let mut message = b"check: ";
    message.append(type_name::get_with_original_ids<Check>().into_string().into_bytes());
    message.append(b"\nnetwork: ");
    message.append(hex::encode(ctx.chain_identifier()));
    message.push_back(NEWLINE);
    message.append(check_lines);
    message.append(b"\nnonce: ");
    message.append(hex::encode(nonce.into_hash()));
    message.append(b"\nvalid until epoch: ");
    message.append(valid_until_epoch.to_string().into_bytes());
    message
}

/// Returns `true` if `signature` is a valid signature, by `public_key`, of `message` signed as an
/// IOTA personal message. `signature` is in the wire format of the key's scheme.
///
/// Temporary: the check is written in Move until a native function checks every scheme. It
/// accepts only Ed25519 keys and returns `false` for any other scheme.
fun verify_personal_message_signature(
    public_key: &PublicKey,
    signature: &vector<u8>,
    message: &vector<u8>,
): bool {
    let ed25519 = signature_scheme::ed25519();
    if (public_key.scheme() != ed25519) return false;
    if (signature.length() != ED25519_WIRE_SIGNATURE_LENGTH) return false;
    if (signature[0] != ed25519.flag()) return false;

    let raw_signature = vector::tabulate!(ED25519_SIGNATURE_LENGTH, |i| signature[1 + i]);
    let signer = vector::tabulate!(
        ED25519_PUBLIC_KEY_LENGTH,
        |i| signature[1 + ED25519_SIGNATURE_LENGTH + i],
    );
    if (&signer != public_key.raw_bytes()) return false;

    // The signed bytes are the hash of the intent followed by the BCS-encoded message.
    let mut intent_message = intent::iota_personal_message().to_bytes();
    intent_message.append(bcs::to_bytes(message));
    ed25519::ed25519_verify(
        &raw_signature,
        public_key.raw_bytes(),
        &hash::blake2b256(&intent_message),
    )
}

// === Test Functions ===

#[test_only]
/// Returns the whole approval message for `Check` and destroys `nonce`.
public fun message_for_testing<Check>(
    nonce: VerifiedNonce<Check>,
    check_lines: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
): vector<u8> {
    message(nonce, check_lines, valid_until_epoch, ctx)
}
