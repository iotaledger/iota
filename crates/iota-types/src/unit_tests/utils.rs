// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::traits::KeyPair as KeypairTraits;
use iota_sdk_crypto::{
    Signer, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
    secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
};
use iota_sdk_types::{
    Address, ObjectId, SenderSignedTransaction, SimpleSignature, Transaction, TransactionKind,
    UserSignature,
    crypto::{Intent, MultisigAggregatedSignature, MultisigCommittee, MultisigMember},
};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    base_types::{dbg_addr, random_object_ref},
    committee::Committee,
    crypto::{AccountKeyPair, AuthorityKeyPair, AuthorityPublicKeyBytes, get_key_pair_from_rng},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, TransactionEnvelope},
};

pub fn make_committee_key<R>(rand: &mut R) -> (Vec<AuthorityKeyPair>, Committee)
where
    R: rand::CryptoRng + rand::RngCore,
{
    make_committee_key_num(4, rand)
}

pub fn make_committee_key_num<R>(num: usize, rand: &mut R) -> (Vec<AuthorityKeyPair>, Committee)
where
    R: rand::CryptoRng + rand::RngCore,
{
    let mut authorities: BTreeMap<AuthorityPublicKeyBytes, u64> = BTreeMap::new();
    let mut keys = Vec::new();

    for _ in 0..num {
        let (_, inner_authority_key): (_, AuthorityKeyPair) = get_key_pair_from_rng(rand);
        authorities.insert(
            // address
            AuthorityPublicKeyBytes::from(inner_authority_key.public()),
            // voting right
            1,
        );
        keys.push(inner_authority_key);
    }

    let committee = Committee::new_for_testing_with_normalized_voting_power(0, authorities);
    (keys, committee)
}

// Creates a fake sender-signed transaction for testing. This transaction will
// not actually work.
pub fn create_fake_transaction() -> TransactionEnvelope {
    let sender_key = AccountKeyPair::generate(rand::thread_rng());
    let sender = sender_key.public_key().derive_address();
    let recipient = dbg_addr(2);
    let object_id = ObjectId::random();
    let object = Object::immutable_with_id_for_testing(object_id);
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(recipient, None);
        builder.finish()
    };
    let tx = Transaction::new_programmable(
        sender,
        vec![object.object_ref()],
        pt,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, // gas price is 1
        1,
    );
    to_sender_signed_transaction(tx, &sender_key)
}

pub fn make_transaction_data(sender: Address) -> Transaction {
    let object =
        Object::immutable_with_id_for_testing(ObjectId::generate(StdRng::from_seed([0; 32])));
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(dbg_addr(2), None);
        builder.finish()
    };
    Transaction::new_programmable(
        sender,
        vec![object.object_ref()],
        pt,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, // gas price is 1
        1,
    )
}

/// Make sponsored [`Transaction`] with a transfer-IOTA programmable
/// transaction and a random gas object, for use in tests.
pub fn make_sponsored_transaction_data(sender: Address, sponsor: Address) -> Transaction {
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(dbg_addr(2), None);
        builder.finish()
    };
    Transaction::new_with_gas_coins_allow_sponsor(
        TransactionKind::new_programmable(pt),
        sender,
        vec![random_object_ref()],
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, // gas price is 1
        1,
        sponsor,
    )
}

/// Make a user signed transaction with the given sender and its keypair. This
/// is not verified or signed by authority.
pub fn make_transaction(sender: Address, kp: &SimpleKeypair) -> TransactionEnvelope {
    let data = make_transaction_data(sender);
    TransactionEnvelope::from_data_and_signer(data, vec![kp])
}

// This is used to sign transaction with signer using default Intent.
pub fn to_sender_signed_transaction(
    tx: Transaction,
    signer: &impl Signer<SimpleSignature>,
) -> TransactionEnvelope {
    to_sender_signed_transaction_with_multi_signers(tx, vec![signer])
}

pub fn to_sender_signed_transaction_with_optional_sponsor(
    tx: Transaction,
    sender_signature: UserSignature,
    sponsor_signer_opt: Option<&impl Signer<SimpleSignature>>,
) -> TransactionEnvelope {
    let mut signatures = vec![sender_signature];
    if let Some(sponsor) = sponsor_signer_opt {
        let sponsor_sig = TransactionEnvelope::signature_from_signer(
            tx.clone(),
            Intent::iota_transaction(),
            sponsor,
        )
        .into();
        signatures.push(sponsor_sig);
    };

    TransactionEnvelope::from_user_sig_data(tx, signatures)
}

pub fn to_sender_signed_transaction_with_multi_signers(
    tx: Transaction,
    signers: Vec<&impl Signer<SimpleSignature>>,
) -> TransactionEnvelope {
    TransactionEnvelope::from_data_and_signer(tx, signers)
}

pub fn keys() -> Vec<SimpleKeypair> {
    let (kp1, kp2, kp3) = multisig_keys();
    vec![kp1.into(), kp2.into(), kp3.into()]
}

pub fn multisig_keys() -> (Ed25519PrivateKey, Secp256k1PrivateKey, Secp256r1PrivateKey) {
    let mut seed = StdRng::from_seed([0; 32]);
    let kp1 = Ed25519PrivateKey::generate(&mut seed);
    let kp2 = Secp256k1PrivateKey::generate(&mut seed);
    let kp3 = Secp256r1PrivateKey::generate(&mut seed);

    (kp1, kp2, kp3)
}

pub fn make_upgraded_multisig_tx() -> TransactionEnvelope {
    let (kp1, kp2, kp3) = multisig_keys();
    let pk1 = kp1.public_key();
    let pk2 = kp2.public_key();
    let pk3 = kp3.public_key();

    let multisig_pk = MultisigCommittee::new(
        vec![
            MultisigMember::new(pk1, 1),
            MultisigMember::new(pk2, 1),
            MultisigMember::new(pk3, 1),
        ],
        2,
    )
    .unwrap();
    let addr = Address::from(&multisig_pk);
    let tx = make_transaction(addr, &SimpleKeypair::from(kp1.clone()));

    let msg = tx.transaction().signing_digest();
    let sig1: SimpleSignature = kp1.sign(&msg);
    let sig2: SimpleSignature = kp2.sign(&msg);

    // Any 2 of 3 signatures verifies ok.
    let multi_sig1 =
        MultisigAggregatedSignature::new(vec![sig1.into(), sig2.into()], multisig_pk).unwrap();
    TransactionEnvelope::new(SenderSignedTransaction::new(
        tx.transaction().clone(),
        vec![UserSignature::Multisig(multi_sig1)],
    ))
}

/// Make a sponsored transaction where both sender and sponsor sign with regular
/// (Ed25519) signatures, for use in tests.
///
/// Returns the transaction together with the sender's and sponsor's addresses
/// so callers can locate each signature within the transaction.
pub fn make_sponsored_regular_sig_tx() -> (TransactionEnvelope, Address, Address) {
    let sender_kp = AccountKeyPair::generate(rand::thread_rng());
    let sender = sender_kp.public_key().derive_address();
    let sponsor_kp = AccountKeyPair::generate(rand::thread_rng());
    let sponsor = sponsor_kp.public_key().derive_address();
    let tx_data = make_sponsored_transaction_data(sender, sponsor);
    let sender_sig: UserSignature = TransactionEnvelope::signature_from_signer(
        tx_data.clone(),
        Intent::iota_transaction(),
        &sender_kp,
    )
    .into();
    let tx =
        to_sender_signed_transaction_with_optional_sponsor(tx_data, sender_sig, Some(&sponsor_kp));
    (tx, sender, sponsor)
}

mod move_authenticator {
    use fastcrypto::hash::HashFunction;
    use iota_sdk_types::{
        Address, Digest, MoveAuthenticator, MoveAuthenticatorV1, SenderSignedTransaction,
        SharedObjectReference, UserSignature,
    };

    use crate::{
        crypto::DefaultHash,
        object::OBJECT_START_VERSION,
        transaction::TransactionEnvelope,
        utils::{make_sponsored_transaction_data, make_transaction_data},
    };

    /// Make a transaction signed with `MoveAuthenticator` for testing.
    pub fn make_move_authenticator_tx(address: Address) -> TransactionEnvelope {
        let data = make_transaction_data(address);
        let (authenticator, _) = make_move_authenticator_sig(address);
        TransactionEnvelope::new(SenderSignedTransaction::new(data, vec![authenticator]))
    }

    /// Build a [`UserSignature::MoveAuthenticator`] and the underlying
    /// [`MoveAuthenticator`] for the given address, for use in tests.
    ///
    /// There is no real Move account behind this address.
    ///
    /// TODO: if it is necessary, AA accounts need to be supported properly in
    /// the `AuthorityState` used for testing.
    pub fn make_move_authenticator_sig(address: Address) -> (UserSignature, MoveAuthenticator) {
        let authenticator =
            MoveAuthenticator::from(MoveAuthenticatorV1::new_with_shared_account_object(
                vec![],
                vec![],
                SharedObjectReference::new(address.into(), OBJECT_START_VERSION, false),
            ));
        let sig = UserSignature::MoveAuthenticator(authenticator.clone());
        (sig, authenticator)
    }

    /// Make a sponsored transaction where both sender and sponsor sign with
    /// [`MoveAuthenticator`], for use in tests.
    ///
    /// Returns the transaction together with the sender's and sponsor's
    /// [`MoveAuthenticator`] so callers can independently verify the expected
    /// auth digests.
    pub fn make_sponsored_move_authenticator_tx(
        sender_addr: Address,
        sponsor_addr: Address,
    ) -> (TransactionEnvelope, MoveAuthenticator, MoveAuthenticator) {
        let (sender_sig, sender_auth) = make_move_authenticator_sig(sender_addr);
        let (sponsor_sig, sponsor_auth) = make_move_authenticator_sig(sponsor_addr);
        let tx_data = make_sponsored_transaction_data(sender_addr, sponsor_addr);
        let tx = TransactionEnvelope::new(SenderSignedTransaction::new(
            tx_data,
            vec![sender_sig, sponsor_sig],
        ));
        (tx, sender_auth, sponsor_auth)
    }

    /// Compute the Blake2b256 hash of the serialized (flag-prefixed) bytes of a
    /// [`UserSignature`], matching the digest used for
    /// non-[`MoveAuthenticator`] signatures by
    /// [`UserSignature::auth_digest`].
    pub fn blake2b256_of_sig(sig: &UserSignature) -> Digest {
        let mut hasher = DefaultHash::default();
        hasher.update(sig.to_bytes());
        Digest::new(hasher.finalize().into())
    }
}

pub use move_authenticator::*;

mod passkey {
    use iota_sdk_crypto::secp256r1::Secp256r1PrivateKey;
    use iota_sdk_types::crypto::PasskeyAuthenticator;

    use super::*;

    /// Build a [`UserSignature::PasskeyAuthenticator`] backed by a
    /// freshly-generated Secp256r1 key pair, for use in tests.
    ///
    /// The challenge field is 32 zero-bytes encoded as base64url without
    /// padding, satisfying the length requirement without needing a real
    /// WebAuthn round-trip.
    pub fn make_passkey_authenticator_sig() -> UserSignature {
        let r1_kp = Secp256r1PrivateKey::generate(rand::thread_rng());
        let user_sig: SimpleSignature = r1_kp.sign(&[0u8; 32]);
        let client_data_json = r#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://test.iota.org"}"#;
        let passkey =
            PasskeyAuthenticator::new(vec![], client_data_json.to_string(), user_sig).unwrap();
        UserSignature::PasskeyAuthenticator(passkey)
    }
}

pub use passkey::*;
