// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Defines the typed constants for IOTA's supported signature schemes.
///
/// Supported schemes:
/// - `ed25519`  (0x00) — EdDSA over Ed25519. 32-byte public key.
/// - `secp256k1`(0x01) — ECDSA over secp256k1 (Bitcoin/Ethereum style). Compressed 33-byte key.
/// - `secp256r1`(0x02) — ECDSA over secp256r1 (NIST P-256). Compressed 33-byte key.
/// - `multisig` (0x03) — k-of-n MultiSig. BCS-encoded `MultiSigPublicKey` as the key payload.
/// - `passkey`  (0x06) — WebAuthn Passkey (P-256 / ES256). Compressed 33-byte key.
module iota::signature_scheme;

// === Errors ===

// === Constants ===

const ED25519: u8 = 0x00;
const SECP256K1: u8 = 0x01;
const SECP256R1: u8 = 0x02;
const MULTISIG: u8 = 0x03;
const PASSKEY: u8 = 0x06;

// === Structs ===

/// A typed wrapper around the single-byte flag that identifies a signature scheme.
public struct SignatureScheme has copy, drop, store {
    /// The raw flag byte that identifies this scheme.
    flag: u8,
}

// === Public Functions ===

/// Returns the `SignatureScheme` for the Ed25519 signature scheme (`0x00`).
public fun ed25519(): SignatureScheme {
    SignatureScheme { flag: ED25519 }
}

/// Returns the `SignatureScheme` for the secp256k1 ECDSA signature scheme (`0x01`).
public fun secp256k1(): SignatureScheme {
    SignatureScheme { flag: SECP256K1 }
}

/// Returns the `SignatureScheme` for the secp256r1 (NIST P-256) ECDSA signature scheme (`0x02`).
public fun secp256r1(): SignatureScheme {
    SignatureScheme { flag: SECP256R1 }
}

/// Returns the `SignatureScheme` for the MultiSig scheme (`0x03`).
public fun multisig(): SignatureScheme {
    SignatureScheme { flag: MULTISIG }
}

/// Returns the `SignatureScheme` for the WebAuthn Passkey (P-256 / ES256) scheme (`0x06`).
public fun passkey(): SignatureScheme {
    SignatureScheme { flag: PASSKEY }
}

// === View Functions ===

/// Returns the raw flag byte.
public fun flag(self: &SignatureScheme): u8 {
    self.flag
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===

/// Constructs a `SignatureScheme` from an arbitrary flag byte.
/// For testing unknown/unsupported scheme paths only — do not use in production code.
#[test_only]
public fun from_flag_for_testing(flag: u8): SignatureScheme {
    SignatureScheme { flag }
}
