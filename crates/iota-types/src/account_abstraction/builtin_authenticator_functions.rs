// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::traits::ToFromBytes;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Identifier, ObjectId, StructTag,
    crypto::{Intent, IntentMessage},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    account_abstraction::{
        authenticator_function::AuthenticatorFunctionRefV1, public_key::MovePublicKey,
    },
    base_types::IotaAddress,
    crypto::{IotaSignature, SignatureScheme},
    dynamic_field::{self, Field},
    error::{IotaError, IotaResult, UserInputError},
    execution::DynamicallyLoadedObjectMetadata,
    object::Object,
    signature::{AuthenticatorTrait, GenericSignature, VerifyParams},
    transaction::{CallArg, TransactionData},
    utils::MoveAuthenticator,
};

pub const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: Identifier =
    Identifier::from_static("builtin_authenticator_functions");

pub const PUBLIC_KEY_FIELD_NAME_STRUCT_NAME: Identifier =
    Identifier::from_static("PublicKeyFieldName");

pub const ED25519_AUTHENTICATOR_FUNCTION_V1_NAME: &str = "ed25519_authenticator_function_ref_v1";
pub const SECP256K1_AUTHENTICATOR_FUNCTION_V1_NAME: &str =
    "secp256k1_authenticator_function_ref_v1";
pub const SECP256R1_AUTHENTICATOR_FUNCTION_V1_NAME: &str =
    "secp256r1_authenticator_function_ref_v1";
pub const MULTISIG_AUTHENTICATOR_FUNCTION_V1_NAME: &str = "multisig_authenticator_function_ref_v1";
pub const PASSKEY_AUTHENTICATOR_FUNCTION_V1_NAME: &str = "passkey_authenticator_function_ref_v1";

/// The pre-loaded data for a built-in authenticator.
///
/// Constructed once during transaction input loading so the executor does not
/// need to re-derive the scheme from the authenticator function reference.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct PreloadedBuiltinAuthenticatorData {
    /// The signature scheme derived from the authenticator function reference.
    /// Both the submitted signature and the on-chain public key must use this
    /// scheme.
    pub expected_scheme: SignatureScheme,
    /// The typed public key stored on-chain.
    pub public_key: MovePublicKey,
}

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
            IotaAddress::FRAMEWORK,
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME,
            PUBLIC_KEY_FIELD_NAME_STRUCT_NAME,
            Vec::new(),
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).expect("PublicKeyFieldName is always BCS-serializable")
    }
}

/// Returns an authentication function that references the built-in ed25519
/// authenticator.
pub fn ed25519_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: ED25519_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Returns an authentication function that references the built-in secp256k1
/// authenticator.
pub fn secp256k1_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: SECP256K1_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Returns an authentication function that references the built-in secp256r1
/// authenticator.
pub fn secp256r1_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: SECP256R1_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Returns an authentication function that references the built-in multisig
/// authenticator.
pub fn multisig_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: MULTISIG_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Returns an authentication function that references the built-in passkey
/// authenticator.
pub fn passkey_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: PASSKEY_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Derives the `PublicKeyFieldName` dynamic field ID for `account_object_id`,
/// fetches the object via `get_object`, and deserializes it.
///
/// Returns `Ok((field_id, Some((public_key, metadata))))` when present,
/// `Ok((field_id, None))` when absent, or `Err` on ID derivation /
/// deserialization failures. Callers are responsible for converting the `None`
/// case into their own error type if a missing key is fatal.
#[allow(clippy::type_complexity)]
pub fn load_builtin_public_key<F>(
    account_object_id: ObjectId,
    get_object: F,
) -> IotaResult<(
    ObjectId,
    Option<(MovePublicKey, DynamicallyLoadedObjectMetadata)>,
)>
where
    F: FnOnce(ObjectId) -> IotaResult<Option<Object>>,
{
    let public_key_field_id = dynamic_field::derive_dynamic_field_id(
        account_object_id,
        &PublicKeyFieldName::tag().into(),
        &PublicKeyFieldName::default().to_bcs_bytes(),
    )
    .map_err(|_| UserInputError::UnableToGetAccountPublicKeyId { account_object_id })?;

    match get_object(public_key_field_id)? {
        Some(object) => {
            let metadata = DynamicallyLoadedObjectMetadata::from(&object);
            let field: Field<PublicKeyFieldName, MovePublicKey> = object
                .data
                .as_struct_opt()
                .expect("dynamic field should never be a package object")
                .to_rust()
                .map_err(|_| UserInputError::InvalidAccountPublicKeyField { account_object_id })?;
            Ok((public_key_field_id, Some((field.value, metadata))))
        }
        None => Ok((public_key_field_id, None)),
    }
}

/// If the given authenticator function reference corresponds to a built-in
/// authenticator, returns the corresponding signature scheme. Otherwise,
/// returns `None`.
pub fn resolve_builtin_signature_scheme(
    authenticator_function_ref: &AuthenticatorFunctionRefV1,
) -> Option<SignatureScheme> {
    // Reject non-framework packages and modules cheaply before matching on name.
    if authenticator_function_ref.module != BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
        || authenticator_function_ref.package != IOTA_FRAMEWORK_PACKAGE_ID
    {
        return None;
    }
    match authenticator_function_ref.function.as_str() {
        ED25519_AUTHENTICATOR_FUNCTION_V1_NAME => Some(SignatureScheme::ED25519),
        SECP256K1_AUTHENTICATOR_FUNCTION_V1_NAME => Some(SignatureScheme::Secp256k1),
        SECP256R1_AUTHENTICATOR_FUNCTION_V1_NAME => Some(SignatureScheme::Secp256r1),
        MULTISIG_AUTHENTICATOR_FUNCTION_V1_NAME => Some(SignatureScheme::MultiSig),
        PASSKEY_AUTHENTICATOR_FUNCTION_V1_NAME => Some(SignatureScheme::PasskeyAuthenticator),
        _ => None,
    }
}

/// Verifies a built-in authenticator signature.
///
/// `authenticator.call_args[0]` must be a `Pure` argument containing a
/// BCS-encoded `Vec<u8>` whose bytes are a `GenericSignature` in wire format
/// (`flag || payload`). This format is consistent for all schemes: Ed25519,
/// Secp256k1, Secp256r1, MultiSig, and Passkey.
///
/// `tx_data_bytes` is the BCS-encoded `TransactionData` used to reconstruct
/// the signing message as `IntentMessage(Intent::iota_transaction(), tx_data)`.
///
/// `VerifyParams` is derived from `protocol_config`.
pub fn verify_builtin_signature(
    protocol_config: &ProtocolConfig,
    authenticator: &MoveAuthenticator,
    builtin_authenticator_data: &PreloadedBuiltinAuthenticatorData,
    tx_data_bytes: &[u8],
) -> IotaResult<()> {
    let expected_scheme = builtin_authenticator_data.expected_scheme;
    let public_key = &builtin_authenticator_data.public_key;

    let signature_bytes = extract_signature_bytes(authenticator)?;
    let signature = GenericSignature::from_bytes(&signature_bytes).map_err(|e| {
        IotaError::InvalidSignature {
            error: format!("Invalid signature bytes in built-in authenticator: {e}"),
        }
    })?;

    let actual_scheme = match &signature {
        GenericSignature::Signature(s) => s.scheme(),
        GenericSignature::MultiSig(_) => SignatureScheme::MultiSig,
        GenericSignature::PasskeyAuthenticator(_) => SignatureScheme::PasskeyAuthenticator,
        _ => {
            return Err(IotaError::InvalidSignature {
                error: "Unsupported signature type in built-in authenticator".into(),
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

    let public_key_scheme = public_key.scheme();
    if public_key_scheme != expected_scheme {
        return Err(IotaError::InvalidSignature {
            error: format!(
                "Public key scheme mismatch: expected {expected_scheme:?}, got {public_key_scheme:?}"
            ),
        });
    }

    // TODO: it would be nice to avoid this deserialization.
    let tx_data: TransactionData =
        bcs::from_bytes(tx_data_bytes).map_err(|e| IotaError::InvalidSignature {
            error: format!("Failed to deserialize transaction data: {e}"),
        })?;
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    let verify_params = VerifyParams {
        accept_passkey_in_multisig: protocol_config.accept_passkey_in_multisig(),
        additional_multisig_checks: protocol_config.additional_multisig_checks(),
    };

    let address = public_key
        .address()
        .map_err(|e| IotaError::InvalidSignature {
            error: format!("Invalid public key bytes in built-in authenticator: {e}"),
        })?;

    signature.verify_claims(&intent_msg, address, &verify_params)
}

/// Extracts the `GenericSignature` wire bytes from `call_args[0]`.
///
/// `call_args[0]` must be a `Pure` argument whose BCS payload decodes to a
/// `Vec<u8>` containing the flag-prefixed signature bytes.
fn extract_signature_bytes(authenticator: &MoveAuthenticator) -> IotaResult<Vec<u8>> {
    let call_args = authenticator.call_args();
    if call_args.len() != 1 {
        return Err(IotaError::InvalidSignature {
            error: "Built-in authenticator expects exactly one call argument (signature: \
                    vector<u8>)"
                .into(),
        });
    }
    let CallArg::Pure(arg_bytes) = &call_args[0] else {
        return Err(IotaError::InvalidSignature {
            error: "Built-in authenticator argument must be a pure vector<u8>".into(),
        });
    };
    bcs::from_bytes::<Vec<u8>>(arg_bytes).map_err(|e| IotaError::InvalidSignature {
        error: format!("Built-in authenticator signature argument BCS decode failed: {e}"),
    })
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/builtin_authenticator_functions_tests.rs"]
mod builtin_authenticator_functions_tests;
