// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::traits::KeyPair;
use iota_sdk_types::{
    ObjectId, Transaction,
    crypto::{
        Intent, IntentAppId, IntentMessage, IntentScope, IntentVersion, PersonalMessage,
        SimpleSignature,
    },
};

use crate::{
    base_types::dbg_addr,
    committee::EpochId,
    crypto::{
        AccountKeyPair, AuthorityKeyPair, AuthoritySignature, IotaAuthoritySignature,
        IotaSignature, get_key_pair,
    },
    object::Object,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionDataAPI, TransactionEnvelope},
};

#[test]
fn test_personal_message_intent() {
    let (addr1, sec1): (_, AccountKeyPair) = get_key_pair();
    let message = "Hello".as_bytes().to_vec();
    let p_message = PersonalMessage(message.into());
    let p_message_2 = p_message.clone();
    let p_message_bcs = bcs::to_bytes(&p_message).unwrap();

    let intent = Intent::iota_app(IntentScope::PersonalMessage);
    let intent_bcs = bcs::to_bytes(&IntentMessage::new(intent, &p_message)).unwrap();
    assert_eq!(intent_bcs.len(), p_message_bcs.len() + 3);

    // Check that the first 3 bytes are the domain separation information.
    assert_eq!(
        &intent_bcs[..3],
        vec![
            IntentScope::PersonalMessage as u8,
            IntentVersion::V0 as u8,
            IntentAppId::Iota as u8,
        ]
    );

    // Check that intent's last bytes match the p_message's bsc bytes.
    assert_eq!(&intent_bcs[3..], &p_message_bcs);

    // Let's ensure we can sign and verify intents.
    let s = SimpleSignature::new_secure(&IntentMessage::new(intent, p_message), &sec1);
    let verification = s.verify_secure(&IntentMessage::new(intent, p_message_2), addr1);
    assert!(verification.is_ok())
}

#[test]
fn test_authority_signature_intent() {
    let epoch: EpochId = 0;
    let kp: AuthorityKeyPair = get_key_pair().1;

    // Create a signed user transaction.
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let recipient = dbg_addr(2);
    let object_id = ObjectId::random();
    let object = Object::immutable_with_id_for_testing(object_id);
    let gas_price = 1000;
    let data = Transaction::new_transfer_iota(
        recipient,
        sender,
        None,
        object.object_ref(),
        gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        gas_price,
    );
    let signature = SimpleSignature::new_secure(&data.intent_message(), &sender_key);
    let tx = TransactionEnvelope::from_data(data, vec![signature]);
    let tx1 = tx.clone();
    assert!(
        tx.try_into_verified_for_testing(&Default::default())
            .is_ok()
    );

    // Create an intent with signed data.
    let intent_message = tx1.intent_message();
    let intent_bcs = bcs::to_bytes(&intent_message).unwrap();

    // Check that the first 3 bytes are the domain separation information.
    assert_eq!(
        &intent_bcs[..3],
        vec![
            IntentScope::TransactionData as u8,
            IntentVersion::V0 as u8,
            IntentAppId::Iota as u8,
        ]
    );

    // Check that intent's last bytes match the signed_data's bsc bytes.
    let signed_data_bcs = bcs::to_bytes(&tx1.data().transaction()).unwrap();
    assert_eq!(&intent_bcs[3..], signed_data_bcs);

    // Let's ensure we can sign and verify intents.
    let s = AuthoritySignature::new_secure(&intent_message, &epoch, &kp);
    let verification = s.verify_secure(&intent_message, 0, kp.public().into());
    assert!(verification.is_ok())
}
