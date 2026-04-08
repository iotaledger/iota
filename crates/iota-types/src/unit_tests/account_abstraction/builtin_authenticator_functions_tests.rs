// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::{
    hash::{HashFunction, Sha256},
    rsa::{Base64UrlUnpadded, Encoding as _},
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::crypto::{Intent, IntentMessage};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID,
    account_abstraction::{
        authenticator_function::AuthenticatorFunctionRefV1,
        builtin_authenticator_functions::{
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME, ED25519_AUTHENTICATOR_FUNCTION_V1_NAME,
            MULTISIG_AUTHENTICATOR_FUNCTION_V1_NAME, PASSKEY_AUTHENTICATOR_FUNCTION_V1_NAME,
            PreloadedBuiltinAuthenticatorData, SECP256K1_AUTHENTICATOR_FUNCTION_V1_NAME,
            SECP256R1_AUTHENTICATOR_FUNCTION_V1_NAME, builtin_signature_scheme,
            ed25519_authenticator_function_ref_v1, load_builtin_public_key,
            multisig_authenticator_function_ref_v1, passkey_authenticator_function_ref_v1,
            secp256k1_authenticator_function_ref_v1, secp256r1_authenticator_function_ref_v1,
            verify_builtin_signature,
        },
        public_key::MovePublicKey,
    },
    base_types::{IotaAddress, ObjectID, SequenceNumber},
    crypto::{IotaKeyPair, PublicKey, Signature, SignatureScheme, get_key_pair_from_rng},
    digests::ObjectDigest,
    error::IotaError,
    move_authenticator::MoveAuthenticator,
    multisig::{MultiSig, MultiSigPublicKey},
    passkey_authenticator::{PasskeyAuthenticator, to_signing_message},
    signature::GenericSignature,
    transaction::{CallArg, ObjectArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionData},
};

// === builtin_signature_scheme() ===

#[test]
fn builtin_scheme_ed25519() {
    let reference = ed25519_authenticator_function_ref_v1();
    assert_eq!(
        builtin_signature_scheme(&reference),
        Some(SignatureScheme::ED25519)
    );
}

#[test]
fn builtin_scheme_secp256k1() {
    let reference = secp256k1_authenticator_function_ref_v1();
    assert_eq!(
        builtin_signature_scheme(&reference),
        Some(SignatureScheme::Secp256k1)
    );
}

#[test]
fn builtin_scheme_secp256r1() {
    let reference = secp256r1_authenticator_function_ref_v1();
    assert_eq!(
        builtin_signature_scheme(&reference),
        Some(SignatureScheme::Secp256r1)
    );
}

#[test]
fn builtin_scheme_multisig() {
    let reference = multisig_authenticator_function_ref_v1();
    assert_eq!(
        builtin_signature_scheme(&reference),
        Some(SignatureScheme::MultiSig)
    );
}

#[test]
fn builtin_scheme_passkey() {
    let reference = passkey_authenticator_function_ref_v1();
    assert_eq!(
        builtin_signature_scheme(&reference),
        Some(SignatureScheme::PasskeyAuthenticator)
    );
}

#[test]
fn builtin_scheme_none_for_wrong_package() {
    let reference = make_ref(
        IOTA_SYSTEM_PACKAGE_ID,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str(),
        ED25519_AUTHENTICATOR_FUNCTION_V1_NAME,
    );
    assert_eq!(builtin_signature_scheme(&reference), None);
}

#[test]
fn builtin_scheme_none_for_wrong_module() {
    let reference = make_ref(
        IOTA_FRAMEWORK_PACKAGE_ID,
        "other_module",
        ED25519_AUTHENTICATOR_FUNCTION_V1_NAME,
    );
    assert_eq!(builtin_signature_scheme(&reference), None);
}

#[test]
fn builtin_scheme_none_for_unknown_function() {
    let reference = make_ref(
        IOTA_FRAMEWORK_PACKAGE_ID,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.as_str(),
        "unknown_authenticator_function_ref_v1",
    );
    assert_eq!(builtin_signature_scheme(&reference), None);
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

// === load_builtin_public_key() ===

#[test]
fn load_builtin_public_key_queries_correct_field_id() {
    let account_id = ObjectID::random();
    let mut queried_id = None;
    let (returned_field_id, result) = load_builtin_public_key(account_id, |id| {
        queried_id = Some(id);
        Ok(None)
    })
    .unwrap();

    // The closure must have been called with the derived field ID.
    assert_eq!(queried_id, Some(returned_field_id));
    // The field ID is derived, so it differs from the account object ID.
    assert_ne!(returned_field_id, account_id);
    assert!(result.is_none());
}

#[test]
fn load_builtin_public_key_propagates_get_object_error() {
    let account_id = ObjectID::random();
    let result = load_builtin_public_key(account_id, |_| {
        Err(IotaError::Storage("simulated storage error".into()))
    });
    assert!(matches!(
        result.unwrap_err(),
        IotaError::Storage(msg) if msg == "simulated storage error"
    ));
}

// === verify_builtin_signature() happy path ===

#[test]
fn verify_builtin_signature_ok_ed25519() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    let (authenticator, data, tx_data_bytes) = signed_authenticator(&key_pair);
    assert!(
        verify_builtin_signature(&protocol_config(), &authenticator, &data, &tx_data_bytes).is_ok()
    );
}

#[test]
fn verify_builtin_signature_ok_secp256k1() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let (authenticator, data, tx_data_bytes) = signed_authenticator(&key_pair);
    assert!(
        verify_builtin_signature(&protocol_config(), &authenticator, &data, &tx_data_bytes).is_ok()
    );
}

#[test]
fn verify_builtin_signature_ok_secp256r1() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);
    let (authenticator, data, tx_data_bytes) = signed_authenticator(&key_pair);
    assert!(
        verify_builtin_signature(&protocol_config(), &authenticator, &data, &tx_data_bytes).is_ok()
    );
}

#[test]
fn verify_builtin_signature_ok_multisig() {
    let mut rng = seeded_rng();
    let kp1 = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    let kp2 = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let multisig_public_key =
        MultiSigPublicKey::new(vec![kp1.public(), kp2.public()], vec![1, 1], 1).unwrap();
    let sender = IotaAddress::from(&multisig_public_key);

    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    let sig1: GenericSignature = Signature::new_secure(&intent_msg, &kp1).into();
    let multisig = GenericSignature::MultiSig(
        MultiSig::combine(vec![sig1], multisig_public_key.clone()).unwrap(),
    );
    let wire = multisig.as_ref().to_vec();

    let builtin_data = PreloadedBuiltinAuthenticatorData {
        expected_scheme: SignatureScheme::MultiSig,
        public_key: MovePublicKey::new(
            SignatureScheme::MultiSig,
            bcs::to_bytes(&multisig_public_key).unwrap(),
        )
        .unwrap(),
    };
    let authenticator = make_authenticator(vec![CallArg::Pure(bcs::to_bytes(&wire).unwrap())]);

    assert!(
        verify_builtin_signature(
            &protocol_config(),
            &authenticator,
            &builtin_data,
            &tx_data_bytes
        )
        .is_ok()
    );
}

#[test]
fn verify_builtin_signature_ok_passkey() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);

    // Passkey address is derived from the Secp256r1 key under the Passkey flag.
    let raw_public_key = key_pair.public().as_ref().to_vec();
    let passkey_public_key =
        PublicKey::try_from_bytes(SignatureScheme::PasskeyAuthenticator, &raw_public_key).unwrap();
    let sender = IotaAddress::from(&passkey_public_key);

    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    // Challenge = Blake2b256 hash of the BCS-encoded intent message.
    let challenge = to_signing_message(&intent_msg);
    let challenge_b64 = Base64UrlUnpadded::encode_string(&challenge);

    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{challenge_b64}","origin":"https://iota.org","crossOrigin":false}}"#
    );
    let authenticator_data = vec![0xAB];

    // WebAuthn message: authenticator_data || sha256(client_data_json).
    let client_data_hash = Sha256::digest(client_data_json.as_bytes()).digest;
    let mut webauthn_msg = authenticator_data.clone();
    webauthn_msg.extend_from_slice(&client_data_hash);

    // Sign the WebAuthn message with the Secp256r1 key.
    let user_sig = Signature::new_hashed(&webauthn_msg, &key_pair);
    let passkey =
        PasskeyAuthenticator::new_for_testing(authenticator_data, client_data_json, user_sig)
            .unwrap();

    let generic_sig = GenericSignature::PasskeyAuthenticator(passkey);
    let wire = generic_sig.as_ref().to_vec();

    let builtin_data = PreloadedBuiltinAuthenticatorData {
        expected_scheme: SignatureScheme::PasskeyAuthenticator,
        public_key: MovePublicKey::new(SignatureScheme::PasskeyAuthenticator, raw_public_key)
            .unwrap(),
    };
    let authenticator = make_authenticator(vec![CallArg::Pure(bcs::to_bytes(&wire).unwrap())]);

    assert!(
        verify_builtin_signature(
            &protocol_config(),
            &authenticator,
            &builtin_data,
            &tx_data_bytes
        )
        .is_ok()
    );
}

// === verify_builtin_signature() errors ===

#[test]
fn verify_builtin_signature_error_no_call_args() {
    let config = protocol_config();
    let authenticator = make_authenticator(vec![]);
    let data = ed25519_data(&mut seeded_rng());

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("exactly one call argument")
    ));
}

#[test]
fn verify_builtin_signature_error_too_many_call_args() {
    let config = protocol_config();
    let mut rng = seeded_rng();
    let authenticator =
        make_authenticator(vec![ed25519_sig_arg(&mut rng), ed25519_sig_arg(&mut rng)]);
    let data = ed25519_data(&mut rng);

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("exactly one call argument")
    ));
}

#[test]
fn verify_builtin_signature_error_non_pure_arg() {
    let config = protocol_config();
    let object_arg = CallArg::Object(ObjectArg::ImmOrOwnedObject((
        ObjectID::ZERO,
        SequenceNumber::default(),
        ObjectDigest::MIN,
    )));
    let authenticator = make_authenticator(vec![object_arg]);
    let data = ed25519_data(&mut seeded_rng());

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("pure vector<u8>")
    ));
}

#[test]
fn verify_builtin_signature_error_invalid_bcs_in_pure_arg() {
    let config = protocol_config();
    // Empty bytes cannot be decoded as BCS Vec<u8> (needs at least a length byte).
    let authenticator = make_authenticator(vec![CallArg::Pure(vec![])]);
    let data = ed25519_data(&mut seeded_rng());

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("BCS decode failed")
    ));
}

#[test]
fn verify_builtin_signature_error_invalid_sig_bytes() {
    let config = protocol_config();
    // BCS-encodes a Vec<u8> with an unrecognized scheme flag so GenericSignature
    // rejects it.
    let garbage: Vec<u8> = vec![0xAB, 0xCD, 0xEF];
    let authenticator = make_authenticator(vec![CallArg::Pure(bcs::to_bytes(&garbage).unwrap())]);
    let data = ed25519_data(&mut seeded_rng());

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid signature bytes in built-in authenticator")
    ));
}

#[test]
fn verify_builtin_signature_error_unsupported_sig_type() {
    let config = protocol_config();
    // A MoveAuthenticator in wire format parses as
    // GenericSignature::MoveAuthenticator, which hits the unsupported branch in
    // verify_builtin_signature.
    let inner_auth = make_authenticator(vec![]);
    let move_auth_wire: Vec<u8> = inner_auth.as_ref().to_vec();
    let authenticator =
        make_authenticator(vec![CallArg::Pure(bcs::to_bytes(&move_auth_wire).unwrap())]);
    let data = ed25519_data(&mut seeded_rng());

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Unsupported signature type in built-in authenticator")
    ));
}

#[test]
fn verify_builtin_signature_error_sig_scheme_mismatch() {
    let config = protocol_config();
    let mut rng = seeded_rng();
    // Signature is ED25519 but the authenticator function expects Secp256k1.
    let authenticator = make_authenticator(vec![ed25519_sig_arg(&mut rng)]);
    let secp256k1_key_pair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let data = PreloadedBuiltinAuthenticatorData {
        expected_scheme: SignatureScheme::Secp256k1,
        public_key: MovePublicKey::from(&secp256k1_key_pair),
    };

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Signature scheme mismatch")
                && error.contains("Secp256k1")
                && error.contains("ED25519")
    ));
}

#[test]
fn verify_builtin_signature_error_public_key_scheme_mismatch() {
    let config = protocol_config();
    let mut rng = seeded_rng();
    // Signature scheme matches expected (ED25519), but the stored public key is
    // Secp256k1.
    let authenticator = make_authenticator(vec![ed25519_sig_arg(&mut rng)]);
    let secp256k1_key_pair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let data = PreloadedBuiltinAuthenticatorData {
        expected_scheme: SignatureScheme::ED25519,
        public_key: MovePublicKey::from(&secp256k1_key_pair),
    };

    assert!(matches!(
        verify_builtin_signature(&config, &authenticator, &data, &[]).unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Public key scheme mismatch")
                && error.contains("ED25519")
                && error.contains("Secp256k1")
    ));
}

// === Helpers ===

fn make_ref(package: ObjectID, module: &str, function: &str) -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package,
        module: module.to_string(),
        function: function.to_string(),
    }
}

fn seeded_rng() -> StdRng {
    StdRng::from_seed([0; 32])
}

fn protocol_config() -> ProtocolConfig {
    ProtocolConfig::get_for_max_version_UNSAFE()
}

fn make_authenticator(call_args: Vec<CallArg>) -> MoveAuthenticator {
    let object_to_authenticate = CallArg::Object(ObjectArg::ImmOrOwnedObject((
        ObjectID::ZERO,
        SequenceNumber::default(),
        ObjectDigest::MIN,
    )));
    MoveAuthenticator::new_v1(call_args, vec![], object_to_authenticate)
}

/// Returns a Pure `CallArg` containing a BCS-encoded ED25519 signature wire
/// bytes (flag || sig || pk) suitable for `verify_builtin_signature`.
fn ed25519_sig_arg(rng: &mut StdRng) -> CallArg {
    let key_pair = IotaKeyPair::Ed25519(get_key_pair_from_rng(rng).1);
    let sig = Signature::new_hashed(b"test", &key_pair);
    let wire = GenericSignature::Signature(sig).as_ref().to_vec();
    CallArg::Pure(bcs::to_bytes(&wire).unwrap())
}

/// Returns `PreloadedBuiltinAuthenticatorData` for an ED25519 key drawn from
/// `rng`.
fn ed25519_data(rng: &mut StdRng) -> PreloadedBuiltinAuthenticatorData {
    let key_pair = IotaKeyPair::Ed25519(get_key_pair_from_rng(rng).1);
    PreloadedBuiltinAuthenticatorData {
        expected_scheme: SignatureScheme::ED25519,
        public_key: MovePublicKey::from(&key_pair),
    }
}

/// Constructs a minimal dummy `TransactionData` for `sender`.
fn dummy_tx_data(sender: IotaAddress) -> TransactionData {
    let gas_ref = (ObjectID::ZERO, SequenceNumber::default(), ObjectDigest::MIN);
    TransactionData::new_transfer_iota(
        IotaAddress::ZERO,
        sender,
        None,
        gas_ref,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        1000,
    )
}

/// Creates a fully signed `(MoveAuthenticator,
/// PreloadedBuiltinAuthenticatorData, tx_data_bytes)` triple for `key_pair`,
/// ready to be passed to `verify_builtin_signature`.
fn signed_authenticator(
    key_pair: &IotaKeyPair,
) -> (
    MoveAuthenticator,
    PreloadedBuiltinAuthenticatorData,
    Vec<u8>,
) {
    let scheme = key_pair.public().scheme();
    let sender = IotaAddress::from(&key_pair.public());
    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();

    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);
    let sig = GenericSignature::Signature(Signature::new_secure(&intent_msg, key_pair));
    let wire = sig.as_ref().to_vec();

    let builtin_data = PreloadedBuiltinAuthenticatorData {
        expected_scheme: scheme,
        public_key: MovePublicKey::from(key_pair),
    };
    let authenticator = make_authenticator(vec![CallArg::Pure(bcs::to_bytes(&wire).unwrap())]);

    (authenticator, builtin_data, tx_data_bytes)
}
