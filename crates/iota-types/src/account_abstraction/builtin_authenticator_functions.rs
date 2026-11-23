// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, Identifier, SignatureScheme, StructTag, Transaction, UserSignature,
    crypto::{Intent, IntentMessage, PersonalMessage},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    account_abstraction::authenticator_function::AuthenticatorFunctionRefV1,
    error::{IotaError, IotaResult},
    public_key::MovePublicKey,
    signature::{AuthenticatorTrait, VerifyParams},
};

pub const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: Identifier =
    Identifier::from_static("builtin_authenticator_functions");

pub const PUBLIC_KEY_FIELD_NAME_STRUCT_NAME: Identifier =
    Identifier::from_static("PublicKeyFieldName");

pub const BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME: &str = "builtin_authenticator_v1";

/// The `#[error]` code of `builtin_authenticator_functions::EInvalidSignature`,
/// the abort raised when the built-in authenticator rejects a signature.
pub const INVALID_SIGNATURE_ERROR_CODE: u64 = 20;

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct PublicKeyFieldName {
    // This field is required to make a Rust struct compatible with an empty Move one.
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
}

impl PublicKeyFieldName {
    pub fn tag() -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME,
            PUBLIC_KEY_FIELD_NAME_STRUCT_NAME,
            Vec::new(),
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).expect("PublicKeyFieldName is always BCS-serializable")
    }
}

/// Returns an authentication function that references the built-in
/// authenticator.
pub fn builtin_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Returns true if `authenticator_function_ref` references the built-in
/// authenticator.
pub fn is_builtin_authenticator_function_ref(
    authenticator_function_ref: &AuthenticatorFunctionRefV1,
) -> bool {
    *authenticator_function_ref == builtin_authenticator_function_ref_v1()
}

/// Verifies a built-in authenticator signature.
///
/// `signature_bytes` is a `UserSignature` in wire format (`flag || payload`)
/// whose scheme must be the scheme of `public_key`. This format is consistent
/// for all schemes: Ed25519, Secp256k1, Secp256r1, MultiSig, and Passkey.
///
/// `tx_data_bytes` is the BCS-encoded `Transaction` used to reconstruct
/// the signing message as `IntentMessage(Intent::iota_transaction(), tx_data)`.
///
/// The multisig and passkey verification rules are taken from
/// `protocol_config`.
pub fn verify_builtin_signature(
    protocol_config: &ProtocolConfig,
    public_key: &MovePublicKey,
    signature_bytes: &[u8],
    tx_data_bytes: &[u8],
) -> IotaResult<()> {
    let (signature, address) = parse_signature_of_public_key(public_key, signature_bytes)?;

    // TODO: it would be nice to avoid this deserialization.
    let tx_data: Transaction =
        bcs::from_bytes(tx_data_bytes).map_err(|e| IotaError::InvalidSignature {
            error: format!("Failed to deserialize transaction data: {e}"),
        })?;
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    let verify_params = VerifyParams::from_protocol_config(protocol_config);

    signature.verify_claims(&intent_msg, address, &verify_params)
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
#[path = "../unit_tests/account_abstraction/builtin_authenticator_functions_tests.rs"]
mod builtin_authenticator_functions_tests;
