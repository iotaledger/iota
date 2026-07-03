// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::multisig_tests;

use iota::multisig;

// All byte vectors are BCS-encoded `MultiSigPublicKey` payloads (no `0x03` scheme flag).

#[test]
fun multisig_validate_pubkey_valid() {
    // 1-of-1 Ed25519: vec_len(1) | tag(0) | 32-byte key | weight(1) | threshold_le16(1)
    let one_of_one =
        x"0100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100";
    assert!(multisig::multisig_validate_pubkey(&one_of_one));

    // 1-of-2 mixed: Ed25519 (weight=1) + Secp256k1 (weight=1), threshold=1
    let mixed =
        x"0200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100";
    assert!(multisig::multisig_validate_pubkey(&mixed));
}

#[test]
fun multisig_validate_pubkey_invalid() {
    // Empty bytes.
    assert!(!multisig::multisig_validate_pubkey(&x""));
    // Empty committee: zero members with a non-zero threshold.
    // Layout: vec_len(0) | threshold_le16(1)
    assert!(!multisig::multisig_validate_pubkey(&x"000100"));
    // Zero threshold (otherwise-valid 1-of-1 Ed25519 with threshold_le16 = 0).
    assert!(
        !multisig::multisig_validate_pubkey(
            &x"01000000000000000000000000000000000000000000000000000000000000000000010000",
        ),
    );
    // Total weight (1) below threshold (2).
    assert!(
        !multisig::multisig_validate_pubkey(
            &x"01000000000000000000000000000000000000000000000000000000000000000000010200",
        ),
    );
    // Zero-weight member: the mixed committee from the valid test with the Ed25519 member's
    // weight set to 0. Total weight (1, from the Secp256k1 member) still meets the threshold,
    // so only the zero-weight rule is violated.
    assert!(
        !multisig::multisig_validate_pubkey(
            &x"0200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88000102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100",
        ),
    );
    // Duplicate Ed25519 members.
    assert!(
        !multisig::multisig_validate_pubkey(
            &x"0200000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010100",
        ),
    );
    // Trailing byte after an otherwise-valid 1-of-1 Ed25519 payload.
    assert!(
        !multisig::multisig_validate_pubkey(
            &x"01000000000000000000000000000000000000000000000000000000000000000000010100ff",
        ),
    );
    // Unknown member scheme tag (3 = ZkLoginDeprecated).
    assert!(!multisig::multisig_validate_pubkey(&x"0103010100"));
    // Secp256k1 member that is not a valid curve point (prefix 0x00).
    assert!(
        !multisig::multisig_validate_pubkey(
            &x"0101000000000000000000000000000000000000000000000000000000000000000000010100",
        ),
    );
}
