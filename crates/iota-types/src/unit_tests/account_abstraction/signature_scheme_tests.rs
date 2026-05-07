// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{account_abstraction::signature_scheme::MoveSignatureScheme, crypto::SignatureScheme};

// === TryFrom<SignatureScheme> ===

#[test]
fn try_from_ed25519_flag() {
    let scheme = MoveSignatureScheme::try_from(SignatureScheme::ED25519).unwrap();
    assert_eq!(scheme.flag(), 0x00);
}

#[test]
fn try_from_secp256k1_flag() {
    let scheme = MoveSignatureScheme::try_from(SignatureScheme::Secp256k1).unwrap();
    assert_eq!(scheme.flag(), 0x01);
}

#[test]
fn try_from_secp256r1_flag() {
    let scheme = MoveSignatureScheme::try_from(SignatureScheme::Secp256r1).unwrap();
    assert_eq!(scheme.flag(), 0x02);
}

#[test]
fn try_from_multisig_flag() {
    let scheme = MoveSignatureScheme::try_from(SignatureScheme::MultiSig).unwrap();
    assert_eq!(scheme.flag(), 0x03);
}

#[test]
fn try_from_passkey_flag() {
    let scheme = MoveSignatureScheme::try_from(SignatureScheme::PasskeyAuthenticator).unwrap();
    assert_eq!(scheme.flag(), 0x06);
}

#[test]
fn try_from_unsupported_schemes_error() {
    // BLS12381, ZkLoginAuthenticatorDeprecated, and MoveAuthenticator are not
    // valid for account public keys.
    for scheme in [
        SignatureScheme::BLS12381,
        #[allow(deprecated)]
        SignatureScheme::ZkLoginAuthenticatorDeprecated,
        SignatureScheme::MoveAuthenticator,
    ] {
        let err = MoveSignatureScheme::try_from(scheme)
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with("Unsupported signature scheme for account public key:"),
            "unexpected error for {scheme:?}: {err}"
        );
    }
}

// === From<MoveSignatureScheme> roundtrip ===

#[test]
fn roundtrip_supported_schemes() {
    for scheme in [
        SignatureScheme::ED25519,
        SignatureScheme::Secp256k1,
        SignatureScheme::Secp256r1,
        SignatureScheme::MultiSig,
        SignatureScheme::PasskeyAuthenticator,
    ] {
        let move_scheme = MoveSignatureScheme::try_from(scheme).unwrap();
        assert_eq!(SignatureScheme::from(move_scheme), scheme);
    }
}

// === BCS layout ===

#[test]
fn bcs_layout_is_single_flag_byte() {
    // MoveSignatureScheme BCS-encodes as exactly 1 byte (the flag), matching
    // the Move struct `SignatureScheme { flag: u8 }`.
    for (scheme, expected_flag) in [
        (SignatureScheme::ED25519, 0x00u8),
        (SignatureScheme::Secp256k1, 0x01),
        (SignatureScheme::Secp256r1, 0x02),
        (SignatureScheme::MultiSig, 0x03),
        (SignatureScheme::PasskeyAuthenticator, 0x06),
    ] {
        let move_scheme = MoveSignatureScheme::try_from(scheme).unwrap();
        let encoded = bcs::to_bytes(&move_scheme).unwrap();
        assert_eq!(encoded, vec![expected_flag], "BCS mismatch for {scheme:?}");
    }
}

#[test]
fn bcs_roundtrip() {
    for scheme in [
        SignatureScheme::ED25519,
        SignatureScheme::Secp256k1,
        SignatureScheme::Secp256r1,
        SignatureScheme::MultiSig,
        SignatureScheme::PasskeyAuthenticator,
    ] {
        let original = MoveSignatureScheme::try_from(scheme).unwrap();
        let encoded = bcs::to_bytes(&original).unwrap();
        let decoded: MoveSignatureScheme = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, original);
    }
}
