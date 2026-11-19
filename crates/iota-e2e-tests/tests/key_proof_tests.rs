// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests of key proofs. Before `smart_account_builtin_auth`
//! attaches a key to a `SmartAccount`, the key holder signs, as an IOTA
//! personal message, the text that the SDK's `key_proof_message_v1` writes
//! for the network, the account and the nonce of the transaction that attaches
//! the key. The tests sign that text with real keys, for the chain identifier
//! of the test cluster and the nonce of a real gas coin.
//!
//! The tests use the `./tests/key_proof` Move package for an account whose
//! authenticator is not the built-in one.

use std::path::PathBuf;

use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_sdk_crypto::{
    Signer, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
    secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
};
use iota_sdk_types::{
    Address, Argument, ClaimAccountTransaction, ExecutionError, ExecutionStatus, Identifier,
    MoveAuthenticatorV1, ObjectReference, Owner, PersonalMessage, ProgrammableTransaction,
    SharedObjectReference, SignatureScheme, SmartAccountClaim, Transaction, TransactionEffects,
    TransactionKind, TypeTag, UserSignature,
    crypto::{
        Intent, MultisigAggregatedSignature, MultisigCommittee, MultisigMember,
        PasskeyAuthenticator, SimpleSignature,
    },
    key_proof::key_proof_message_v1,
};
use iota_test_transaction_builder::{TestTransactionBuilder, publish_package};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    digests::get_testnet_chain_identifier,
    effects::TransactionEffectsAPI,
    gas_coin::mock_simulation_gas_coin,
    move_package,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
};
use move_command_line_common::error_bitset::ErrorBitset;
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

const PACKAGE_PATH: &str = "tests/key_proof";
const MODULE_NAME: &str = "key_proof_account";

/// The last epoch in which the proofs of the tests are valid.
const VALID_UNTIL_EPOCH: u64 = 10;

/// The origin of the mock WebAuthn client that holds the passkeys.
const PASSKEY_ORIGIN: &str = "https://www.iota.org";

/// Abort code of `smart_account_builtin_auth::verify_key_proof` when the
/// current epoch is later than the last epoch of the proof.
const KEY_PROOF_EXPIRED_ABORT_CODE: u64 = 3;

/// Abort code of `smart_account_builtin_auth::verify_key_proof` when the proof
/// is not a signature by the key of the text for the transaction.
const INVALID_KEY_PROOF_ABORT_CODE: u64 = 4;

/// An Ed25519 key creates a `SmartAccount` with `builder_v1`, and the account
/// then sends a transaction with the built-in authenticator.
#[sim_test]
async fn ed25519_key_proof_creates_a_smart_account() {
    create_account_and_send_from_it(KeyHolder::ed25519(1)).await;
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
    let account = create_account(&test_cluster, &key).await;
    send_from_account(&test_cluster, account, &key).await;
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
    let key = KeyHolder::ed25519(4);

    let create_account_tx = |proof| {
        Transaction::new_programmable_allow_sponsor(
            sender,
            vec![sponsor_gas],
            create_account_pt(&key, proof, VALID_UNTIL_EPOCH),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
            sponsor,
        )
    };
    let message = key_proof_message_for_sender(&test_cluster, &create_account_tx(vec![]), true);
    let tx = create_account_tx(key.sign_personal_message(&message).await);

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

/// A MultiSig key can't sign a key proof: `builder_v1` refuses a MultiSig key
/// whatever it signs.
#[sim_test]
async fn multisig_key_proof_is_refused() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::new_multisig([5, 6]);

    let effects = try_create_account(&test_cluster, &key, &key, VALID_UNTIL_EPOCH, |tx| {
        key_proof_message_for_sender(&test_cluster, tx, true)
    })
    .await;

    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);
}

/// An account replaces its key with `rotate_public_key` in a transaction it
/// sends, with the new key's proof, and then sends a transaction signed by the
/// new key.
#[sim_test]
async fn rotate_public_key_attaches_a_key_with_its_key_proof() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::ed25519(7);
    let new_key = KeyHolder::Keypair(SimpleKeypair::from(Secp256r1PrivateKey::random_with(
        StdRng::from_seed([8; 32]),
    )));
    let account = create_account(&test_cluster, &key).await;

    let effects = try_rotate_public_key(&test_cluster, account, &key, &new_key, |tx| {
        key_proof_message_for_sender(&test_cluster, tx, false)
    })
    .await;
    assert_success(&effects);

    send_from_account(&test_cluster, account, &new_key).await;
}

/// An account whose authenticator is not the built-in one attaches a key and
/// switches to the built-in authenticator with `rotate_to_builtin_auth_v1`, in
/// a transaction it sends, and then sends a transaction signed by the key.
#[sim_test]
async fn rotate_to_builtin_auth_attaches_a_key_with_its_key_proof() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let account = create_account_with_custom_authenticator(&test_cluster).await;
    let key = KeyHolder::ed25519(9);

    let sender = Address::from(account.object_id);
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let rotate = |proof| {
        Transaction::new_programmable(
            sender,
            vec![gas],
            attach_key_pt("rotate_to_builtin_auth_v1", account, &key, proof),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message = key_proof_message_for_sender(&test_cluster, &rotate(vec![]), false);
    let tx = rotate(key.sign_personal_message(&message).await);
    test_cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![account_authenticator(account, vec![])],
        ))
        .await;

    send_from_account(&test_cluster, account, &key).await;
}

/// One proof passes every check for its account in its transaction, so two
/// accounts can be created with it in one transaction. A later transaction has
/// another nonce, so the same proof fails there.
#[sim_test]
async fn same_key_proof_passes_twice_in_a_transaction_and_fails_in_a_later_one() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::ed25519(10);
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;

    let create_two_accounts = |proof: Vec<u8>| {
        let mut builder = ProgrammableTransactionBuilder::new();
        add_create_account(&mut builder, &key, proof.clone(), VALID_UNTIL_EPOCH);
        add_create_account(&mut builder, &key, proof, VALID_UNTIL_EPOCH);
        Transaction::new_programmable(
            sender,
            vec![gas],
            builder.finish(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message = key_proof_message_for_sender(&test_cluster, &create_two_accounts(vec![]), true);
    let proof = key.sign_personal_message(&message).await;
    let effects = test_cluster
        .sign_and_execute_transaction(&create_two_accounts(proof.clone()))
        .await;
    let accounts = effects
        .created()
        .iter()
        .filter(|created| matches!(created.owner(), Owner::Shared(_)))
        .count();
    assert_eq!(accounts, 2);

    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let tx = Transaction::new_programmable(
        sender,
        vec![gas],
        create_account_pt(&key, proof, VALID_UNTIL_EPOCH),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let effects = execute(&test_cluster, test_cluster.sign_transaction(&tx)).await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);
}

/// `builder_v1` refuses a proof for another network, for another sender or for
/// an existing account, a proof by another key, and an expired proof.
#[sim_test]
async fn create_account_with_a_wrong_key_proof_fails() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::ed25519(11);
    let chain_identifier = chain_identifier(&test_cluster);

    let other_network = get_testnet_chain_identifier().into_bytes();
    let effects = try_create_account(&test_cluster, &key, &key, VALID_UNTIL_EPOCH, |tx| {
        key_proof_message(&other_network, tx, tx.sender(), true, VALID_UNTIL_EPOCH)
    })
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    let other_sender = test_cluster.get_address_1();
    let effects = try_create_account(&test_cluster, &key, &key, VALID_UNTIL_EPOCH, |tx| {
        key_proof_message(&chain_identifier, tx, other_sender, true, VALID_UNTIL_EPOCH)
    })
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    let effects = try_create_account(&test_cluster, &key, &key, VALID_UNTIL_EPOCH, |tx| {
        key_proof_message_for_sender(&test_cluster, tx, false)
    })
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    // An Ed25519 signature carries the signer's public key, so this fails only
    // if the check uses the attached key.
    let other_key = KeyHolder::ed25519(17);
    let effects = try_create_account(&test_cluster, &key, &other_key, VALID_UNTIL_EPOCH, |tx| {
        key_proof_message_for_sender(&test_cluster, tx, true)
    })
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    test_cluster.force_new_epoch().await;
    let effects = try_create_account(&test_cluster, &key, &key, 0, |tx| {
        key_proof_message(&chain_identifier, tx, tx.sender(), true, 0)
    })
    .await;
    assert_key_proof_abort(&effects, KEY_PROOF_EXPIRED_ABORT_CODE);
}

/// `rotate_public_key` refuses a proof for another account, and a proof for a
/// new account.
#[sim_test]
async fn rotate_public_key_with_a_wrong_key_proof_fails() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::ed25519(12);
    let new_key = KeyHolder::ed25519(13);
    let account = create_account(&test_cluster, &key).await;
    let other_account = create_account(&test_cluster, &KeyHolder::ed25519(14)).await;
    let chain_identifier = chain_identifier(&test_cluster);

    let effects = try_rotate_public_key(&test_cluster, account, &key, &new_key, |tx| {
        key_proof_message(
            &chain_identifier,
            tx,
            Address::from(other_account.object_id),
            false,
            VALID_UNTIL_EPOCH,
        )
    })
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    let effects = try_rotate_public_key(&test_cluster, account, &key, &new_key, |tx| {
        key_proof_message_for_sender(&test_cluster, tx, true)
    })
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);
}

/// A simulation reads the nonce of the gas coins it runs with. A dry run or a
/// gas estimation with other gas coins than the ones the proof was made for,
/// or with none, fails, so the client must pick the gas coins before the key
/// holder signs.
#[sim_test]
async fn simulation_with_other_gas_coins_refuses_the_key_proof() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::ed25519(15);
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let other_gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;

    let create_account_tx = |gas, proof| {
        Transaction::new_programmable(
            sender,
            gas,
            create_account_pt(&key, proof, VALID_UNTIL_EPOCH),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message =
        key_proof_message_for_sender(&test_cluster, &create_account_tx(vec![gas], vec![]), true);
    let proof = key.sign_personal_message(&message).await;

    let effects = simulate(
        &test_cluster,
        create_account_tx(vec![other_gas], proof.clone()),
    )
    .await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    let effects = simulate(&test_cluster, create_account_tx(vec![], proof.clone())).await;
    assert_key_proof_abort(&effects, INVALID_KEY_PROOF_ABORT_CODE);

    let effects = simulate(&test_cluster, create_account_tx(vec![gas], proof)).await;
    assert_success(&effects);
}

/// A simulation of a transaction without gas coins pays with a mock gas coin,
/// and reads the nonce of that coin.
#[sim_test]
async fn simulation_without_gas_coins_reads_the_nonce_of_the_mock_gas_coin() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let key = KeyHolder::ed25519(16);
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let mock_gas = mock_simulation_gas_coin(sender).object_ref();

    let create_account_tx = |gas, proof| {
        Transaction::new_programmable(
            sender,
            gas,
            create_account_pt(&key, proof, VALID_UNTIL_EPOCH),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message = key_proof_message_for_sender(
        &test_cluster,
        &create_account_tx(vec![mock_gas], vec![]),
        true,
    );
    let proof = key.sign_personal_message(&message).await;

    let effects = simulate(&test_cluster, create_account_tx(vec![], proof)).await;
    assert_success(&effects);
}

/// `ClaimAccount` creates an account without a key proof, and the account
/// then sends a transaction with the built-in authenticator.
#[sim_test]
async fn claim_account_needs_no_key_proof() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let owner = test_cluster.get_address_0();
    let keypair = test_cluster
        .wallet
        .config()
        .keystore()
        .get_key(&owner)
        .unwrap()
        .as_keypair()
        .unwrap()
        .clone();
    let public_key = keypair.public_key();
    let claim = SmartAccountClaim {
        public_key_scheme: public_key.scheme().to_u8(),
        public_key_raw_bytes: public_key.as_ref().to_vec(),
    };

    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), owner)
        .await;
    let tx = Transaction::new(
        TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim)),
        owner,
        gas,
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let effects = test_cluster.sign_and_execute_transaction(&tx).await;

    send_from_account(
        &test_cluster,
        created_account(&effects),
        &KeyHolder::Keypair(keypair),
    )
    .await;
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
    /// A MultiSig key whose members all sign.
    Multisig {
        members: Vec<Ed25519PrivateKey>,
        committee: MultisigCommittee,
    },
}

impl KeyHolder {
    /// An Ed25519 key drawn from `seed`.
    fn ed25519(seed: u8) -> Self {
        Self::Keypair(SimpleKeypair::from(Ed25519PrivateKey::random_with(
            StdRng::from_seed([seed; 32]),
        )))
    }

    /// A MultiSig key of Ed25519 members drawn from `seeds`, with weight 1
    /// each and a threshold of all of them.
    fn new_multisig<const N: usize>(seeds: [u8; N]) -> Self {
        let members: Vec<_> = seeds
            .iter()
            .map(|seed| Ed25519PrivateKey::random_with(StdRng::from_seed([*seed; 32])))
            .collect();
        let committee = MultisigCommittee::new(
            members
                .iter()
                .map(|member| MultisigMember::new(member.public_key(), 1))
                .collect(),
            N as u16,
        )
        .unwrap();
        Self::Multisig { members, committee }
    }

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
            Self::Multisig { .. } => SignatureScheme::Multisig,
        }
    }

    /// The public key without the scheme flag.
    fn raw_public_key(&self) -> Vec<u8> {
        match self {
            Self::Keypair(keypair) => keypair.public_key().as_ref().to_vec(),
            Self::Passkey { public_key, .. } => public_key.clone(),
            Self::Multisig { committee, .. } => bcs::to_bytes(committee).unwrap(),
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
            Self::Multisig { members, committee } => {
                let signatures = members
                    .iter()
                    .map(|member| {
                        let signature: SimpleSignature = member.sign(digest);
                        signature.into()
                    })
                    .collect();
                UserSignature::Multisig(
                    MultisigAggregatedSignature::new(signatures, committee.clone()).unwrap(),
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

/// Returns the text of a proof for attaching a key, in `tx`, to its sender or
/// to a new account it creates, on the network of `test_cluster`, valid until
/// `VALID_UNTIL_EPOCH`.
fn key_proof_message_for_sender(
    test_cluster: &TestCluster,
    tx: &Transaction,
    new_account: bool,
) -> String {
    key_proof_message(
        &chain_identifier(test_cluster),
        tx,
        tx.sender(),
        new_account,
        VALID_UNTIL_EPOCH,
    )
}

/// Creates a `SmartAccount` with `key` attached, sent by address 0 of
/// `test_cluster` with a valid proof, and returns it.
async fn create_account(test_cluster: &TestCluster, key: &KeyHolder) -> ObjectReference {
    let effects = try_create_account(test_cluster, key, key, VALID_UNTIL_EPOCH, |tx| {
        key_proof_message_for_sender(test_cluster, tx, true)
    })
    .await;
    assert_success(&effects);
    created_account(&effects)
}

/// Executes a transaction in which address 0 of `test_cluster` creates a
/// `SmartAccount` with `key` attached, with `signer`'s signature of `message`
/// as the proof, valid until `valid_until_epoch`. `message` gets the
/// transaction without the proof. Returns the effects, whether the transaction
/// succeeded or not.
async fn try_create_account(
    test_cluster: &TestCluster,
    key: &KeyHolder,
    signer: &KeyHolder,
    valid_until_epoch: u64,
    message: impl FnOnce(&Transaction) -> String,
) -> TransactionEffects {
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let create_account_tx = |proof| {
        Transaction::new_programmable(
            sender,
            vec![gas],
            create_account_pt(key, proof, valid_until_epoch),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message = message(&create_account_tx(vec![]));
    let tx = create_account_tx(signer.sign_personal_message(&message).await);
    execute(test_cluster, test_cluster.sign_transaction(&tx)).await
}

/// Executes a transaction in which `account`, authenticated with its key
/// `account_key`, replaces that key with `new_key` through
/// `rotate_public_key`, with `new_key`'s signature of `message` as the proof.
/// `message` gets the transaction without the proof. Returns the effects,
/// whether the transaction succeeded or not.
async fn try_rotate_public_key(
    test_cluster: &TestCluster,
    account: ObjectReference,
    account_key: &KeyHolder,
    new_key: &KeyHolder,
    message: impl FnOnce(&Transaction) -> String,
) -> TransactionEffects {
    let sender = Address::from(account.object_id);
    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let rotate = |proof| {
        Transaction::new_programmable(
            sender,
            vec![gas],
            attach_key_pt("rotate_public_key", account, new_key, proof),
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        )
    };
    let message = message(&rotate(vec![]));
    let tx = rotate(new_key.sign_personal_message(&message).await);
    let signature = account_key.sign_transaction(&tx).await;
    let authenticator = account_authenticator(
        account,
        vec![CallArg::Pure(bcs::to_bytes(&signature).unwrap())],
    );
    execute(
        test_cluster,
        TransactionEnvelope::from_user_sig_data(tx, vec![authenticator]),
    )
    .await
}

/// Creates a `SmartAccount` whose authenticator is the
/// `key_proof_account::authenticate` function of the test package, which
/// accepts every transaction, and returns it.
async fn create_account_with_custom_authenticator(test_cluster: &TestCluster) -> ObjectReference {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), PACKAGE_PATH].iter().collect();
    let package_id = publish_package(&test_cluster.wallet, path).await.object_id;
    let package_metadata = test_cluster
        .get_latest_object_ref(&move_package::derive_package_metadata_id(package_id))
        .await;
    let account_type: TypeTag = "0x2::smart_account::SmartAccount".parse().unwrap();

    let mut builder = ProgrammableTransactionBuilder::new();
    let arguments = vec![
        builder
            .obj(CallArg::ImmutableOrOwned(package_metadata))
            .unwrap(),
        builder.pure(MODULE_NAME).unwrap(),
        builder.pure("authenticate").unwrap(),
    ];
    let authenticator = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("authenticator_function"),
        Identifier::from_static("create_auth_function_ref_v1"),
        vec![account_type],
        arguments,
    );
    let account_builder = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account"),
        Identifier::from_static("builder_v1"),
        vec![],
        vec![authenticator],
    );
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account"),
        Identifier::from_static("build_v1"),
        vec![],
        vec![account_builder],
    );
    let tx = test_cluster
        .test_transaction_builder()
        .await
        .programmable(builder.finish())
        .build();
    created_account(&test_cluster.sign_and_execute_transaction(&tx).await)
}

/// Adds to `builder` the commands that make the `PublicKey` of `key`, and
/// returns it.
fn public_key_argument(builder: &mut ProgrammableTransactionBuilder, key: &KeyHolder) -> Argument {
    let scheme_function = match key.scheme() {
        SignatureScheme::Ed25519 => "ed25519",
        SignatureScheme::Secp256k1 => "secp256k1",
        SignatureScheme::Secp256r1 => "secp256r1",
        SignatureScheme::Multisig => "multisig",
        SignatureScheme::PasskeyAuthenticator => "passkey",
        scheme => panic!("no built-in authenticator for {scheme:?}"),
    };
    let scheme = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("signature_scheme"),
        Identifier::from_static(scheme_function),
        vec![],
        vec![],
    );
    let raw_public_key = builder.pure(key.raw_public_key()).unwrap();
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("public_key"),
        Identifier::from_static("create"),
        vec![],
        vec![scheme, raw_public_key],
    )
}

/// Adds to `builder` the commands that create a `SmartAccount` with `key`
/// attached, with `proof` as the key proof, valid until `valid_until_epoch`.
fn add_create_account(
    builder: &mut ProgrammableTransactionBuilder,
    key: &KeyHolder,
    proof: Vec<u8>,
    valid_until_epoch: u64,
) {
    let arguments = vec![
        public_key_argument(builder, key),
        builder.pure(proof).unwrap(),
        builder.pure(valid_until_epoch).unwrap(),
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
}

/// A programmable transaction that creates a `SmartAccount` with `key`
/// attached, with `proof` as the key proof, valid until `valid_until_epoch`.
fn create_account_pt(
    key: &KeyHolder,
    proof: Vec<u8>,
    valid_until_epoch: u64,
) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    add_create_account(&mut builder, key, proof, valid_until_epoch);
    builder.finish()
}

/// A programmable transaction that attaches `key` to `account` with
/// `smart_account_builtin_auth::<function>`, where `function` is
/// `rotate_public_key` or `rotate_to_builtin_auth_v1`, with `proof` as the key
/// proof, valid until `VALID_UNTIL_EPOCH`. `account` must send it.
fn attach_key_pt(
    function: &'static str,
    account: ObjectReference,
    key: &KeyHolder,
    proof: Vec<u8>,
) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    let arguments = vec![
        builder
            .obj(CallArg::Shared(SharedObjectReference::new(
                account.object_id,
                account.version,
                true,
            )))
            .unwrap(),
        public_key_argument(&mut builder, key),
        builder.pure(proof).unwrap(),
        builder.pure(VALID_UNTIL_EPOCH).unwrap(),
    ];
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("smart_account_builtin_auth"),
        Identifier::from_static(function),
        vec![],
        arguments,
    );
    builder.finish()
}

/// Returns the `SmartAccount` that `effects` created: the only shared object
/// they created. Its version is the initial shared version.
fn created_account(effects: &TransactionEffects) -> ObjectReference {
    *effects
        .created()
        .iter()
        .find(|created| matches!(created.owner(), Owner::Shared(_)))
        .expect("the transaction creates a SmartAccount")
        .reference()
}

/// A `MoveAuthenticator` for a transaction sent by `account`, created by
/// [`created_account`], with `call_args` as the arguments of its authenticator
/// function.
fn account_authenticator(account: ObjectReference, call_args: Vec<CallArg>) -> UserSignature {
    UserSignature::MoveAuthenticator(
        MoveAuthenticatorV1::new(
            call_args,
            vec![],
            SharedObjectReference::new(account.object_id, account.version, false),
        )
        .into(),
    )
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
    let authenticator = account_authenticator(
        account,
        vec![CallArg::Pure(bcs::to_bytes(&signature).unwrap())],
    );
    test_cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![authenticator],
        ))
        .await;
}

/// Executes `tx` and returns its effects, whether it succeeded or not.
async fn execute(test_cluster: &TestCluster, tx: TransactionEnvelope) -> TransactionEffects {
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await
        .unwrap();
    effects
}

/// Simulates `tx` on the fullnode, as a dry run does, and returns its effects.
async fn simulate(test_cluster: &TestCluster, tx: Transaction) -> TransactionEffects {
    test_cluster
        .grpc_client()
        .simulate_transaction(tx)
        .await
        .unwrap()
        .body()
        .executed_transaction()
        .unwrap()
        .effects()
        .unwrap()
        .effects()
        .unwrap()
}

fn assert_success(effects: &TransactionEffects) {
    assert!(
        matches!(effects.status(), ExecutionStatus::Success),
        "expected success, got {:?}",
        effects.status()
    );
}

/// Asserts that the transaction aborted in
/// `smart_account_builtin_auth::verify_key_proof` with `expected_code`.
fn assert_key_proof_abort(effects: &TransactionEffects, expected_code: u64) {
    let ExecutionStatus::Failure {
        error: ExecutionError::MoveAbort { location, code },
        ..
    } = effects.status()
    else {
        panic!("expected a Move abort, got {:?}", effects.status());
    };
    assert_eq!(location.module.as_str(), "smart_account_builtin_auth");
    assert_eq!(
        location.function_name.as_ref().map(Identifier::as_str),
        Some("verify_key_proof")
    );
    assert_eq!(
        ErrorBitset::from_u64(*code)
            .and_then(|bitset| bitset.error_code())
            .map(u64::from),
        Some(expected_code)
    );
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
