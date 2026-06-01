// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::traits::KeyPair as KeypairTraits;
use iota_sdk_types::crypto::{Intent, IntentMessage};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    IotaAddress,
    base_types::{ObjectID, dbg_addr, random_object_ref},
    committee::Committee,
    crypto::{
        AccountKeyPair, AuthorityKeyPair, AuthorityPublicKeyBytes, IotaKeyPair, Signature, Signer,
        get_key_pair, get_key_pair_from_rng,
    },
    multisig::{MultiSig, MultiSigPublicKey},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{
        SenderSignedData, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, Transaction, TransactionData,
        TransactionDataAPI, TransactionKind,
    },
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
pub fn create_fake_transaction() -> Transaction {
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = dbg_addr(2);
    let object_id = ObjectID::random();
    let object = Object::immutable_with_id_for_testing(object_id);
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(recipient, None);
        builder.finish()
    };
    let data = TransactionData::new_programmable(
        sender,
        vec![object.compute_object_reference()],
        pt,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, // gas price is 1
        1,
    );
    to_sender_signed_transaction(data, &sender_key)
}

pub fn make_transaction_data(sender: IotaAddress) -> TransactionData {
    let object =
        Object::immutable_with_id_for_testing(ObjectID::generate(StdRng::from_seed([0; 32])));
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(dbg_addr(2), None);
        builder.finish()
    };
    TransactionData::new_programmable(
        sender,
        vec![object.compute_object_reference()],
        pt,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, // gas price is 1
        1,
    )
}

/// Make sponsored [`TransactionData`] with a transfer-IOTA programmable
/// transaction and a random gas object, for use in tests.
pub fn make_sponsored_transaction_data(
    sender: IotaAddress,
    sponsor: IotaAddress,
) -> TransactionData {
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(dbg_addr(2), None);
        builder.finish()
    };
    TransactionData::new_with_gas_coins_allow_sponsor(
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
pub fn make_transaction(sender: IotaAddress, kp: &IotaKeyPair) -> Transaction {
    let data = make_transaction_data(sender);
    Transaction::from_data_and_signer(data, vec![kp])
}

// This is used to sign transaction with signer using default Intent.
pub fn to_sender_signed_transaction(
    data: TransactionData,
    signer: &dyn Signer<Signature>,
) -> Transaction {
    to_sender_signed_transaction_with_multi_signers(data, vec![signer])
}

pub fn to_sender_signed_transaction_with_optional_sponsor(
    data: TransactionData,
    sender_signature: GenericSignature,
    sponsor_signer_opt: Option<&dyn Signer<Signature>>,
) -> Transaction {
    let mut signatures = vec![sender_signature];
    if let Some(sponsor) = sponsor_signer_opt {
        let sponsor_sig =
            Transaction::signature_from_signer(data.clone(), Intent::iota_transaction(), sponsor)
                .into();
        signatures.push(sponsor_sig);
    };

    Transaction::from_generic_sig_data(data, signatures)
}

pub fn to_sender_signed_transaction_with_multi_signers(
    data: TransactionData,
    signers: Vec<&dyn Signer<Signature>>,
) -> Transaction {
    Transaction::from_data_and_signer(data, signers)
}

pub fn keys() -> Vec<IotaKeyPair> {
    let mut seed = StdRng::from_seed([0; 32]);
    let kp1: IotaKeyPair = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut seed).1);
    let kp2: IotaKeyPair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut seed).1);
    let kp3: IotaKeyPair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut seed).1);
    vec![kp1, kp2, kp3]
}

pub fn make_upgraded_multisig_tx() -> Transaction {
    let keys = keys();
    let pk1 = &keys[0].public();
    let pk2 = &keys[1].public();
    let pk3 = &keys[2].public();

    let multisig_pk = MultiSigPublicKey::new(
        vec![pk1.clone(), pk2.clone(), pk3.clone()],
        vec![1, 1, 1],
        2,
    )
    .unwrap();
    let addr = IotaAddress::from(&multisig_pk);
    let tx = make_transaction(addr, &keys[0]);

    let msg = IntentMessage::new(Intent::iota_transaction(), tx.transaction_data().clone());
    let sig1 = Signature::new_secure(&msg, &keys[0]).into();
    let sig2 = Signature::new_secure(&msg, &keys[1]).into();

    // Any 2 of 3 signatures verifies ok.
    let multi_sig1 = MultiSig::combine(vec![sig1, sig2], multisig_pk).unwrap();
    Transaction::new(SenderSignedData::new(
        tx.transaction_data().clone(),
        vec![GenericSignature::MultiSig(multi_sig1)],
    ))
}

/// Make a sponsored transaction where both sender and sponsor sign with regular
/// (Ed25519) signatures, for use in tests.
///
/// Returns the transaction together with the sender's and sponsor's addresses
/// so callers can locate each signature within the transaction.
pub fn make_sponsored_regular_sig_tx() -> (Transaction, IotaAddress, IotaAddress) {
    let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();
    let (sponsor, sponsor_kp): (_, AccountKeyPair) = get_key_pair();
    let tx_data = make_sponsored_transaction_data(sender, sponsor);
    let sender_sig: GenericSignature =
        Transaction::signature_from_signer(tx_data.clone(), Intent::iota_transaction(), &sender_kp)
            .into();
    let tx =
        to_sender_signed_transaction_with_optional_sponsor(tx_data, sender_sig, Some(&sponsor_kp));
    (tx, sender, sponsor)
}

mod move_authenticator {
    use fastcrypto::hash::HashFunction;
    use iota_sdk_types::Digest;

    pub use crate::move_authenticator::MoveAuthenticator;
    use crate::{
        base_types::IotaAddress,
        crypto::DefaultHash,
        object::OBJECT_START_VERSION,
        signature::GenericSignature,
        transaction::{CallArg, SenderSignedData, SharedObjectRef, Transaction},
        utils::{make_sponsored_transaction_data, make_transaction_data},
    };

    /// Make a transaction signed with `MoveAuthenticator` for testing.
    pub fn make_move_authenticator_tx(address: IotaAddress) -> Transaction {
        let data = make_transaction_data(address);
        let (authenticator, _) = make_move_authenticator_sig(address);
        Transaction::new(SenderSignedData::new(data, vec![authenticator]))
    }

    /// Build a [`GenericSignature::MoveAuthenticator`] and the underlying
    /// [`MoveAuthenticator`] for the given address, for use in tests.
    ///
    /// There is no real Move account behind this address.
    ///
    /// TODO: if it is necessary, AA accounts need to be supported properly in
    /// the `AuthorityState` used for testing.
    pub fn make_move_authenticator_sig(
        address: IotaAddress,
    ) -> (GenericSignature, MoveAuthenticator) {
        let authenticator = MoveAuthenticator::new_v1(
            vec![],
            vec![],
            CallArg::Shared(SharedObjectRef {
                object_id: address.into(),
                initial_shared_version: OBJECT_START_VERSION,
                mutable: false,
            }),
        );
        let sig = GenericSignature::MoveAuthenticator(authenticator.clone());
        (sig, authenticator)
    }

    /// Make a sponsored transaction where both sender and sponsor sign with
    /// [`MoveAuthenticator`], for use in tests.
    ///
    /// Returns the transaction together with the sender's and sponsor's
    /// [`MoveAuthenticator`] so callers can independently verify the expected
    /// auth digests.
    pub fn make_sponsored_move_authenticator_tx(
        sender_addr: IotaAddress,
        sponsor_addr: IotaAddress,
    ) -> (Transaction, MoveAuthenticator, MoveAuthenticator) {
        let (sender_sig, sender_auth) = make_move_authenticator_sig(sender_addr);
        let (sponsor_sig, sponsor_auth) = make_move_authenticator_sig(sponsor_addr);
        let tx_data = make_sponsored_transaction_data(sender_addr, sponsor_addr);
        let tx = Transaction::new(SenderSignedData::new(
            tx_data,
            vec![sender_sig, sponsor_sig],
        ));
        (tx, sender_auth, sponsor_auth)
    }

    /// Compute the Blake2b256 hash of the serialized (flag-prefixed) bytes of a
    /// [`GenericSignature`], matching the digest used for
    /// non-[`MoveAuthenticator`] signatures by
    /// [`crate::transaction::auth_digest_for_sig`].
    pub fn blake2b256_of_sig(sig: &GenericSignature) -> Digest {
        let mut hasher = DefaultHash::default();
        hasher.update(sig.as_ref());
        Digest::new(hasher.finalize().into())
    }
}

pub use move_authenticator::*;

mod passkey {
    use fastcrypto::secp256r1::Secp256r1KeyPair;

    use crate::{
        crypto::{Signature, Signer, get_key_pair},
        passkey_authenticator::PasskeyAuthenticator,
        signature::GenericSignature,
    };

    /// Build a [`GenericSignature::PasskeyAuthenticator`] backed by a
    /// freshly-generated Secp256r1 key pair, for use in tests.
    ///
    /// The challenge field is 32 zero-bytes encoded as base64url without
    /// padding, satisfying the length requirement without needing a real
    /// WebAuthn round-trip.
    pub fn make_passkey_authenticator_sig() -> GenericSignature {
        let (_, r1_kp): (_, Secp256r1KeyPair) = get_key_pair();
        let user_sig: Signature = r1_kp.sign(&[0u8; 32]);
        let client_data_json = r#"{"type":"webauthn.get","challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin":"https://test.iota.org"}"#;
        let passkey =
            PasskeyAuthenticator::new_for_testing(vec![], client_data_json.to_string(), user_sig)
                .unwrap();
        GenericSignature::PasskeyAuthenticator(passkey)
    }
}

pub use passkey::*;
