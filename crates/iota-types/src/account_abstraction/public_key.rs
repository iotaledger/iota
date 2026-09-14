// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use eyre::eyre;
use fastcrypto::{
    ed25519::Ed25519PublicKey, secp256k1::Secp256k1PublicKey, secp256r1::Secp256r1PublicKey,
    traits::ToFromBytes,
};
use iota_sdk_types::{Address, Identifier, StructTag, crypto::PublicKey as SdkPublicKey};
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
/// [`Self::new`] applies the same validation as the Move `public_key::create`
/// function: a supported scheme, non-empty bytes, and a valid curve point for
/// the declared scheme — or, for `MultiSig`, a canonically encoded committee
/// whose structure and member keys are themselves valid.
///
/// Deserialization performs none of that, so a `MovePublicKey` read from chain
/// may hold bytes that do not represent a valid key. Code consuming a
/// chain-read value must handle errors rather than assume validity.
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
        if raw_bytes.is_empty() {
            return Err(eyre!("Public key bytes are empty"));
        }
        // Rejects the schemes that are not valid for account public keys, as
        // the Move function's `EUnknownPublicKeyScheme` branch does.
        let move_scheme: MoveSignatureScheme = scheme.try_into()?;
        if scheme == SignatureScheme::MultiSig {
            // Mirrors the `iota::multisig::multisig_validate_pubkey` native: a
            // canonical BCS decode (which rejects trailing bytes and
            // unsupported member schemes), then committee validation, then each
            // member key against its curve. Committee validation alone does not
            // check curve points.
            let committee = bcs::from_bytes::<MultiSigPublicKey>(&raw_bytes)
                .map_err(|e| eyre!("Invalid MultiSigPublicKey: {e}"))?;
            committee
                .validate()
                .map_err(|e| eyre!("Invalid MultiSigPublicKey: {e}"))?;
            for member in committee.members() {
                if !member_public_key_is_on_curve(member.public_key()) {
                    return Err(eyre!("Invalid MultiSig member public key"));
                }
            }
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

/// Whether a MultiSig member's key is a point on its scheme's curve.
///
/// Any scheme not listed here is rejected: `PublicKey` is `#[non_exhaustive]`,
/// so a member scheme added upstream stays rejected until it is handled here.
///
/// Shared with the `iota::multisig::multisig_validate_pubkey` native so that
/// the on-chain check and [`MovePublicKey::new`] cannot disagree.
pub fn member_public_key_is_on_curve(public_key: &SdkPublicKey) -> bool {
    match public_key {
        SdkPublicKey::Ed25519(pk) => Ed25519PublicKey::from_bytes(pk.inner()).is_ok(),
        SdkPublicKey::Secp256k1(pk) => Secp256k1PublicKey::from_bytes(pk.inner()).is_ok(),
        SdkPublicKey::Secp256r1(pk) => Secp256r1PublicKey::from_bytes(pk.inner()).is_ok(),
        SdkPublicKey::Passkey(pk) => Secp256r1PublicKey::from_bytes(pk.inner().inner()).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/public_key_tests.rs"]
mod public_key_tests;
