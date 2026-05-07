// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use eyre::ensure;
use iota_sdk_types::{Address, Identifier, SignatureScheme, StructTag};
use serde::{Deserialize, Serialize};

pub const SIGNATURE_SCHEME_MODULE_NAME: Identifier = Identifier::from_static("signature_scheme");
pub const SIGNATURE_SCHEME_STRUCT_NAME: Identifier = Identifier::from_static("SignatureScheme");

/// Rust mirror of the Move `signature_scheme::SignatureScheme` struct.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
pub struct MoveSignatureScheme {
    /// The raw flag byte that identifies the scheme.
    flag: u8,
}

impl MoveSignatureScheme {
    pub fn tag() -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            SIGNATURE_SCHEME_MODULE_NAME,
            SIGNATURE_SCHEME_STRUCT_NAME,
            Vec::new(),
        )
    }

    /// Returns the raw flag byte that identifies this scheme.
    pub fn flag(&self) -> u8 {
        self.flag
    }
}

impl TryFrom<SignatureScheme> for MoveSignatureScheme {
    type Error = eyre::Report;

    /// Converts a `SignatureScheme` to a `MoveSignatureScheme`.
    ///
    /// Returns an error for schemes that are not valid for account public keys:
    /// BLS12381 and MoveAuthenticator.
    fn try_from(scheme: SignatureScheme) -> Result<Self, eyre::Report> {
        ensure!(
            matches!(
                scheme,
                SignatureScheme::Ed25519
                    | SignatureScheme::Secp256k1
                    | SignatureScheme::Secp256r1
                    | SignatureScheme::Multisig
                    | SignatureScheme::PasskeyAuthenticator
            ),
            "Unsupported signature scheme for account public key: {scheme:?}"
        );

        Ok(Self {
            flag: scheme.to_u8(),
        })
    }
}

impl From<MoveSignatureScheme> for SignatureScheme {
    fn from(move_scheme: MoveSignatureScheme) -> Self {
        SignatureScheme::from_byte(move_scheme.flag).expect("invariant: scheme flag valid")
    }
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/signature_scheme_tests.rs"]
mod signature_scheme_tests;
