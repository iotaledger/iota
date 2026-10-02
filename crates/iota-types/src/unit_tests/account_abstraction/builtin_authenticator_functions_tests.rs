// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::{
    hash::{HashFunction, Sha256},
    rsa::{Base64UrlUnpadded, Encoding as _},
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_crypto::{
    Signer, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
    secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
};
use iota_sdk_types::{
    Address, CommandArgumentError, MoveAuthenticator, MoveAuthenticatorV1, ObjectDigest, ObjectId,
    ObjectReference, SignatureScheme, SimpleSignature, Transaction, TypeTag, UserSignature,
    Version,
    crypto::{
        Intent, IntentMessage, MultisigAggregatedSignature, MultisigCommittee, MultisigMember,
        PasskeyAuthenticator,
    },
};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID,
    account_abstraction::{
        authenticator_function::AuthenticatorFunctionRefV1,
        builtin_authenticator_functions::{
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME, ED25519_AUTHENTICATOR_FUNCTION_V1_NAME,
            MULTISIG_AUTHENTICATOR_FUNCTION_V1_NAME, PASSKEY_AUTHENTICATOR_FUNCTION_V1_NAME,
            SECP256K1_AUTHENTICATOR_FUNCTION_V1_NAME, SECP256R1_AUTHENTICATOR_FUNCTION_V1_NAME,
            ed25519_authenticator_function_ref_v1, extract_signature_bytes,
            multisig_authenticator_function_ref_v1, passkey_authenticator_function_ref_v1,
            resolve_builtin_signature_scheme, secp256k1_authenticator_function_ref_v1,
            secp256r1_authenticator_function_ref_v1, verify_builtin_signature,
        },
        public_key::MovePublicKey,
        signature_scheme::MoveSignatureScheme,
    },
    crypto::PublicKey,
    error::{ExecutionErrorKind, IotaError},
    signature::VerifyParams,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
};

// === resolve_builtin_signature_scheme() ===

#[test]
fn builtin_scheme_ed25519() {
    let reference = ed25519_authenticator_function_ref_v1();
    assert_eq!(
        resolve_builtin_signature_scheme(&reference),
        Some(move_scheme(SignatureScheme::Ed25519))
    );
}

#[test]
fn builtin_scheme_secp256k1() {
    let reference = secp256k1_authenticator_function_ref_v1();
    assert_eq!(
        resolve_builtin_signature_scheme(&reference),
        Some(move_scheme(SignatureScheme::Secp256k1))
    );
}

#[test]
fn builtin_scheme_secp256r1() {
    let reference = secp256r1_authenticator_function_ref_v1();
    assert_eq!(
        resolve_builtin_signature_scheme(&reference),
        Some(move_scheme(SignatureScheme::Secp256r1))
    );
}

#[test]
fn builtin_scheme_multisig() {
    let reference = multisig_authenticator_function_ref_v1();
    assert_eq!(
        resolve_builtin_signature_scheme(&reference),
        Some(move_scheme(SignatureScheme::Multisig))
    );
}

#[test]
fn builtin_scheme_passkey() {
    let reference = passkey_authenticator_function_ref_v1();
    assert_eq!(
        resolve_builtin_signature_scheme(&reference),
        Some(move_scheme(SignatureScheme::PasskeyAuthenticator))
    );
}

#[test]
fn builtin_scheme_none_for_wrong_package() {
    let reference = make_ref(
        IOTA_SYSTEM_PACKAGE_ID,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str(),
        ED25519_AUTHENTICATOR_FUNCTION_V1_NAME,
    );
    assert_eq!(resolve_builtin_signature_scheme(&reference), None);
}

#[test]
fn builtin_scheme_none_for_wrong_module() {
    let reference = make_ref(
        IOTA_FRAMEWORK_PACKAGE_ID,
        "other_module",
        ED25519_AUTHENTICATOR_FUNCTION_V1_NAME,
    );
    assert_eq!(resolve_builtin_signature_scheme(&reference), None);
}

#[test]
fn builtin_scheme_none_for_unknown_function() {
    let reference = make_ref(
        IOTA_FRAMEWORK_PACKAGE_ID,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str(),
        "unknown_authenticator_function_ref_v1",
    );
    assert_eq!(resolve_builtin_signature_scheme(&reference), None);
}

// === authenticator function ref constructors ===

#[test]
fn ed25519_ref_has_correct_fields() {
    let reference = ed25519_authenticator_function_ref_v1();

    assert_eq!(reference.package, IOTA_FRAMEWORK_PACKAGE_ID);
    assert_eq!(
        reference.module,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
    );
    assert_eq!(reference.function, ED25519_AUTHENTICATOR_FUNCTION_V1_NAME);
}

#[test]
fn secp256k1_ref_has_correct_fields() {
    let reference = secp256k1_authenticator_function_ref_v1();
    assert_eq!(reference.package, IOTA_FRAMEWORK_PACKAGE_ID);
    assert_eq!(
        reference.module,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
    );
    assert_eq!(reference.function, SECP256K1_AUTHENTICATOR_FUNCTION_V1_NAME);
}

#[test]
fn secp256r1_ref_has_correct_fields() {
    let reference = secp256r1_authenticator_function_ref_v1();
    assert_eq!(reference.package, IOTA_FRAMEWORK_PACKAGE_ID);
    assert_eq!(
        reference.module,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
    );
    assert_eq!(reference.function, SECP256R1_AUTHENTICATOR_FUNCTION_V1_NAME);
}

#[test]
fn multisig_ref_has_correct_fields() {
    let reference = multisig_authenticator_function_ref_v1();
    assert_eq!(reference.package, IOTA_FRAMEWORK_PACKAGE_ID);
    assert_eq!(
        reference.module,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
    );
    assert_eq!(reference.function, MULTISIG_AUTHENTICATOR_FUNCTION_V1_NAME);
}

#[test]
fn passkey_ref_has_correct_fields() {
    let reference = passkey_authenticator_function_ref_v1();
    assert_eq!(reference.package, IOTA_FRAMEWORK_PACKAGE_ID);
    assert_eq!(
        reference.module,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str()
    );
    assert_eq!(reference.function, PASSKEY_AUTHENTICATOR_FUNCTION_V1_NAME);
}

// === extract_signature_bytes() ===

#[test]
fn extract_signature_bytes_ok() {
    let signature = vec![1u8, 2, 3];
    let authenticator = make_authenticator(vec![CallArg::pure(&signature)], vec![]);
    assert_eq!(extract_signature_bytes(&authenticator).unwrap(), signature);
}

#[test]
fn extract_signature_bytes_error_type_args() {
    let authenticator = make_authenticator(vec![CallArg::pure(&vec![1u8])], vec![TypeTag::U8]);

    assert_eq!(
        extract_signature_bytes(&authenticator).unwrap_err().kind(),
        &ExecutionErrorKind::TypeArityMismatch
    );
}

#[test]
fn extract_signature_bytes_error_no_call_args() {
    let authenticator = make_authenticator(vec![], vec![]);

    assert_eq!(
        extract_signature_bytes(&authenticator).unwrap_err().kind(),
        &ExecutionErrorKind::ArityMismatch
    );
}

#[test]
fn extract_signature_bytes_error_too_many_call_args() {
    let authenticator = make_authenticator(
        vec![CallArg::pure(&vec![1u8]), CallArg::pure(&vec![2u8])],
        vec![],
    );

    assert_eq!(
        extract_signature_bytes(&authenticator).unwrap_err().kind(),
        &ExecutionErrorKind::ArityMismatch
    );
}

#[test]
fn extract_signature_bytes_error_non_pure_arg() {
    let object_arg = CallArg::ImmutableOrOwned(ObjectReference::new(
        ObjectId::ZERO,
        Version::default(),
        ObjectDigest::MIN,
    ));
    let authenticator = make_authenticator(vec![object_arg], vec![]);

    assert_eq!(
        extract_signature_bytes(&authenticator).unwrap_err().kind(),
        &ExecutionErrorKind::CommandArgumentError {
            argument: 1,
            kind: CommandArgumentError::TypeMismatch,
        }
    );
}

#[test]
fn extract_signature_bytes_error_invalid_bcs_in_pure_arg() {
    // Empty bytes cannot be decoded as BCS Vec<u8> (needs at least a length byte).
    let authenticator = make_authenticator(vec![CallArg::Pure(vec![])], vec![]);

    assert_eq!(
        extract_signature_bytes(&authenticator).unwrap_err().kind(),
        &ExecutionErrorKind::CommandArgumentError {
            argument: 1,
            kind: CommandArgumentError::InvalidBcsBytes,
        }
    );
}

// === verify_builtin_signature() happy path ===

#[test]
fn verify_builtin_signature_ok_ed25519() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut seeded_rng()));
    let verifier = builtin_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_signature_ok_secp256k1() {
    let key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(&mut seeded_rng()));
    let verifier = builtin_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_signature_ok_secp256r1() {
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut seeded_rng()));
    let verifier = builtin_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_signature_ok_multisig() {
    let mut rng = seeded_rng();
    let kp1 = Ed25519PrivateKey::random_with(&mut rng);
    let kp2 = Secp256k1PrivateKey::random_with(&mut rng);
    let multisig_public_key = MultisigCommittee::new(
        vec![
            MultisigMember::new(kp1.public_key(), 1),
            MultisigMember::new(kp2.public_key(), 1),
        ],
        1,
    )
    .unwrap();
    let sender = Address::from(&multisig_public_key);

    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    let msg = intent_msg.signing_digest();
    let sig1: SimpleSignature = kp1.sign(&msg);
    let multisig = UserSignature::Multisig(
        MultisigAggregatedSignature::new(vec![sig1.into()], multisig_public_key.clone()).unwrap(),
    );

    let verifier = BuiltinSignatureVerifier {
        public_key: MovePublicKey::new(
            SignatureScheme::Multisig,
            bcs::to_bytes(&multisig_public_key).unwrap(),
        )
        .unwrap(),
        signature: multisig.to_bytes(),
        tx_data_bytes,
    };
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_signature_ok_passkey() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut rng));

    // Passkey address is derived from the Secp256r1 key under the Passkey flag.
    let raw_public_key = key_pair.public_key().as_ref().to_vec();
    let passkey_public_key =
        PublicKey::try_from_bytes(SignatureScheme::PasskeyAuthenticator, &raw_public_key).unwrap();
    let sender = Address::from(&passkey_public_key);

    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    // Challenge = Blake2b256 hash of the BCS-encoded intent message.
    let challenge = intent_msg.signing_digest();
    let challenge_b64 = Base64UrlUnpadded::encode_string(challenge.as_ref());

    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{challenge_b64}","origin":"https://iota.org","crossOrigin":false}}"#
    );
    let authenticator_data = vec![0xAB];

    // WebAuthn message: authenticator_data || sha256(client_data_json).
    let client_data_hash = Sha256::digest(client_data_json.as_bytes()).digest;
    let mut webauthn_msg = authenticator_data.clone();
    webauthn_msg.extend_from_slice(&client_data_hash);

    // Sign the WebAuthn message with the Secp256r1 key.
    let user_sig: SimpleSignature = key_pair.sign(&webauthn_msg);
    let passkey =
        PasskeyAuthenticator::new(authenticator_data, client_data_json, user_sig).unwrap();

    let verifier = BuiltinSignatureVerifier {
        public_key: MovePublicKey::new(SignatureScheme::PasskeyAuthenticator, raw_public_key)
            .unwrap(),
        signature: UserSignature::PasskeyAuthenticator(passkey).to_bytes(),
        tx_data_bytes,
    };
    assert!(verifier.verify().is_ok());
}

// === verify_builtin_signature() errors ===

#[test]
fn verify_builtin_signature_error_invalid_sig_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    // An unrecognized scheme flag so UserSignature rejects it.
    verifier.signature = vec![0xAB, 0xCD, 0xEF];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid signature bytes in built-in authenticator")
    ));
}

#[test]
fn verify_builtin_signature_error_unsupported_sig_type() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    // A MoveAuthenticator in wire format parses as
    // UserSignature::MoveAuthenticator, which hits the unsupported branch in
    // verify_builtin_signature.
    verifier.signature = make_authenticator(vec![], vec![]).to_bytes();

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Unsupported signature type in built-in authenticator")
    ));
}

#[test]
fn verify_builtin_signature_error_sig_scheme_mismatch() {
    let mut rng = seeded_rng();
    // Signature is ED25519 but the public key is Secp256k1.
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let secp256k1_key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(&mut rng));
    let mut verifier = builtin_signature_verifier(&key_pair);
    verifier.public_key = MovePublicKey::from(&secp256k1_key_pair);

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Signature scheme mismatch")
                && error.contains("Secp256k1")
                && error.contains("Ed25519")
    ));
}

#[test]
fn verify_builtin_signature_error_invalid_public_key_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    // Construct a MovePublicKey with 1 raw byte for ED25519 (requires 32) by
    // bypassing new() via BCS deserialization.
    let mut bcs_bytes = vec![SignatureScheme::Ed25519.to_u8()];
    bcs_bytes.extend(bcs::to_bytes(&vec![0u8; 1]).unwrap());
    verifier.public_key = bcs::from_bytes(&bcs_bytes).unwrap();

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid public key bytes in built-in authenticator")
    ));
}

#[test]
fn verify_builtin_signature_error_other_public_key() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let other_key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let mut verifier = builtin_signature_verifier(&key_pair);
    verifier.public_key = MovePublicKey::from(&other_key_pair);

    assert!(verifier.verify().is_err());
}

#[test]
fn verify_builtin_signature_error_invalid_tx_data_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    verifier.tx_data_bytes = vec![];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Failed to deserialize transaction data")
    ));
}

// === Helpers ===

/// Calls `verify_builtin_signature` with the arguments for a signed
/// transaction.
struct BuiltinSignatureVerifier {
    public_key: MovePublicKey,
    signature: Vec<u8>,
    tx_data_bytes: Vec<u8>,
}

impl BuiltinSignatureVerifier {
    fn verify(&self) -> Result<(), IotaError> {
        let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        let verify_params = VerifyParams::new(
            protocol_config.accept_passkey_in_multisig(),
            protocol_config.additional_multisig_checks(),
        );
        verify_builtin_signature(
            &verify_params,
            &self.public_key,
            &self.signature,
            &self.tx_data_bytes,
        )
    }
}

fn make_ref(package: ObjectId, module: &str, function: &str) -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package,
        module: module.to_string(),
        function: function.to_string(),
    }
}

fn seeded_rng() -> StdRng {
    StdRng::from_seed([0; 32])
}

fn make_authenticator(call_args: Vec<CallArg>, type_args: Vec<TypeTag>) -> MoveAuthenticator {
    MoveAuthenticatorV1::new_with_immutable_account_object(
        call_args,
        type_args,
        ObjectReference::new(ObjectId::ZERO, Version::default(), ObjectDigest::MIN),
    )
    .into()
}

/// Constructs a minimal dummy `Transaction` for `sender`.
fn dummy_tx_data(sender: Address) -> Transaction {
    let gas_ref = ObjectReference::new(ObjectId::ZERO, Version::default(), ObjectDigest::MIN);
    Transaction::new_transfer_iota(
        Address::ZERO,
        sender,
        None,
        gas_ref,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        1000,
    )
}

/// Signs a dummy transaction sent by the address of `key_pair` and returns the
/// `BuiltinSignatureVerifier` for it.
fn builtin_signature_verifier(key_pair: &SimpleKeypair) -> BuiltinSignatureVerifier {
    let public_key = key_pair.public_key();
    let tx_data = dummy_tx_data(public_key.derive_address());
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();

    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);
    let sig: SimpleSignature = key_pair.sign(&intent_msg.signing_digest());

    BuiltinSignatureVerifier {
        public_key: MovePublicKey::from(key_pair),
        signature: UserSignature::Simple(sig).to_bytes(),
        tx_data_bytes,
    }
}

fn move_scheme(scheme: SignatureScheme) -> MoveSignatureScheme {
    MoveSignatureScheme::try_from(scheme).unwrap()
}
