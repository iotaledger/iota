// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;

use iota_core::authority_client::AuthorityAPI;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_crypto::{secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair};
use iota_sdk_types::crypto::{
    Intent, IntentMessage, PasskeyAuthenticator, PasskeyPublicKey, PublicKey, Secp256r1PublicKey,
    Secp256r1Signature, SimpleSignature,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::IotaAddress,
    error::{IotaError, IotaResult},
    multisig::{MultiSig, MultiSigPublicKey, MultisigMember},
    passkey_authenticator::to_signing_message,
    signature::GenericSignature,
    transaction::Transaction,
    utils::{make_upgraded_multisig_tx, multisig_keys},
};
use p256::pkcs8::DecodePublicKey;
use passkey_authenticator::{Authenticator, UserCheck, UserValidationMethod};
use passkey_client::Client;
use passkey_types::{
    Bytes, Passkey,
    ctap2::{Aaguid, Ctap2Error},
    rand::random_vec,
    webauthn::{
        AttestationConveyancePreference, CredentialCreationOptions, CredentialRequestOptions,
        PublicKeyCredentialCreationOptions, PublicKeyCredentialParameters,
        PublicKeyCredentialRequestOptions, PublicKeyCredentialRpEntity, PublicKeyCredentialType,
        PublicKeyCredentialUserEntity, UserVerificationRequirement,
    },
};
use test_cluster::{TestCluster, TestClusterBuilder};
use url::Url;

async fn do_upgraded_multisig_test() -> IotaResult {
    let test_cluster = TestClusterBuilder::new().build().await;
    let tx = make_upgraded_multisig_tx();

    test_cluster
        .authority_aggregator()
        .authority_clients
        .values()
        .next()
        .unwrap()
        .authority_client()
        .handle_transaction(tx, Some(SocketAddr::new([127, 0, 0, 1].into(), 0)))
        .await
        .map(|_| ())
}

async fn create_credential_and_sign_test_tx_with_passkey_multisig(
    test_cluster: &TestCluster,
    sender: Option<IotaAddress>,
    change_intent: bool,
    change_tx: bool,
) -> Transaction {
    // set up authenticator and client
    let my_aaguid = Aaguid::new_empty();
    let user_validation_method = MyUserValidationMethod {};
    let store: Option<Passkey> = None;
    let my_authenticator = Authenticator::new(my_aaguid, store, user_validation_method);
    let mut my_client = Client::new(my_authenticator);
    let origin = Url::parse("https://www.iota.org").unwrap();

    // Create credential.
    let challenge_bytes_from_rp: Bytes = random_vec(32).into();
    let user_entity = PublicKeyCredentialUserEntity {
        id: random_vec(32).into(),
        display_name: "Johnny Passkey".into(),
        name: "jpasskey@example.org".into(),
    };
    let request = CredentialCreationOptions {
        public_key: PublicKeyCredentialCreationOptions {
            rp: PublicKeyCredentialRpEntity {
                id: None, // Leaving the ID as None means use the effective domain
                name: origin.domain().unwrap().into(),
            },
            user: user_entity,
            challenge: challenge_bytes_from_rp,
            pub_key_cred_params: vec![PublicKeyCredentialParameters {
                ty: PublicKeyCredentialType::PublicKey,
                alg: coset::iana::Algorithm::ES256,
            }],
            timeout: None,
            exclude_credentials: None,
            authenticator_selection: None,
            hints: None,
            attestation: AttestationConveyancePreference::None,
            attestation_formats: None,
            extensions: None,
        },
    };
    let my_webauthn_credential = my_client.register(&origin, request, None).await.unwrap();
    let verifying_key = p256::ecdsa::VerifyingKey::from_public_key_der(
        my_webauthn_credential
            .response
            .public_key
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    // Derive compact pubkey from DER format.
    let encoded_point = verifying_key.to_encoded_point(false);
    let x = encoded_point.x();
    let y = encoded_point.y();
    let prefix = if y.unwrap()[31] % 2 == 0 { 0x02 } else { 0x03 };
    let mut pk_bytes = [prefix; Secp256r1PublicKey::LENGTH];
    pk_bytes[1..].copy_from_slice(x.unwrap());
    let passkey_pk = PasskeyPublicKey::new(Secp256r1PublicKey::new(pk_bytes));

    // Construct a multisig with 4 pks (ed25519, secp256k1, secp256r1, passkey) with
    // threshold = 1.
    let (kp1, kp2, kp3) = multisig_keys();
    let pk0 = kp1.public_key(); // ed25519
    let pk1 = kp2.public_key(); // secp256k1
    let pk2 = kp3.public_key(); // secp256r1

    let multisig_pk = MultiSigPublicKey::new(
        vec![
            MultisigMember::new(pk0, 1),
            MultisigMember::new(pk1, 1),
            MultisigMember::new(pk2, 1),
            MultisigMember::new(passkey_pk, 1),
        ],
        1,
    )
    .unwrap();

    // Compute iota address as sender, fund gas and make a test transaction.
    let sender = match sender {
        Some(s) => s,
        None => IotaAddress::from(&multisig_pk),
    };

    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), sender)
        .await;
    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());

    // Compute the challenge = blake2b_hash(intent_msg(tx)) for passkey credential
    // request. If change_intent, mangle the intent bytes. If change_tx, mangle the
    // hashed tx bytes.
    let passkey_challenge = if change_intent {
        to_signing_message(&IntentMessage::new(
            Intent::personal_message(),
            intent_msg.value.clone(),
        ))
        .to_vec()
    } else if change_tx {
        random_vec(32)
    } else {
        to_signing_message(&intent_msg).to_vec()
    };

    // Request a signature from passkey with challenge set to passkey_digest.
    let credential_request = CredentialRequestOptions {
        public_key: PublicKeyCredentialRequestOptions {
            challenge: Bytes::from(passkey_challenge),
            timeout: None,
            rp_id: Some(String::from(origin.domain().unwrap())),
            allow_credentials: None,
            user_verification: UserVerificationRequirement::default(),
            attestation: Default::default(),
            attestation_formats: None,
            extensions: None,
            hints: None,
        },
    };

    let authenticated_cred = my_client
        .authenticate(&origin, credential_request, None)
        .await
        .unwrap();

    // Parse signature from der format in response and normalize it to lower s.
    let sig_bytes_der = authenticated_cred.response.signature.as_slice();
    let sig = p256::ecdsa::Signature::from_der(sig_bytes_der).unwrap();
    let sig_bytes = sig.normalize_s().unwrap_or(sig).to_bytes();

    // Parse authenticator_data and client_data_json from response.
    let authenticator_data = authenticated_cred.response.authenticator_data.as_slice();
    let client_data_json = authenticated_cred.response.client_data_json.as_slice();

    let sig = PasskeyAuthenticator::new(
        authenticator_data.to_vec(),
        String::from_utf8(client_data_json.to_vec()).unwrap(),
        SimpleSignature::new_secp256r1(
            Secp256r1Signature::new(sig_bytes.into()),
            Secp256r1PublicKey::new(pk_bytes),
        ),
    )
    .unwrap();

    let multisig =
        GenericSignature::MultiSig(MultiSig::new(vec![sig.into()], multisig_pk.clone()).unwrap());

    Transaction::from_generic_sig_data(tx_data, vec![multisig])
}

struct MyUserValidationMethod {}

#[async_trait::async_trait]
impl UserValidationMethod for MyUserValidationMethod {
    type PasskeyItem = Passkey;

    async fn check_user<'a>(
        &self,
        _credential: Option<&'a Passkey>,
        presence: bool,
        verification: bool,
    ) -> Result<UserCheck, Ctap2Error> {
        Ok(UserCheck {
            presence,
            verification,
        })
    }

    fn is_verification_enabled(&self) -> Option<bool> {
        Some(true)
    }

    fn is_presence_enabled(&self) -> bool {
        true
    }
}

#[sim_test]
async fn test_upgraded_multisig_feature_allow() {
    let res = do_upgraded_multisig_test().await;

    // we didn't make a real transaction with a valid object, but we verify that we
    // pass the feature gate.
    assert!(matches!(res.unwrap_err(), IotaError::UserInput { .. }));
}

#[sim_test]
async fn test_multisig_e2e() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let context = &test_cluster.wallet;
    let rgp = test_cluster.get_reference_gas_price().await;

    let (kp1, kp2, kp3) = multisig_keys();
    let pk0 = kp1.public_key(); // ed25519
    let pk1 = kp2.public_key(); // secp256k1
    let pk2 = kp3.public_key(); // secp256r1

    let multisig_pk = MultiSigPublicKey::new_unchecked(
        vec![
            MultisigMember::new(pk0, 1),
            MultisigMember::new(pk1, 1),
            MultisigMember::new(pk2, 1),
        ],
        2,
    );

    let keys: [SimpleKeypair; 3] = [kp1.into(), kp2.into(), kp3.into()];
    let multisig_addr = IotaAddress::from(&multisig_pk);

    // fund wallet and get a gas object to use later.
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), multisig_addr)
        .await;

    // 1. sign with key 0 and 1 executes successfully.
    let tx1 = TestTransactionBuilder::new(multisig_addr, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(multisig_pk.clone(), &[&keys[0], &keys[1]], 0b011);
    let res = context.execute_transaction_must_succeed(tx1).await;
    assert!(res.status_ok().unwrap());

    // 2. sign with key 1 and 2 executes successfully.
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), multisig_addr)
        .await;
    let tx2 = TestTransactionBuilder::new(multisig_addr, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(multisig_pk.clone(), &[&keys[1], &keys[2]], 0b110);
    let res = context.execute_transaction_must_succeed(tx2).await;
    assert!(res.status_ok().unwrap());

    // 3. signature 2 and 1 swapped fails to execute.
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), multisig_addr)
        .await;
    let tx3 = TestTransactionBuilder::new(multisig_addr, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(multisig_pk.clone(), &[&keys[2], &keys[1]], 0b110);
    let res = context.execute_transaction_may_fail(tx3).await;
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Invalid sig for pk=AQIOF81ZOeRrGWZBlozXWZELold+J/pz/eOHbbm+xbzrKw==")
    );

    // 4. sign with key 0 only is below threshold, fails to execute.
    let tx4 = TestTransactionBuilder::new(multisig_addr, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(multisig_pk.clone(), &[&keys[0]], 0b001);
    let res = context.execute_transaction_may_fail(tx4).await;
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Insufficient weight=1 threshold=2")
    );

    // 5. multisig with no single sig fails to execute. An empty multisig is
    // rejected at signature deserialization time (the SDK's `validate()` returns
    // `InvalidSignatureNumber`), which surfaces as a generic invalid-signature
    // error rather than the detailed multisig message.
    let tx5 = TestTransactionBuilder::new(multisig_addr, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(multisig_pk.clone(), &[], 0b001);
    let res = context.execute_transaction_may_fail(tx5).await;
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Invalid signature was given to the function")
    );

    // 6. multisig two dup sigs fails to execute.
    let tx6 = TestTransactionBuilder::new(multisig_addr, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(multisig_pk.clone(), &[&keys[0], &keys[0]], 0b011);
    let res = context.execute_transaction_may_fail(tx6).await;
    assert!(
        res.as_ref()
            .unwrap_err()
            .to_string()
            .contains("Invalid sig for pk")
    );
    assert!(
        res.as_ref()
            .unwrap_err()
            .to_string()
            .contains("error=signature/pubkey type mismatch")
    );

    // 7. mismatch pks in sig with multisig address fails to execute.
    let pk3: PublicKey = Secp256r1PrivateKey::generate(rand::thread_rng())
        .public_key()
        .into();
    let wrong_multisig_pk = MultiSigPublicKey::new_unchecked(
        vec![
            MultisigMember::new(pk0, 1),
            MultisigMember::new(pk1, 1),
            MultisigMember::new(pk3.clone(), 1),
        ],
        2,
    );
    let wrong_sender = IotaAddress::from(&wrong_multisig_pk);
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20000000000), wrong_sender)
        .await;
    let tx7 = TestTransactionBuilder::new(wrong_sender, gas, rgp)
        .transfer_iota(None, IotaAddress::ZERO)
        .build_and_sign_multisig(wrong_multisig_pk.clone(), &[&keys[0], &keys[2]], 0b101);
    let res = context.execute_transaction_may_fail(tx7).await;
    assert!(
        res.unwrap_err()
            .to_string()
            .contains(format!("Invalid sig for pk={}", pk3.to_base64()).as_str())
    );
}

#[sim_test]
async fn test_multisig_passkey_feature_deny() {
    // if feature disabled, fails to execute.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_accept_passkey_in_multisig_for_testing(false);
        config
    });
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(15000)
        .build()
        .await;
    let tx =
        create_credential_and_sign_test_tx_with_passkey_multisig(&test_cluster, None, false, false)
            .await;
    // feature flag disabled fails latest multisig tx.
    let res = test_cluster.wallet.execute_transaction_may_fail(tx).await;
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Passkey sig not supported inside multisig")
    );
}

#[sim_test]
async fn test_multisig_passkey_scenarios() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_accept_passkey_in_multisig_for_testing(true);
        config
    });
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(15000)
        .build()
        .await;
    let tx =
        create_credential_and_sign_test_tx_with_passkey_multisig(&test_cluster, None, false, false)
            .await;
    let res = test_cluster.wallet.execute_transaction_may_fail(tx).await;
    assert!(res.is_ok());

    // wrong sender fails to verify
    let tx2 = create_credential_and_sign_test_tx_with_passkey_multisig(
        &test_cluster,
        Some(IotaAddress::ZERO),
        false,
        false,
    )
    .await;
    let res = test_cluster.wallet.execute_transaction_may_fail(tx2).await;
    assert!(res.is_err());

    // wrong intent fails to verify
    let tx3 =
        create_credential_and_sign_test_tx_with_passkey_multisig(&test_cluster, None, true, false)
            .await;
    let res = test_cluster.wallet.execute_transaction_may_fail(tx3).await;
    assert!(res.is_err());

    // wrong challenge mismatch tx fails to verify
    let tx4 =
        create_credential_and_sign_test_tx_with_passkey_multisig(&test_cluster, None, false, true)
            .await;
    let res = test_cluster.wallet.execute_transaction_may_fail(tx4).await;
    assert!(res.is_err());
}
