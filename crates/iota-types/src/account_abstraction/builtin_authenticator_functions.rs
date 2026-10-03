// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{
    Address, CommandArgumentError, Identifier, MoveAuthenticator, SignatureScheme, StructTag,
    Transaction, UserSignature,
    crypto::{Intent, IntentMessage},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    account_abstraction::{
        authenticator_function::AuthenticatorFunctionRefV1, public_key::MovePublicKey,
    },
    error::{ExecutionError, ExecutionErrorKind, IotaError, IotaResult},
    move_authenticator::MoveAuthenticatorExt,
    signature::{AuthenticatorTrait, VerifyParams},
    transaction::CallArg,
};

pub const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: Identifier =
    Identifier::from_static("builtin_authenticator_functions");

pub const PUBLIC_KEY_FIELD_NAME_STRUCT_NAME: Identifier =
    Identifier::from_static("PublicKeyFieldName");

pub const BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME: &str = "builtin_authenticator_v1";

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
pub fn verify_builtin_signature(
    verify_params: &VerifyParams,
    public_key: &MovePublicKey,
    signature_bytes: &[u8],
    tx_data_bytes: &[u8],
) -> IotaResult<()> {
    let expected_scheme = public_key.scheme();

    let signature =
        UserSignature::from_bytes(signature_bytes).map_err(|e| IotaError::InvalidSignature {
            error: format!("Invalid signature bytes in built-in authenticator: {e}"),
        })?;

    let actual_scheme = match signature.scheme() {
        scheme @ (SignatureScheme::Ed25519
        | SignatureScheme::Secp256k1
        | SignatureScheme::Secp256r1
        | SignatureScheme::Multisig
        | SignatureScheme::PasskeyAuthenticator) => scheme,
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

    let address = public_key
        .address()
        .map_err(|e| IotaError::InvalidSignature {
            error: format!("Invalid public key bytes in built-in authenticator: {e}"),
        })?;

    // TODO: it would be nice to avoid this deserialization.
    let tx_data: Transaction =
        bcs::from_bytes(tx_data_bytes).map_err(|e| IotaError::InvalidSignature {
            error: format!("Failed to deserialize transaction data: {e}"),
        })?;
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    signature.verify_claims(&intent_msg, address, verify_params)
}

/// Extracts the `UserSignature` wire bytes from `call_args[0]`.
///
/// `call_args[0]` must be a `Pure` argument whose BCS payload decodes to a
/// `Vec<u8>` containing the flag-prefixed signature bytes, and `authenticator`
/// must carry no type arguments.
///
/// Returns the error a Move call to a built-in authenticator function reports
/// for the same mistake; the signature is that function's second argument,
/// after the account.
pub fn extract_signature_bytes(
    authenticator: &MoveAuthenticator,
) -> Result<Vec<u8>, ExecutionError> {
    if !authenticator.type_args().is_empty() {
        return Err(ExecutionError::new_with_source(
            ExecutionErrorKind::TypeArityMismatch,
            "Built-in authenticator expects no type arguments",
        ));
    }
    let call_args = authenticator.call_args();
    if call_args.len() != 1 {
        return Err(ExecutionError::new_with_source(
            ExecutionErrorKind::ArityMismatch,
            "Built-in authenticator expects exactly one call argument (signature: vector<u8>)",
        ));
    }
    let CallArg::Pure(arg_bytes) = &call_args[0] else {
        return Err(signature_argument_error(
            CommandArgumentError::TypeMismatch,
            "Built-in authenticator argument must be a pure vector<u8>".to_string(),
        ));
    };
    bcs::from_bytes::<Vec<u8>>(arg_bytes).map_err(|e| {
        signature_argument_error(
            CommandArgumentError::InvalidBcsBytes,
            format!("Built-in authenticator signature argument BCS decode failed: {e}"),
        )
    })
}

fn signature_argument_error(kind: CommandArgumentError, message: String) -> ExecutionError {
    ExecutionError::new_with_source(ExecutionErrorKind::command_argument_error(kind, 1), message)
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/builtin_authenticator_functions_tests.rs"]
mod builtin_authenticator_functions_tests;
