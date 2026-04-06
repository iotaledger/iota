// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use fastcrypto::traits::KeyPair as KeypairTraits;
use iota_sdk_types::crypto::{Intent, IntentMessage};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    IotaAddress,
    base_types::{ObjectID, dbg_addr},
    committee::Committee,
    crypto::{
        AccountKeyPair, AuthorityKeyPair, AuthorityPublicKeyBytes, IotaKeyPair, Signature, Signer,
        get_key_pair, get_key_pair_from_rng,
    },
    multisig::{MultiSig, MultiSigPublicKey, MultisigMember},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::GenericSignature,
    transaction::{
        SenderSignedData, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, Transaction, TransactionData,
        TransactionDataAPI,
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
        let test_data: Vec<TestData> = serde_json::from_reader(file).unwrap();
        test_data[1].zklogin_inputs.clone()
    }

    fn get_inputs_with_bad_address_seed() -> ZkLoginInputs {
        thread_local! {
        static ZKLOGIN_INPUTS: ZkLoginInputs = ZkLoginInputs::from_json("{\"proofPoints\":{\"a\":[\"17276311605393076686048412951904952585208929623427027497902331765285829154985\",\"2195957390349729412627479867125563520760023859523358729791332629632025124364\",\"1\"],\"b\":[[\"10285059021604767951039627893758482248204478992077021270802057708215366770814\",\"20086937595807139308592304218494658586282197458549968652049579308384943311509\"],[\"7481123765095657256931104563876569626157448050870256177668773471703520958615\",\"11912752790863530118410797223176516777328266521602785233083571774104055633375\"],[\"1\",\"0\"]],\"c\":[\"15742763887654796666500488588763616323599882100448686869458326409877111249163\",\"6112916537574993759490787691149241262893771114597679488354854987586060572876\",\"1\"]},\"issBase64Details\":{\"value\":\"wiaXNzIjoiaHR0cHM6Ly9pZC50d2l0Y2gudHYvb2F1dGgyIiw\",\"indexMod4\":2},\"headerBase64\":\"eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6IjEifQ\"}", SHORT_ADDRESS_SEED).unwrap(); }
        ZKLOGIN_INPUTS.with(|a| a.clone())
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
            vec![
                MultisigMember::new(pk1.clone(), 1),
                MultisigMember::new(pk2.clone(), 1),
                MultisigMember::new(pk3.clone(), 1),
            ],
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
}

mod move_authenticator {
    pub use crate::move_authenticator::MoveAuthenticator;
    use crate::{
        base_types::IotaAddress,
        object::OBJECT_START_VERSION,
        signature::GenericSignature,
        transaction::{CallArg, SenderSignedData, SharedObjectRef, Transaction},
        utils::make_transaction_data,
    };

    /// Make a transaction signed with `MoveAuthenticator` for testing.
    pub fn make_move_authenticator_tx(address: IotaAddress) -> Transaction {
        let data = make_transaction_data(address);

        // There is no a real Move account behind this address.
        //
        // TODO: if it is necessary, AA accounts need to be supported properly in the
        // `AuthorityState` used for testing.
        let self_call_arg = CallArg::Shared(SharedObjectRef {
            object_id: address.into(),
            initial_shared_version: OBJECT_START_VERSION,
            mutable: false,
        });
        let authenticator = GenericSignature::MoveAuthenticator(MoveAuthenticator::new_v1(
            vec![],
            vec![],
            self_call_arg,
        ));

        Transaction::new(SenderSignedData::new(data, vec![authenticator]))
    }
}

pub use move_authenticator::*;
