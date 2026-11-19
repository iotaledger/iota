// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests of key proofs. Before `smart_account_builtin_auth`
//! attaches a key to a `SmartAccount`, the key holder signs, as an IOTA
//! personal message, the text that the SDK's `key_proof_message_v1` writes
//! for the network, the account and the nonce of the transaction that attaches
//! the key. The tests sign that text with real keys, for the chain identifier
//! of the test cluster and the nonce of a real gas coin.

use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_sdk_crypto::{
    Signer, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
    secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
};
use iota_sdk_types::{
    Address, Identifier, MoveAuthenticatorV1, ObjectReference, Owner, PersonalMessage,
    ProgrammableTransaction, SharedObjectReference, SignatureScheme, Transaction,
    TransactionEffects, UserSignature,
    crypto::{Intent, PasskeyAuthenticator, SimpleSignature},
    key_proof::key_proof_message_v1,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    effects::TransactionEffectsAPI,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
};
use p256::pkcs8::DecodePublicKey;
use passkey_authenticator::{Authenticator as PasskeyClient, UserCheck, UserValidationMethod};
use passkey_client::Client as WebAuthnClient;
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
use rand::{SeedableRng, rngs::StdRng};
use test_cluster::{TestCluster, TestClusterBuilder};
use url::Url;

/// The last epoch in which the proofs of the tests are valid.
const VALID_UNTIL_EPOCH: u64 = 10;

/// The origin of the mock WebAuthn client that holds the passkeys.
const PASSKEY_ORIGIN: &str = "https://www.iota.org";

/// An Ed25519 key creates a `SmartAccount` with `builder_v1`, and the account
/// then sends a transaction with the built-in authenticator.
#[sim_test]
async fn ed25519_key_proof_creates_a_smart_account() {
    create_account_and_send_from_it(KeyHolder::Keypair(SimpleKeypair::from(
        Ed25519PrivateKey::random_with(StdRng::from_seed([1; 32])),
    )))
    .await;
}

/// As above, with a Secp256k1 key.
#[sim_test]
async fn secp256k1_key_proof_creates_a_smart_account() {
    create_account_and_send_from_it(KeyHolder::Keypair(SimpleKeypair::from(
        Secp256k1PrivateKey::random_with(StdRng::from_seed([2; 32])),
    )))
    .await;
}

/// As above, with a Secp256r1 key.
#[sim_test]
async fn secp256r1_key_proof_creates_a_smart_account() {
    create_account_and_send_from_it(KeyHolder::Keypair(SimpleKeypair::from(
        Secp256r1PrivateKey::random_with(StdRng::from_seed([3; 32])),
    )))
    .await;
}

/// As above, with a passkey.
#[sim_test]
async fn passkey_key_proof_creates_a_smart_account() {
    create_account_and_send_from_it(KeyHolder::new_passkey().await).await;
}

async fn create_account_and_send_from_it(key: KeyHolder) {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;

    let create_account = |proof| {
        Transaction::new_programmable(
            sender,
            vec![gas],
            create_account_pt(&key, proof),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let proof = new_account_proof(&test_cluster, &key, &create_account(vec![])).await;
    let effects = test_cluster
        .sign_and_execute_transaction(&create_account(proof))
        .await;

    send_from_account(&test_cluster, created_account(&effects), &key).await;
}

/// In a sponsored transaction, the nonce that a proof names is the nonce of
/// the sponsor's first gas coin.
#[sim_test]
async fn sponsored_key_proof_names_the_nonce_of_the_sponsor_gas_coin() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let sender = test_cluster.get_address_0();
    let sponsor = test_cluster.get_address_1();
    let rgp = test_cluster.get_reference_gas_price().await;
    let sponsor_gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sponsor)
        .await;
    let key = KeyHolder::Keypair(SimpleKeypair::from(Ed25519PrivateKey::random_with(
        StdRng::from_seed([4; 32]),
    )));

    let create_account = |proof| {
        Transaction::new_programmable_allow_sponsor(
            sender,
            vec![sponsor_gas],
            create_account_pt(&key, proof),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
            sponsor,
        )
    };
    let proof = new_account_proof(&test_cluster, &key, &create_account(vec![])).await;
    let tx = create_account(proof);

    let keystore = test_cluster.wallet.config().keystore();
    let signatures = [sender, sponsor]
        .iter()
        .map(|signer| {
            UserSignature::Simple(
                keystore
                    .sign_secure(signer, &tx, Intent::iota_transaction())
                    .unwrap(),
            )
        })
        .collect();
    let effects = test_cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(tx, signatures))
        .await;

    send_from_account(&test_cluster, created_account(&effects), &key).await;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A key that signs key proofs and transactions.
enum KeyHolder {
    Keypair(SimpleKeypair),
    /// A WebAuthn credential of a mock authenticator.
    Passkey {
        credential: Box<Passkey>,
        /// The compressed Secp256r1 public key of the credential.
        public_key: Vec<u8>,
    },
}

impl KeyHolder {
    /// Registers a new passkey with a mock WebAuthn authenticator.
    async fn new_passkey() -> Self {
        let origin = Url::parse(PASSKEY_ORIGIN).unwrap();
        let mut client = WebAuthnClient::new(PasskeyClient::new(
            Aaguid::new_empty(),
            None::<Passkey>,
            AlwaysApprove,
        ));
        let request = CredentialCreationOptions {
            public_key: PublicKeyCredentialCreationOptions {
                rp: PublicKeyCredentialRpEntity {
                    id: None,
                    name: origin.domain().unwrap().into(),
                },
                user: PublicKeyCredentialUserEntity {
                    id: random_vec(32).into(),
                    display_name: "Test".into(),
                    name: "test@iota.org".into(),
                },
                challenge: random_vec(32).into(),
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
        let registered = client
            .register(&origin, request, None)
            .await
            .expect("passkey registration failed");

        let verifying_key = p256::ecdsa::VerifyingKey::from_public_key_der(
            registered.response.public_key.unwrap().as_slice(),
        )
        .unwrap();
        let point = verifying_key.to_encoded_point(false);
        let prefix = if point.y().unwrap()[31] % 2 == 0 {
            0x02
        } else {
            0x03
        };
        let mut public_key = vec![prefix];
        public_key.extend_from_slice(point.x().unwrap());

        Self::Passkey {
            credential: Box::new(
                client
                    .authenticator()
                    .store()
                    .clone()
                    .expect("the authenticator stores the registered passkey"),
            ),
            public_key,
        }
    }

    fn scheme(&self) -> SignatureScheme {
        match self {
            Self::Keypair(keypair) => keypair.public_key().scheme(),
            Self::Passkey { .. } => SignatureScheme::PasskeyAuthenticator,
        }
    }

    /// The public key without the scheme flag.
    fn raw_public_key(&self) -> Vec<u8> {
        match self {
            Self::Keypair(keypair) => keypair.public_key().as_ref().to_vec(),
            Self::Passkey { public_key, .. } => public_key.clone(),
        }
    }

    /// Returns the flag-prefixed wire bytes of this key's signature of
    /// `digest`. For a passkey, `digest` is the WebAuthn challenge.
    async fn sign(&self, digest: &[u8]) -> Vec<u8> {
        match self {
            Self::Keypair(keypair) => {
                let signature: SimpleSignature = keypair.sign(digest);
                UserSignature::Simple(signature).to_bytes()
            }
            Self::Passkey {
                credential,
                public_key,
            } => {
                let origin = Url::parse(PASSKEY_ORIGIN).unwrap();
                let mut client = WebAuthnClient::new(PasskeyClient::new(
                    Aaguid::new_empty(),
                    Some(Passkey::clone(credential)),
                    AlwaysApprove,
                ));
                let request = CredentialRequestOptions {
                    public_key: PublicKeyCredentialRequestOptions {
                        challenge: Bytes::from(digest.to_vec()),
                        timeout: None,
                        rp_id: Some(origin.domain().unwrap().into()),
                        allow_credentials: None,
                        user_verification: UserVerificationRequirement::default(),
                        attestation: Default::default(),
                        attestation_formats: None,
                        extensions: None,
                        hints: None,
                    },
                };
                let authenticated = client
                    .authenticate(&origin, request, None)
                    .await
                    .expect("passkey authentication failed");

                let signature =
                    p256::ecdsa::Signature::from_der(authenticated.response.signature.as_slice())
                        .unwrap();
                let signature = signature.normalize_s().unwrap_or(signature);
                let mut simple_signature = vec![SignatureScheme::Secp256r1.to_u8()];
                simple_signature.extend_from_slice(&signature.to_bytes());
                simple_signature.extend_from_slice(public_key);

                UserSignature::PasskeyAuthenticator(
                    PasskeyAuthenticator::new(
                        authenticated
                            .response
                            .authenticator_data
                            .as_slice()
                            .to_vec(),
                        String::from_utf8_lossy(authenticated.response.client_data_json.as_slice())
                            .into(),
                        SimpleSignature::from_bytes(&simple_signature).unwrap(),
                    )
                    .unwrap(),
                )
                .to_bytes()
            }
        }
    }

    /// Returns the wire bytes of this key's signature of `message`, signed as
    /// an IOTA personal message.
    async fn sign_personal_message(&self, message: &str) -> Vec<u8> {
        self.sign(&PersonalMessage(message.as_bytes().into()).signing_digest())
            .await
    }

    /// Returns the wire bytes of this key's signature of `tx`.
    async fn sign_transaction(&self, tx: &Transaction) -> Vec<u8> {
        self.sign(&tx.signing_digest()).await
    }
}

/// Returns the chain identifier of the test cluster: the digest of its genesis
/// checkpoint.
fn chain_identifier(test_cluster: &TestCluster) -> [u8; 32] {
    *test_cluster
        .swarm
        .config()
        .genesis
        .checkpoint()
        .digest()
        .bytes()
}

/// Returns the text that a key holder signs to approve attaching their key to
/// `account` in `tx`, on the network of `chain_identifier`.
///
/// The nonce comes from the gas payment of `tx` only, so a transaction built
/// without the proof has the nonce of the same transaction built with it.
fn key_proof_message(
    chain_identifier: &[u8; 32],
    tx: &Transaction,
    account: Address,
    new_account: bool,
    valid_until_epoch: u64,
) -> String {
    key_proof_message_v1(
        chain_identifier,
        &account,
        new_account,
        &tx.nonce()
            .expect("a transaction with a gas coin has a nonce"),
        valid_until_epoch,
    )
}

/// Returns `key`'s proof for a new account that the sender of `tx` creates in
/// `tx`, valid until `VALID_UNTIL_EPOCH`.
async fn new_account_proof(
    test_cluster: &TestCluster,
    key: &KeyHolder,
    tx: &Transaction,
) -> Vec<u8> {
    let message = key_proof_message(
        &chain_identifier(test_cluster),
        tx,
        tx.sender(),
        true,
        VALID_UNTIL_EPOCH,
    );
    key.sign_personal_message(&message).await
}

/// A programmable transaction that creates a `SmartAccount` with `key`
/// attached, with `proof` as the key proof, valid until `VALID_UNTIL_EPOCH`.
fn create_account_pt(key: &KeyHolder, proof: Vec<u8>) -> ProgrammableTransaction {
    let scheme_function = match key.scheme() {
        SignatureScheme::Ed25519 => "ed25519",
        SignatureScheme::Secp256k1 => "secp256k1",
        SignatureScheme::Secp256r1 => "secp256r1",
        SignatureScheme::Multisig => "multisig",
        SignatureScheme::PasskeyAuthenticator => "passkey",
        scheme => panic!("no built-in authenticator for {scheme:?}"),
    };

    let mut builder = ProgrammableTransactionBuilder::new();
    let scheme = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("signature_scheme"),
        Identifier::from_static(scheme_function),
        vec![],
        vec![],
    );
    let raw_public_key = builder.pure(key.raw_public_key()).unwrap();
    let public_key = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("public_key"),
        Identifier::from_static("create"),
        vec![],
        vec![scheme, raw_public_key],
    );
    let arguments = vec![
        public_key,
        builder.pure(proof).unwrap(),
        builder.pure(VALID_UNTIL_EPOCH).unwrap(),
    ];
    let account_builder = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account_builtin_auth"),
        Identifier::from_static("builder_v1"),
        vec![],
        arguments,
    );
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account"),
        Identifier::from_static("build_v1"),
        vec![],
        vec![account_builder],
    );
    builder.finish()
}

/// Returns the `SmartAccount` that `effects` created: the only shared object
/// they created.
fn created_account(effects: &TransactionEffects) -> ObjectReference {
    *effects
        .created()
        .iter()
        .find(|created| matches!(created.owner(), Owner::Shared(_)))
        .expect("the transaction creates a SmartAccount")
        .reference()
}

/// Sends a transaction from `account`, authenticated by the built-in
/// authenticator with `key`'s signature, and checks that it succeeds.
async fn send_from_account(test_cluster: &TestCluster, account: ObjectReference, key: &KeyHolder) {
    let sender = Address::from(account.object_id);
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let tx = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, Address::ZERO)
        .build();

    let signature = key.sign_transaction(&tx).await;
    let authenticator = UserSignature::MoveAuthenticator(
        MoveAuthenticatorV1::new(
            vec![CallArg::Pure(bcs::to_bytes(&signature).unwrap())],
            vec![],
            SharedObjectReference::new(account.object_id, account.version, false),
        )
        .into(),
    );
    test_cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![authenticator],
        ))
        .await;
}

/// A `UserValidationMethod` that approves every request, for the mock
/// WebAuthn authenticator.
struct AlwaysApprove;

#[async_trait::async_trait]
impl UserValidationMethod for AlwaysApprove {
    type PasskeyItem = Passkey;

    async fn check_user<'a>(
        &self,
        _credential: Option<&'a Self::PasskeyItem>,
        _presence: bool,
        _verification: bool,
    ) -> Result<UserCheck, Ctap2Error> {
        Ok(UserCheck {
            presence: true,
            verification: true,
        })
    }

    fn is_verification_enabled(&self) -> Option<bool> {
        Some(true)
    }

    fn is_presence_enabled(&self) -> bool {
        true
    }
}
