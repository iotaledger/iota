// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use eyre::eyre;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::{
    Address, Ed25519PublicKey, Identifier, Secp256k1PublicKey, Secp256r1PublicKey, SignatureScheme,
    SigningDigest, StructTag, UserSignature,
    crypto::{Intent, IntentMessage, MultisigCommittee, PersonalMessage},
};
use move_core_types::runtime_value::{MoveStructLayout, MoveTypeLayout};
use serde::{Deserialize, Serialize};

use crate::{
    crypto::PublicKey,
    error::{IotaError, IotaResult},
    move_signature_scheme::MoveSignatureScheme,
    signature::{AuthenticatorTrait, VerifyParams},
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
/// that passes `MultisigCommittee::validate()`. MultiSig member keys are not
/// checked against their curves.
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
    /// For `MultiSig` keys `raw_bytes` is BCS-decoded as a `MultisigCommittee`.
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
        if scheme == SignatureScheme::Multisig {
            // Mirrors the `iota::multisig::multisig_validate_pubkey` native: a
            // canonical BCS decode (which rejects trailing bytes and
            // unsupported member schemes), then committee validation.
            bcs::from_bytes::<MultisigCommittee>(&raw_bytes)
                .map_err(|e| eyre!("Invalid MultisigCommittee: {e}"))?
                .validate()
                .map_err(|e| eyre!("Invalid MultisigCommittee: {e}"))?;
        } else {
            // As in Move, the length is checked first: the parser also accepts
            // the uncompressed and hybrid forms of a secp256k1 or secp256r1
            // key, while a `PublicKey` holds the compressed form only.
            let expected_length = match scheme {
                SignatureScheme::Ed25519 => Ed25519PublicKey::LENGTH,
                SignatureScheme::Secp256k1 => Secp256k1PublicKey::LENGTH,
                SignatureScheme::Secp256r1 | SignatureScheme::PasskeyAuthenticator => {
                    Secp256r1PublicKey::LENGTH
                }
                _ => return Err(eyre!("Unsupported signature scheme: {scheme:?}")),
            };
            if raw_bytes.len() != expected_length {
                return Err(eyre!(
                    "Invalid public key bytes: {} bytes, expected {expected_length}",
                    raw_bytes.len()
                ));
            }
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

    /// Returns the runtime layout of the Move `public_key::PublicKey` struct.
    pub fn layout() -> MoveTypeLayout {
        let scheme = MoveTypeLayout::Struct(Box::new(MoveStructLayout(Box::new(vec![
            MoveTypeLayout::U8,
        ]))));
        let raw_bytes = MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8));
        MoveTypeLayout::Struct(Box::new(MoveStructLayout(Box::new(vec![
            scheme, raw_bytes,
        ]))))
    }

    /// Returns the `SignatureScheme` for this public key.
    pub fn scheme(&self) -> SignatureScheme {
        self.scheme.into()
    }

    /// Derives the `Address` for this public key.
    pub fn address(&self) -> Result<Address, eyre::Report> {
        let scheme = self.scheme();
        if scheme == SignatureScheme::Multisig {
            let multisig_public_key = bcs::from_bytes::<MultisigCommittee>(&self.raw_bytes)
                .map_err(|e| eyre!("Invalid MultisigCommittee bytes: {e}"))?;
            Ok(multisig_public_key.derive_address())
        } else {
            let public_key = PublicKey::try_from_bytes(scheme, &self.raw_bytes)
                .map_err(|e| eyre!("Invalid public key bytes: {e}"))?;
            Ok(Address::from(&public_key))
        }
    }
}

impl From<&SimpleKeypair> for MovePublicKey {
    fn from(key_pair: &SimpleKeypair) -> Self {
        let public_key = key_pair.public_key();
        Self::new(public_key.scheme(), public_key.as_ref().to_vec())
            .expect("SimpleKeypair always yields valid MovePublicKey")
    }
}

/// Verifies a built-in authenticator signature.
///
/// `signature_bytes` is a `UserSignature` in wire format (`flag || payload`)
/// whose scheme must be the scheme of `public_key`. This format is consistent
/// for all schemes: Ed25519, Secp256k1, Secp256r1, MultiSig, and Passkey.
///
/// `tx_signing_digest` is the signing digest of the transaction: the digest of
/// `IntentMessage(Intent::iota_transaction(), tx_data)`.
///
/// The multisig and passkey verification rules are taken from
/// `protocol_config`.
pub fn verify_builtin_authenticator_signature(
    protocol_config: &ProtocolConfig,
    public_key: &MovePublicKey,
    signature_bytes: &[u8],
    tx_signing_digest: &SigningDigest,
) -> IotaResult<()> {
    let (signature, address) = parse_signature_of_public_key(public_key, signature_bytes)?;

    let verify_params = VerifyParams::from_protocol_config(protocol_config);

    signature.verify_signing_digest(tx_signing_digest, address, &verify_params)
}

/// Verifies a signature, by `public_key`, of a personal message.
///
/// `signature_bytes` is a `UserSignature` in wire format (`flag || payload`)
/// whose scheme must be the scheme of `public_key`: Ed25519, Secp256k1,
/// Secp256r1 or Passkey. A MultiSig `public_key` is refused.
///
/// `message` is the content of the personal message. The signing message is
/// `IntentMessage(Intent::personal_message(), PersonalMessage(message))`.
///
/// The verification rules taken from `protocol_config` apply only to MultiSig
/// signatures, so they have no effect here. The parameter lets a later
/// protocol version change the check.
pub fn verify_personal_message_signature(
    protocol_config: &ProtocolConfig,
    public_key: &MovePublicKey,
    signature_bytes: &[u8],
    message: &[u8],
) -> IotaResult<()> {
    if public_key.scheme() == SignatureScheme::Multisig {
        return Err(IotaError::InvalidSignature {
            error: "A MultiSig public key is not accepted for a personal message".into(),
        });
    }

    let (signature, address) = parse_signature_of_public_key(public_key, signature_bytes)?;

    let intent_msg =
        IntentMessage::new(Intent::personal_message(), PersonalMessage(message.into()));

    let verify_params = VerifyParams::from_protocol_config(protocol_config);

    signature.verify_claims(&intent_msg, address, &verify_params)
}

/// Parses `signature_bytes` as a `UserSignature` whose scheme is the scheme of
/// `public_key`, and returns it with the address of `public_key`.
///
/// A signature of another scheme is refused before it is decoded.
fn parse_signature_of_public_key(
    public_key: &MovePublicKey,
    signature_bytes: &[u8],
) -> IotaResult<(UserSignature, Address)> {
    let expected_scheme = public_key.scheme();

    // The scheme is read from the flag byte, because decoding the whole
    // signature costs more: for a Passkey signature it parses
    // `clientDataJSON` as JSON.
    let flag = signature_bytes
        .first()
        .ok_or_else(|| IotaError::InvalidSignature {
            error: "Invalid signature bytes: missing signature scheme flag".into(),
        })?;
    let actual_scheme = match SignatureScheme::from_byte(*flag) {
        Ok(
            scheme @ (SignatureScheme::Ed25519
            | SignatureScheme::Secp256k1
            | SignatureScheme::Secp256r1
            | SignatureScheme::Multisig
            | SignatureScheme::PasskeyAuthenticator),
        ) => scheme,
        Ok(_) => {
            return Err(IotaError::InvalidSignature {
                error: "Unsupported signature type".into(),
            });
        }
        Err(e) => {
            return Err(IotaError::InvalidSignature {
                error: format!("Invalid signature bytes: {e}"),
            });
        }
    };
    if actual_scheme != expected_scheme {
        return Err(IotaError::InvalidSignature {
            error: format!(
                "Signature scheme mismatch: expected {expected_scheme:?}, got {actual_scheme:?}"
            ),
        });
    }

    let signature =
        UserSignature::from_bytes(signature_bytes).map_err(|e| IotaError::InvalidSignature {
            error: format!("Invalid signature bytes: {e}"),
        })?;

    let address = public_key
        .address()
        .map_err(|e| IotaError::InvalidSignature {
            error: format!("Invalid public key bytes: {e}"),
        })?;

    Ok((signature, address))
}

#[cfg(test)]
#[path = "unit_tests/move_public_key_tests.rs"]
mod move_public_key_tests;
