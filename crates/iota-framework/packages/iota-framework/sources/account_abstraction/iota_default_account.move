// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// `IotaDefaultAccount` is a Move-based abstract account that verifies the
/// same GenericSignature byte formats used today — Ed25519, Secp256k1,
/// Secp256r1, MultiSig, and Passkey — without any client-side changes.
///
/// All signing schemes sign `blake2b256(bcs(IntentMessage<TransactionData>))`,
/// which is exposed via `auth_context::signing_digest()`. Credentials
/// (public keys) are stored in a dynamic field on the object.
module iota::iota_default_account;

use iota::account;
use iota::authenticator_function;
use iota::bcs::{Self, BCS};
use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use std::ascii;
use std::hash::sha2_256;

// === Constants ===

const ED25519_FLAG: u8 = 0x00;
const SECP256K1_FLAG: u8 = 0x01;
const SECP256R1_FLAG: u8 = 0x02;
const MULTISIG_FLAG: u8 = 0x03;
const PASSKEY_FLAG: u8 = 0x06;

/// SHA-256 hash identifier used by the ecdsa_k1 and ecdsa_r1 natives.
const SHA256: u8 = 1;

const ED25519_PK_LEN: u64 = 32;
const ECDSA_PK_LEN: u64 = 33;
const SIG_LEN: u64 = 64;

// === Errors ===

#[error(code = 0)]
const EUnsupportedScheme: vector<u8> = b"Unsupported signature scheme flag";

#[error(code = 1)]
const EInvalidSignature: vector<u8> = b"Signature verification failed";

#[error(code = 2)]
const EPublicKeyMismatch: vector<u8> = b"Signature public key does not match stored credential";

#[error(code = 3)]
const EInsufficientWeight: vector<u8> = b"MultiSig: accumulated weight below threshold";

#[error(code = 4)]
const EInvalidKeyLength: vector<u8> = b"Public key has incorrect byte length";

#[error(code = 5)]
const EChallengeMismatch: vector<u8> = b"Passkey: client_data_json challenge does not match signing digest";

#[error(code = 6)]
const EUnsupportedSubScheme: vector<u8> = b"MultiSig: unsupported sub-scheme flag";

// === Structs ===

/// An abstract account whose only on-chain data is its object ID.
/// The authorised credential (public key) is stored in a dynamic field.
public struct IotaDefaultAccount has key {
    id: UID,
}

/// Dynamic field key for the stored signing credential.
public struct CredentialKey has copy, drop, store {}

// === Constructors ===

/// Creates a new `IotaDefaultAccount` that is authenticated via an Ed25519 key.
/// The stored credential bytes are `[0x00 || pk(32)]`.
public fun new_ed25519(pk: vector<u8>, ctx: &mut TxContext): IotaDefaultAccount {
    assert!(pk.length() == ED25519_PK_LEN, EInvalidKeyLength);
    let mut credential = vector[ED25519_FLAG];
    credential.append(pk);
    new_with_credential(credential, ctx)
}

/// Creates a new `IotaDefaultAccount` that is authenticated via a Secp256k1 key.
/// The stored credential bytes are `[0x01 || pk(33)]`.
public fun new_secp256k1(pk: vector<u8>, ctx: &mut TxContext): IotaDefaultAccount {
    assert!(pk.length() == ECDSA_PK_LEN, EInvalidKeyLength);
    let mut credential = vector[SECP256K1_FLAG];
    credential.append(pk);
    new_with_credential(credential, ctx)
}

/// Creates a new `IotaDefaultAccount` that is authenticated via a Secp256r1 key.
/// The stored credential bytes are `[0x02 || pk(33)]`.
public fun new_secp256r1(pk: vector<u8>, ctx: &mut TxContext): IotaDefaultAccount {
    assert!(pk.length() == ECDSA_PK_LEN, EInvalidKeyLength);
    let mut credential = vector[SECP256R1_FLAG];
    credential.append(pk);
    new_with_credential(credential, ctx)
}

/// Creates a new `IotaDefaultAccount` authenticated via a Passkey (Secp256r1).
/// The stored credential bytes are `[0x06 || pk(33)]`.
public fun new_passkey(pk: vector<u8>, ctx: &mut TxContext): IotaDefaultAccount {
    assert!(pk.length() == ECDSA_PK_LEN, EInvalidKeyLength);
    let mut credential = vector[PASSKEY_FLAG];
    credential.append(pk);
    new_with_credential(credential, ctx)
}

/// Creates a new `IotaDefaultAccount` authenticated via MultiSig.
/// `multisig_pk_bytes` must be `bcs(MultiSigPublicKey{ pk_map, threshold })`,
/// where `pk_map` is a vector of `(flag, pk_bytes, weight)` triples.
/// The stored credential bytes are `[0x03 || multisig_pk_bytes]`.
public fun new_multisig(multisig_pk_bytes: vector<u8>, ctx: &mut TxContext): IotaDefaultAccount {
    let mut credential = vector[MULTISIG_FLAG];
    credential.append(multisig_pk_bytes);
    new_with_credential(credential, ctx)
}

// === Account creation (register as abstract account) ===

/// Creates and registers an Ed25519-authenticated `IotaDefaultAccount` as a shared object.
/// The stored credential bytes are `[0x00 || pk(32)]`.
public fun create_ed25519(pk: vector<u8>, ctx: &mut TxContext) {
    let account = new_ed25519(pk, ctx);
    account::create_account_v1(account, self_auth_fn_ref());
}

/// Creates and registers a Secp256k1-authenticated `IotaDefaultAccount` as a shared object.
/// The stored credential bytes are `[0x01 || pk(33)]`.
public fun create_secp256k1(pk: vector<u8>, ctx: &mut TxContext) {
    let account = new_secp256k1(pk, ctx);
    account::create_account_v1(account, self_auth_fn_ref());
}

/// Creates and registers a Secp256r1-authenticated `IotaDefaultAccount` as a shared object.
/// The stored credential bytes are `[0x02 || pk(33)]`.
public fun create_secp256r1(pk: vector<u8>, ctx: &mut TxContext) {
    let account = new_secp256r1(pk, ctx);
    account::create_account_v1(account, self_auth_fn_ref());
}

/// Creates and registers a Passkey-authenticated `IotaDefaultAccount` as a shared object.
/// The stored credential bytes are `[0x06 || pk(33)]`.
public fun create_passkey(pk: vector<u8>, ctx: &mut TxContext) {
    let account = new_passkey(pk, ctx);
    account::create_account_v1(account, self_auth_fn_ref());
}

/// Creates and registers a MultiSig-authenticated `IotaDefaultAccount` as a shared object.
/// `multisig_pk_bytes` must be `bcs(MultiSigPublicKey{ pk_map, threshold })`.
/// The stored credential bytes are `[0x03 || multisig_pk_bytes]`.
public fun create_multisig(multisig_pk_bytes: vector<u8>, ctx: &mut TxContext) {
    let account = new_multisig(multisig_pk_bytes, ctx);
    account::create_account_v1(account, self_auth_fn_ref());
}

// === Authenticator function ===

/// Entry point for Move-based transaction authentication.
///
/// `generic_signature` must be a GenericSignature in the current wire format:
/// - Ed25519:   `[0x00 | sig(64) | pk(32)]`
/// - Secp256k1: `[0x01 | sig(64) | pk(33)]`
/// - Secp256r1: `[0x02 | sig(64) | pk(33)]`
/// - MultiSig:  `[0x03 | BCS(MultiSig{sigs, bitmap, pk_map, threshold})]`
/// - Passkey:   `[0x06 | BCS(PasskeyAuthenticator{auth_data, client_data_json, sig(64), pk(33)})]`
///
/// This is bit-for-bit identical to the signatures accepted by the existing
/// Rust-based verifier, so no client-side changes are required.
#[authenticator]
public fun authenticate(
    account: &IotaDefaultAccount,
    generic_signature: vector<u8>,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let flag = *generic_signature.borrow(0);
    let signing_digest = auth_ctx.signing_digest();

    if (flag == ED25519_FLAG) {
        verify_ed25519(account, &generic_signature, signing_digest);
    } else if (flag == SECP256K1_FLAG) {
        verify_secp256k1(account, &generic_signature, signing_digest);
    } else if (flag == SECP256R1_FLAG) {
        verify_secp256r1(account, &generic_signature, signing_digest);
    } else if (flag == MULTISIG_FLAG) {
        verify_multisig(account, &generic_signature, signing_digest);
    } else if (flag == PASSKEY_FLAG) {
        verify_passkey(account, &generic_signature, signing_digest);
    } else {
        abort EUnsupportedScheme
    }
}

// === Private verification helpers ===

fun verify_ed25519(
    account: &IotaDefaultAccount,
    sig_bytes: &vector<u8>,
    signing_digest: &vector<u8>,
) {
    // Layout: [0x00 | sig(64) | pk(32)]
    let sig = slice(sig_bytes, 1, SIG_LEN);
    let pk = slice(sig_bytes, 1 + SIG_LEN, ED25519_PK_LEN);

    let credential = stored_credential(account);
    let expected = build_credential(ED25519_FLAG, &pk);
    assert!(credential == &expected, EPublicKeyMismatch);

    assert!(ed25519::ed25519_verify(&sig, &pk, signing_digest), EInvalidSignature);
}

fun verify_secp256k1(
    account: &IotaDefaultAccount,
    sig_bytes: &vector<u8>,
    signing_digest: &vector<u8>,
) {
    // Layout: [0x01 | sig(64) | pk(33)]
    let sig = slice(sig_bytes, 1, SIG_LEN);
    let pk = slice(sig_bytes, 1 + SIG_LEN, ECDSA_PK_LEN);

    let credential = stored_credential(account);
    let expected = build_credential(SECP256K1_FLAG, &pk);
    assert!(credential == &expected, EPublicKeyMismatch);

    assert!(ecdsa_k1::secp256k1_verify(&sig, &pk, signing_digest, SHA256), EInvalidSignature);
}

fun verify_secp256r1(
    account: &IotaDefaultAccount,
    sig_bytes: &vector<u8>,
    signing_digest: &vector<u8>,
) {
    // Layout: [0x02 | sig(64) | pk(33)]
    let sig = slice(sig_bytes, 1, SIG_LEN);
    let pk = slice(sig_bytes, 1 + SIG_LEN, ECDSA_PK_LEN);

    let credential = stored_credential(account);
    let expected = build_credential(SECP256R1_FLAG, &pk);
    assert!(credential == &expected, EPublicKeyMismatch);

    assert!(ecdsa_r1::secp256r1_verify(&sig, &pk, signing_digest, SHA256), EInvalidSignature);
}

fun verify_passkey(
    account: &IotaDefaultAccount,
    sig_bytes: &vector<u8>,
    signing_digest: &vector<u8>,
) {
    // Layout: [0x06 | BCS(RawPasskeyAuthenticator{auth_data, client_data_json, user_signature})]
    // where user_signature = [0x02 | sig(64) | pk(33)] (98 bytes total, BCS-encoded as vector<u8>).
    let payload = slice(sig_bytes, 1, sig_bytes.length() - 1);
    let mut bcs_parser = bcs::new(payload);

    let authenticator_data = bcs_parser.peel_vec_u8();
    let client_data_json = bcs_parser.peel_vec_u8();
    // user_signature is a BCS vector<u8> containing [flag(1) | sig(64) | pk(33)]
    let user_signature = bcs_parser.peel_vec_u8();
    let sig = slice(&user_signature, 1, SIG_LEN);
    let pk = slice(&user_signature, 1 + SIG_LEN, ECDSA_PK_LEN);

    let credential = stored_credential(account);
    let expected = build_credential(PASSKEY_FLAG, &pk);
    assert!(credential == &expected, EPublicKeyMismatch);

    // The challenge in client_data_json must equal the signing digest (base64url-encoded).
    let challenge = extract_challenge(&client_data_json);
    assert!(base64url_decode(&challenge) == *signing_digest, EChallengeMismatch);

    // WebAuthn message: authenticator_data || SHA-256(client_data_json)
    let mut msg = authenticator_data;
    let cdj_hash = sha2_256(client_data_json);
    msg.append(cdj_hash);

    assert!(ecdsa_r1::secp256r1_verify(&sig, &pk, &msg, SHA256), EInvalidSignature);
}

fun verify_multisig(
    account: &IotaDefaultAccount,
    sig_bytes: &vector<u8>,
    signing_digest: &vector<u8>,
) {
    // Layout: [0x03 | BCS(MultiSig{sigs, bitmap, pk_map, threshold})]
    // sigs:      vector<(flag: u8, sig_bytes: vector<u8>)>  (BCS enum: ULEB128 tag + payload)
    // bitmap:    u16
    // pk_map:    vector<(flag: u8, pk_bytes: vector<u8>, weight: u8)>
    // threshold: u16
    let payload = slice(sig_bytes, 1, sig_bytes.length() - 1);
    let mut bcs_parser = bcs::new(payload);

    // Deserialize sigs as vector<vector<u8>>: each entry is [flag(1) || bytes]
    let num_sigs = bcs_parser.peel_vec_length();
    let mut sig_list: vector<vector<u8>> = vector[];
    let mut i = 0;
    while (i < num_sigs) {
        // CompressedSignature is a BCS enum: ULEB128 tag followed by fixed payload
        let sig_flag = bcs_parser.peel_u8();
        let sig_payload = if (sig_flag == ED25519_FLAG) {
            peel_fixed_bytes(&mut bcs_parser, SIG_LEN)
        } else {
            // Secp256k1 / Secp256r1: 64-byte sig
            peel_fixed_bytes(&mut bcs_parser, SIG_LEN)
        };
        let mut entry = vector[sig_flag];
        entry.append(sig_payload);
        sig_list.push_back(entry);
        i = i + 1;
    };

    let bitmap = bcs_parser.peel_u16();

    // Deserialize pk_map: vector<(flag: u8, pk_bytes: vector<u8>, weight: u8)>
    let num_pks = bcs_parser.peel_vec_length();
    let mut pk_flags: vector<u8> = vector[];
    let mut pk_bytes_list: vector<vector<u8>> = vector[];
    let mut weights: vector<u8> = vector[];
    let mut j = 0;
    while (j < num_pks) {
        let pk_flag = bcs_parser.peel_u8();
        let pk_bytes = if (pk_flag == ED25519_FLAG) {
            peel_fixed_bytes(&mut bcs_parser, ED25519_PK_LEN)
        } else {
            peel_fixed_bytes(&mut bcs_parser, ECDSA_PK_LEN)
        };
        let weight = bcs_parser.peel_u8();
        pk_flags.push_back(pk_flag);
        pk_bytes_list.push_back(pk_bytes);
        weights.push_back(weight);
        j = j + 1;
    };

    let threshold = bcs_parser.peel_u16();

    // Check that the stored credential matches the public key set.
    // Credential: [0x03 || BCS(pk_map_entries, threshold)]
    // We reconstruct it for comparison.
    let credential = stored_credential(account);
    let expected_cred = build_multisig_credential(&pk_flags, &pk_bytes_list, &weights, threshold);
    assert!(credential == &expected_cred, EPublicKeyMismatch);

    // Verify each signature indicated by the bitmap and accumulate weights.
    let mut weight_sum: u16 = 0;
    let mut sig_idx: u64 = 0;
    let mut k: u8 = 0;
    while ((k as u64) < num_pks) {
        if (bitmap & (1u16 << k) != 0) {
            let pk_flag = *pk_flags.borrow(k as u64);
            let pk = pk_bytes_list.borrow(k as u64);
            let sig_entry = sig_list.borrow(sig_idx);
            sig_idx = sig_idx + 1;

            let sub_sig_flag = *sig_entry.borrow(0);
            let sub_sig = slice(sig_entry, 1, SIG_LEN);

            let ok = if (sub_sig_flag == ED25519_FLAG && pk_flag == ED25519_FLAG) {
                ed25519::ed25519_verify(&sub_sig, pk, signing_digest)
            } else if (sub_sig_flag == SECP256K1_FLAG && pk_flag == SECP256K1_FLAG) {
                ecdsa_k1::secp256k1_verify(&sub_sig, pk, signing_digest, SHA256)
            } else if (sub_sig_flag == SECP256R1_FLAG && pk_flag == SECP256R1_FLAG) {
                ecdsa_r1::secp256r1_verify(&sub_sig, pk, signing_digest, SHA256)
            } else {
                abort EUnsupportedSubScheme
            };

            if (ok) {
                weight_sum = weight_sum + (*weights.borrow(k as u64) as u16);
            };
        };
        k = k + 1;
    };

    assert!(weight_sum >= threshold, EInsufficientWeight);
}

// === Internal helpers ===

/// Returns an `AuthenticatorFunctionRefV1` pointing to this module's `authenticate` function.
fun self_auth_fn_ref(): authenticator_function::AuthenticatorFunctionRefV1<IotaDefaultAccount> {
    authenticator_function::create_auth_function_ref_v1_internal<IotaDefaultAccount>(
        @iota,
        ascii::string(b"iota_default_account"),
        ascii::string(b"authenticate"),
    )
}

fun new_with_credential(credential: vector<u8>, ctx: &mut TxContext): IotaDefaultAccount {
    let mut id = object::new(ctx);
    dynamic_field::add(&mut id, CredentialKey {}, credential);
    IotaDefaultAccount { id }
}

fun stored_credential(account: &IotaDefaultAccount): &vector<u8> {
    dynamic_field::borrow(&account.id, CredentialKey {})
}

fun build_credential(flag: u8, pk: &vector<u8>): vector<u8> {
    let mut cred = vector[flag];
    cred.append(*pk);
    cred
}

/// Reconstruct the multisig credential bytes for comparison with the stored value.
/// Format: [0x03 || BCS(pk_map_entries || threshold)]
fun build_multisig_credential(
    pk_flags: &vector<u8>,
    pk_bytes_list: &vector<vector<u8>>,
    weights: &vector<u8>,
    threshold: u16,
): vector<u8> {
    let mut cred = vector[MULTISIG_FLAG];
    let n = pk_flags.length();
    // Encode as a BCS vector: ULEB128 length followed by entries
    cred.append(encode_uleb128(n));
    let mut i = 0;
    while (i < n) {
        cred.push_back(*pk_flags.borrow(i));
        cred.append(*pk_bytes_list.borrow(i));
        cred.push_back(*weights.borrow(i));
        i = i + 1;
    };
    // Append threshold as little-endian u16
    cred.push_back((threshold & 0xff) as u8);
    cred.push_back(((threshold >> 8) & 0xff) as u8);
    cred
}

/// Extract a contiguous slice from a byte vector.
fun slice(v: &vector<u8>, start: u64, len: u64): vector<u8> {
    let mut result = vector[];
    let mut i = 0;
    while (i < len) {
        result.push_back(*v.borrow(start + i));
        i = i + 1;
    };
    result
}

/// Read exactly `len` bytes from a BCS parser into a vector.
fun peel_fixed_bytes(bcs_parser: &mut BCS, len: u64): vector<u8> {
    let mut result = vector[];
    let mut i = 0;
    while (i < len) {
        result.push_back(bcs_parser.peel_u8());
        i = i + 1;
    };
    result
}

/// Encode a `u64` length value as a ULEB128 byte sequence.
fun encode_uleb128(mut value: u64): vector<u8> {
    let mut result = vector[];
    loop {
        let byte = (value & 0x7f) as u8;
        value = value >> 7;
        if (value == 0) {
            result.push_back(byte);
            break
        } else {
            result.push_back(byte | 0x80);
        }
    };
    result
}

// === Passkey helpers ===

/// Extract the value of the "challenge" field from a WebAuthn `client_data_json` byte vector.
/// The JSON is assumed to have the form: `{"type":"webauthn.get","challenge":"<base64url>", ...}`.
/// Returns the raw base64url-encoded string bytes (without quotes).
fun extract_challenge(client_data_json: &vector<u8>): vector<u8> {
    let key = b"\"challenge\":\"";
    let n = client_data_json.length();
    let key_len = key.length();

    let mut i = 0;
    while (i + key_len <= n) {
        if (bytes_match(client_data_json, i, &key)) {
            // Found the key; read until the closing quote
            let start = i + key_len;
            let mut end = start;
            while (end < n && *client_data_json.borrow(end) != 34u8) { // 34 = '"'
                end = end + 1;
            };
            return slice(client_data_json, start, end - start)
        };
        i = i + 1;
    };
    // If not found, return empty — the EChallengeMismatch assert will fire.
    vector[]
}

/// Returns true if `haystack[offset..(offset + needle.length())]` equals `needle`.
fun bytes_match(haystack: &vector<u8>, offset: u64, needle: &vector<u8>): bool {
    let n = needle.length();
    let h_len = haystack.length();
    if (offset + n > h_len) return false;
    let mut i = 0;
    while (i < n) {
        if (*haystack.borrow(offset + i) != *needle.borrow(i)) return false;
        i = i + 1;
    };
    true
}

/// Decode a base64url-encoded byte vector (no padding).
/// The base64url alphabet uses `-` (62) and `_` (63) instead of `+` and `/`.
/// Output length = floor(n * 3 / 4), e.g. 43 input chars → 32 output bytes.
fun base64url_decode(input: &vector<u8>): vector<u8> {
    let n = input.length();
    // Number of bytes the output should contain (no padding):
    //   n%4==0 → n*3/4 bytes; n%4==2 → +1 byte; n%4==3 → +2 bytes; n%4==1 → invalid
    let out_len = (n * 3) / 4;
    let mut result = vector[];
    let mut i = 0;
    while (i < n) {
        let a = base64url_char(*input.borrow(i));
        let b = if (i + 1 < n) { base64url_char(*input.borrow(i + 1)) } else { 0 };
        let c = if (i + 2 < n) { base64url_char(*input.borrow(i + 2)) } else { 0 };
        let d = if (i + 3 < n) { base64url_char(*input.borrow(i + 3)) } else { 0 };

        let triple = ((a as u32) << 18) | ((b as u32) << 12) | ((c as u32) << 6) | (d as u32);

        if (result.length() < out_len) result.push_back(((triple >> 16) & 0xff) as u8);
        if (result.length() < out_len) result.push_back(((triple >> 8) & 0xff) as u8);
        if (result.length() < out_len) result.push_back((triple & 0xff) as u8);

        i = i + 4;
    };
    result
}

/// Map a base64url character to its 6-bit value.
fun base64url_char(c: u8): u8 {
    if (c >= 65 && c <= 90) { // A-Z
        c - 65
    } else if (c >= 97 && c <= 122) { // a-z
        c - 71
    } else if (c >= 48 && c <= 57) { // 0-9
        c + 4
    } else if (c == 45) { // '-'
        62
    } else if (c == 95) { // '_'
        63
    } else {
        0
    }
}
