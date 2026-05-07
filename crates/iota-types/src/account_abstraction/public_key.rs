// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use eyre::eyre;
use iota_sdk_types::{Address, Identifier, StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    account_abstraction::signature_scheme::MoveSignatureScheme,
    crypto::{IotaKeyPair, PublicKey, SignatureScheme},
    multisig::MultiSigPublicKey,
};

pub const PUBLIC_KEY_MODULE_NAME: Identifier = Identifier::from_static("public_key");
pub const PUBLIC_KEY_STRUCT_NAME: Identifier = Identifier::from_static("PublicKey");

/// Rust mirror of the Move `public_key::PublicKey` struct, stored as a dynamic
/// field value on built-in authenticator accounts. BCS layout matches the Move
/// struct.
///
/// **Validation gap**: the Move `public_key::create` function only checks byte
/// length and MultiSig BCS structure — it does not verify curve-point validity.
/// This means a `MovePublicKey` read from chain via BCS deserialization may
/// hold raw bytes that are length-correct but do not represent a valid curve
/// point.  `new()` performs full validation (including curve-point checks), but
/// deserialization does not.  All code that consumes a chain-read
/// `MovePublicKey` must tolerate this by handling errors rather than assuming
/// the bytes are cryptographically valid.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct MovePublicKey {
    /// The signature scheme for this key.
    scheme: MoveSignatureScheme,
    /// Raw key material without the scheme flag prefix.
    ///
    /// For `MultiSig` keys `raw_bytes` is BCS-decoded as a `MultiSigPublicKey`.
    /// For all other schemes `raw_bytes` is the raw public key bytes.
    raw_bytes: Vec<u8>,
}

impl MovePublicKey {
    /// Constructs a `MovePublicKey` from a `scheme` and raw key `raw_bytes`.
    ///
    /// Returns an error if `scheme` is not supported for account public keys,
    /// or if `raw_bytes` are not valid for the declared scheme.
    pub fn new(scheme: SignatureScheme, raw_bytes: Vec<u8>) -> Result<Self, eyre::Report> {
        // Validate scheme support before inspecting the bytes.
        let move_scheme: MoveSignatureScheme = scheme.try_into()?;

        if raw_bytes.is_empty() {
            return Err(eyre!("Public key bytes are empty"));
        }
        if scheme == SignatureScheme::MultiSig {
            bcs::from_bytes::<MultiSigPublicKey>(&raw_bytes)
                .map_err(|e| eyre!("Invalid MultiSigPublicKey: {e}"))?;
        } else {
            PublicKey::try_from_bytes(scheme, &raw_bytes)
                .map_err(|e| eyre!("Invalid public key bytes: {e}"))?;
        }
        Ok(Self {
            scheme: move_scheme,
            raw_bytes,
        })
    }

    pub fn tag() -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            PUBLIC_KEY_MODULE_NAME,
            PUBLIC_KEY_STRUCT_NAME,
            Vec::new(),
        )
    }

    /// Returns the `SignatureScheme` for this public key.
    pub fn scheme(&self) -> SignatureScheme {
        self.scheme.into()
    }

    /// Derives the `Address` for this public key.
    pub fn address(&self) -> Result<Address, eyre::Report> {
        let scheme = self.scheme();
        if scheme == SignatureScheme::MultiSig {
            let multisig_public_key = bcs::from_bytes::<MultiSigPublicKey>(&self.raw_bytes)
                .map_err(|e| eyre!("Invalid MultiSigPublicKey bytes: {e}"))?;
            Ok(Address::from(&multisig_public_key))
        } else {
            let public_key = PublicKey::try_from_bytes(scheme, &self.raw_bytes)
                .map_err(|e| eyre!("Invalid public key bytes: {e}"))?;
            Ok(Address::from(&public_key))
        }
    }
}

impl From<&IotaKeyPair> for MovePublicKey {
    fn from(key_pair: &IotaKeyPair) -> Self {
        let public_key = key_pair.public();
        Self::new(public_key.scheme(), public_key.as_ref().to_vec())
            .expect("IotaKeyPair always yields valid MovePublicKey")
    }
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/public_key_tests.rs"]
mod public_key_tests;
