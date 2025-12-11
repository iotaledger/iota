// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::disallowed_names)]

use std::str::FromStr;

use base_types_tests::timelock::TimeLock;
use fastcrypto::{
    encoding::{Base58, Encoding},
    traits::EncodeDecodeBase64,
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_2::types::{
    crypto::{Intent, IntentMessage, IntentScope},
    object::VersionExt,
};
use move_binary_format::file_format;

use super::*;
use crate::{
    balance::Balance,
    crypto::{
        AccountKeyPair, AuthorityKeyPair, AuthoritySignature, IotaAuthoritySignature,
        IotaSignature, Signature,
        bcs_signable_test::{Bar, Foo},
        get_key_pair, get_key_pair_from_bytes,
    },
    gas_coin::GasCoin,
    id::{ID, UID},
    object::Object,
};

#[test]
fn test_bcs_enum() {
    let address = Owner::AddressOwner(Address::new(rand::random()));
    let shared = Owner::Shared {
        initial_shared_version: 1,
    };

    let address_ser = bcs::to_bytes(&address).unwrap();
    let shared_ser = bcs::to_bytes(&shared).unwrap();

    println!("{address_ser:?}");
    println!("{shared_ser:?}");
    assert!(shared_ser.len() < address_ser.len());
}

#[test]
fn test_signatures() {
    let (addr1, sec1): (_, AccountKeyPair) = get_key_pair();
    let (addr2, _sec2): (_, AccountKeyPair) = get_key_pair();

    let foo = IntentMessage::new(Intent::iota_transaction(), Foo("hello".into()));
    let foox = IntentMessage::new(Intent::iota_transaction(), Foo("hellox".into()));
    let bar = IntentMessage::new(Intent::iota_transaction(), Bar("hello".into()));

    let s = Signature::new_secure(&foo, &sec1);
    assert!(
        s.verify_secure(&foo, addr1, SignatureScheme::ED25519)
            .is_ok()
    );
    assert!(
        s.verify_secure(&foo, addr2, SignatureScheme::ED25519)
            .is_err()
    );
    assert!(
        s.verify_secure(&foox, addr1, SignatureScheme::ED25519)
            .is_err()
    );
    assert!(
        s.verify_secure(
            &IntentMessage::new(
                Intent::iota_app(IntentScope::SenderSignedTransaction),
                Foo("hello".into())
            ),
            addr1,
            SignatureScheme::ED25519
        )
        .is_err()
    );

    // The struct type is different, but the serialization is the same.
    assert!(
        s.verify_secure(&bar, addr1, SignatureScheme::ED25519)
            .is_ok()
    );
}

#[test]
fn test_signatures_serde() {
    let (_, sec1): (_, AccountKeyPair) = get_key_pair();
    let foo = Foo("hello".into());
    let s = Signature::new_secure(&IntentMessage::new(Intent::iota_transaction(), foo), &sec1);

    let serialized = bcs::to_bytes(&s).unwrap();
    println!("{serialized:?}");
    let deserialized: Signature = bcs::from_bytes(&serialized).unwrap();
    assert_eq!(deserialized.as_ref(), s.as_ref());
}

#[test]
fn test_max_sequence_number() {
    let max = Version::MAX_VALID_EXCL;
    assert_eq!(max * 2 + 1, u64::MAX);
}

#[test]
fn test_gas_coin_ser_deser_roundtrip() {
    let id = ObjectId::new(rand::random());
    let coin = GasCoin::new(id, 10);
    let coin_bytes = coin.to_bcs_bytes();

    let deserialized_coin: GasCoin = bcs::from_bytes(&coin_bytes).unwrap();
    assert_eq!(deserialized_coin.id(), coin.id());
    assert_eq!(deserialized_coin.value(), coin.value());
}

#[test]
fn test_lamport_increment_version() {
    let versions = [1, 3, 257, 42];

    let incremented = Version::lamport_increment(versions);

    for version in versions {
        assert!(version < incremented, "Expected: {version} < {incremented}");
    }
}

#[test]
fn test_object_id_conversions() {}

#[test]
fn test_object_id_display() {
    let id = ObjectId::from_str(SAMPLE_ADDRESS).unwrap();
    assert_eq!(format!("{id:?}"), SAMPLE_ADDRESS);
}

#[test]
fn test_object_id_str_lossless() {
    let id = ObjectId::from_str("0000000000000000000000000000000000c0f1f95c5b1c5f0eda533eff269000")
        .unwrap();
    let id_empty =
        ObjectId::from_str("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    let id_one =
        ObjectId::from_str("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap();

    assert_eq!(id.to_short_string(false), "c0f1f95c5b1c5f0eda533eff269000",);
    assert_eq!(id_empty.to_short_string(false), "0",);
    assert_eq!(id_one.to_short_string(false), "1",);
}

#[test]
fn test_object_id_from_hex_literal() {
    let hex_literal = "0x1";
    let hex = "0000000000000000000000000000000000000000000000000000000000000001";

    let obj_id_from_literal = ObjectId::from_hex(hex_literal).unwrap();
    let obj_id = ObjectId::from_str(hex).unwrap();

    assert_eq!(obj_id_from_literal, obj_id);
    assert_eq!(hex_literal, obj_id.to_hex());

    // Missing '0x'
    ObjectId::from_hex(hex).unwrap_err();
    // Too long
    ObjectId::from_hex("0x10000000000000000000000000000000000000000000000000000000000000001")
        .unwrap_err();
    assert_eq!(
        "0x0000000000000000000000000000000000000000000000000000000000000001",
        obj_id.to_hex()
    );
}

#[test]
fn test_object_id_ref() {
    let obj_id = ObjectId::new([1u8; ObjectId::LENGTH]);
    let _: &[u8] = obj_id.as_ref();
}

#[test]
fn test_object_id_deserialize_from_json_value() {
    let obj_id = ObjectId::new(rand::random());
    let json_value = serde_json::to_value(obj_id).expect("serde_json::to_value fail.");
    let obj_id2: ObjectId =
        serde_json::from_value(json_value).expect("serde_json::from_value fail.");
    assert_eq!(obj_id, obj_id2)
}

#[test]
fn test_object_id_serde_json() {
    let json_hex = format!("\"{SAMPLE_ADDRESS}\"");

    let obj_id = ObjectId::from_hex(SAMPLE_ADDRESS).unwrap();

    let json = serde_json::to_string(&obj_id).unwrap();
    let json_obj_id: ObjectId = serde_json::from_str(&json_hex).unwrap();

    assert_eq!(json, json_hex);
    assert_eq!(obj_id, json_obj_id);
}

#[test]
fn test_object_id_serde_not_human_readable() {
    let obj_id = ObjectId::new(rand::random());
    let serialized = bcs::to_bytes(&obj_id).unwrap();
    assert_eq!(obj_id.as_bytes().to_vec(), serialized);
    let deserialized: ObjectId = bcs::from_bytes(&serialized).unwrap();
    assert_eq!(deserialized, obj_id);
}

#[test]
fn test_object_id_serde_with_expected_value() {
    let object_id_vec = SAMPLE_ADDRESS_VEC;
    let object_id = ObjectId::new(object_id_vec);
    let json_serialized = serde_json::to_string(&object_id).unwrap();
    let bcs_serialized = bcs::to_bytes(&object_id).unwrap();

    let expected_json_address = format!("\"{SAMPLE_ADDRESS}\"");
    assert_eq!(expected_json_address, json_serialized);
    assert_eq!(object_id_vec, bcs_serialized.as_slice());
}

#[test]
fn test_object_id_zero_padding() {
    let hex = "0x2";
    let long_hex = "0x0000000000000000000000000000000000000000000000000000000000000002";
    let long_hex_alt = "0000000000000000000000000000000000000000000000000000000000000002";
    let obj_id_1 = ObjectId::from_str(hex).unwrap();
    let obj_id_2 = ObjectId::from_str(long_hex).unwrap();
    let obj_id_3 = ObjectId::from_str(long_hex_alt).unwrap();
    let obj_id_4: ObjectId = serde_json::from_str(&format!("\"{hex}\"")).unwrap();
    let obj_id_5: ObjectId = serde_json::from_str(&format!("\"{long_hex}\"")).unwrap();
    let obj_id_6: ObjectId = serde_json::from_str(&format!("\"{long_hex_alt}\"")).unwrap();
    assert_eq!(Address::FRAMEWORK.as_bytes(), obj_id_1.as_bytes());
    assert_eq!(Address::FRAMEWORK.as_bytes(), obj_id_2.as_bytes());
    assert_eq!(Address::FRAMEWORK.as_bytes(), obj_id_3.as_bytes());
    assert_eq!(Address::FRAMEWORK.as_bytes(), obj_id_4.as_bytes());
    assert_eq!(Address::FRAMEWORK.as_bytes(), obj_id_5.as_bytes());
    assert_eq!(Address::FRAMEWORK.as_bytes(), obj_id_6.as_bytes());
}

#[test]
fn test_address_display() {
    let id = Address::from_str(SAMPLE_ADDRESS).unwrap();
    assert_eq!(format!("{id:?}"), SAMPLE_ADDRESS);
}

#[test]
fn test_address_serde_not_human_readable() {
    let address = Address::new(rand::random());
    let serialized = bincode::serialize(&address).unwrap();
    let bcs_serialized = bcs::to_bytes(&address).unwrap();
    // bincode use 8 bytes for BYTES len and bcs use 1 byte
    assert_eq!(serialized, bcs_serialized);
    assert_eq!(address.as_bytes(), serialized.as_slice());
    let deserialized: Address = bincode::deserialize(&serialized).unwrap();
    assert_eq!(deserialized, address);
}

#[test]
fn test_address_serde_human_readable() {
    let address = Address::new(rand::random());
    let serialized = serde_json::to_string(&address).unwrap();
    assert_eq!(format!("\"{address}\""), serialized);
    let deserialized: Address = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, address);
}

#[test]
fn test_address_serde_with_expected_value() {
    let address = Address::new(SAMPLE_ADDRESS_VEC);
    let json_serialized = serde_json::to_string(&address).unwrap();
    let bcs_serialized = bcs::to_bytes(&address).unwrap();

    assert_eq!(format!("\"{SAMPLE_ADDRESS}\""), json_serialized);
    assert_eq!(SAMPLE_ADDRESS_VEC, bcs_serialized.as_slice());
}

#[test]
fn test_transaction_digest_serde_not_human_readable() {
    let digest = TransactionDigest::new(rand::random());
    let serialized = bincode::serialize(&digest).unwrap();
    let bcs_serialized = bcs::to_bytes(&digest).unwrap();
    // bincode use 8 bytes for BYTES len and bcs use 1 byte
    assert_eq!(serialized[8..], bcs_serialized[1..]);
    assert_eq!(digest.inner(), &serialized[8..]);
    let deserialized: TransactionDigest = bincode::deserialize(&serialized).unwrap();
    assert_eq!(deserialized, digest);
}

#[test]
fn test_transaction_digest_serde_human_readable() {
    let digest = TransactionDigest::new(rand::random());
    let serialized = serde_json::to_string(&digest).unwrap();
    assert_eq!(
        format!("\"{}\"", Base58::encode(digest.inner())),
        serialized
    );
    let deserialized: TransactionDigest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, digest);
}

#[test]
fn test_authority_signature_serde_not_human_readable() {
    let (_, key): (_, AuthorityKeyPair) = get_key_pair();
    let sig = AuthoritySignature::new_secure(
        &IntentMessage::new(Intent::iota_transaction(), Foo("some data".to_string())),
        &0,
        &key,
    );
    let serialized = bincode::serialize(&sig).unwrap();
    let bcs_serialized = bcs::to_bytes(&sig).unwrap();

    assert_eq!(serialized, bcs_serialized);
    let deserialized: AuthoritySignature = bincode::deserialize(&serialized).unwrap();
    assert_eq!(deserialized.as_ref(), sig.as_ref());
}

#[test]
fn test_authority_signature_serde_human_readable() {
    let (_, key): (_, AuthorityKeyPair) = get_key_pair();
    let sig = AuthoritySignature::new_secure(
        &IntentMessage::new(Intent::iota_transaction(), Foo("some data".to_string())),
        &0,
        &key,
    );
    let serialized = serde_json::to_string(&sig).unwrap();
    assert_eq!(format!("\"{}\"", sig.encode_base64()), serialized);
    let deserialized: AuthoritySignature = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.as_ref(), sig.as_ref());
}

#[test]
fn test_object_id_from_empty_string() {
    assert!(ObjectId::from_str("").is_err());
}

#[test]
fn test_move_object_size_for_gas_metering() {
    let object = Object::with_id_owner_for_testing(
        ObjectId::new(rand::random()),
        Address::new(rand::random()),
    );
    let size = object.object_size_for_gas_metering();
    let serialized = bcs::to_bytes(&object).unwrap();
    // If the following assertion breaks, it's likely you have changed MoveObject's
    // fields. Make sure to adjust `object_size_for_gas_metering()` to include
    // those changes.
    assert_eq!(size - 4, serialized.len());
}

#[test]
fn test_move_package_size_for_gas_metering() {
    let module = file_format::empty_module();
    let config = ProtocolConfig::get_for_max_version_UNSAFE();
    let package = Object::new_package(
        &[module],
        TransactionDigest::GENESIS_MARKER,
        &config,
        &[], // empty dependencies for empty package (no modules)
    )
    .unwrap();
    let size = package.object_size_for_gas_metering();
    let serialized = bcs::to_bytes(&package).unwrap();
    // If the following assertion breaks, it's likely you have changed MovePackage's
    // fields. Make sure to adjust `object_size_for_gas_metering()` to include
    // those changes.
    assert_eq!(size, serialized.len());
}

// A sample address in hex generated by the current address derivation
// algorithm.
const SAMPLE_ADDRESS: &str = "0xee6283a054cab26620a531d18c49ea3560239cfd8ba83a393bbaa7d7eed2082a";
const SAMPLE_ADDRESS_VEC: [u8; 32] = [
    238, 98, 131, 160, 84, 202, 178, 102, 32, 165, 49, 209, 140, 73, 234, 53, 96, 35, 156, 253,
    139, 168, 58, 57, 59, 186, 167, 215, 238, 210, 8, 42,
];

// Derive a sample address and public key tuple from KeyPair bytes.
fn derive_sample_address() -> (Address, AccountKeyPair) {
    let (address, pub_key) = get_key_pair_from_bytes(&[
        10, 112, 5, 142, 174, 127, 187, 146, 251, 68, 22, 191, 128, 68, 84, 13, 102, 71, 77, 57,
        92, 154, 128, 240, 158, 45, 13, 123, 57, 21, 194, 214, 189, 215, 127, 86, 129, 189, 1, 4,
        90, 106, 17, 10, 123, 200, 40, 18, 34, 173, 240, 91, 213, 72, 183, 249, 213, 210, 39, 181,
        105, 254, 59, 163,
    ])
    .unwrap();
    (address, pub_key)
}

// Required to capture address derivation algorithm updates that break some
// tests and deployments.
#[test]
fn test_address_backwards_compatibility() {
    let (address, _) = derive_sample_address();
    assert_eq!(
        address.to_string(),
        SAMPLE_ADDRESS,
        "If this test broke, then the algorithm for deriving addresses from public keys has \
               changed. If this was intentional, please compute a new sample address in hex format \
               from `derive_sample_address` and update the SAMPLE_ADDRESS const above with the new \
               derived address hex value. Note that existing deployments (i.e. devnet) might \
               also require updates if they use fixed values generated by the old algorithm."
    );
}

// tests translating into and out of a MoveObjectType from a StructTag
#[test]
fn move_object_type_consistency() {
    // Tests consistency properties for the relationship between a StructTag and a
    // MoveObjectType
    fn assert_consistent(tag: &StructTag) -> MoveObjectType {
        let ty: MoveObjectType = tag.clone().into();
        // check into/out of the tag works
        assert!(ty.is(tag));
        let ty_as_tag: StructTag = ty.clone().into();
        assert_eq!(&ty_as_tag, tag);
        // test same type information
        assert_eq!(ty.address(), tag.address);
        assert_eq!(ty.module(), tag.module.as_ident_str());
        assert_eq!(ty.name(), tag.name.as_ident_str());
        assert_eq!(&ty.type_params(), &tag.type_params);
        assert_eq!(ty.module_id(), tag.module_id());
        // sanity check special cases
        assert!(!ty.is_gas_coin() || ty.is_coin());
        assert!(!ty.is_timelocked_balance() || ty.is_timelock());
        let cases = [
            ty.is_coin(),
            ty.is_staked_iota(),
            ty.is_coin_metadata(),
            ty.is_dynamic_field(),
            ty.is_timelock(),
            ty.is_timelocked_staked_iota(),
        ];
        assert!(cases.into_iter().map(|is_ty| is_ty as u8).sum::<u8>() <= 1);
        ty
    }

    let ty = assert_consistent(&GasCoin::type_());
    assert!(ty.is_coin());
    assert!(ty.is_gas_coin());
    let ty = assert_consistent(&StakedIota::type_());
    assert!(ty.is_staked_iota());
    let ty = assert_consistent(&Coin::type_(TypeTag::U64));
    assert!(ty.is_coin());
    let ty = assert_consistent(&CoinMetadata::type_(GasCoin::type_()));
    assert!(ty.is_coin_metadata());
    let ty = assert_consistent(&DynamicFieldInfo::dynamic_field_type(
        TypeTag::Struct(Box::new(ID::type_())),
        TypeTag::U64,
    ));
    assert!(ty.is_dynamic_field());
    let ty = assert_consistent(&TimeLock::<Balance>::type_(
        Balance::type_(GAS::type_().into()).into(),
    ));
    assert_eq!(ty, MoveObjectType::timelocked_iota_balance());
    assert!(ty.is_timelock());
    assert!(ty.is_timelocked_balance());
    let ty = assert_consistent(&TimeLock::<Coin>::type_(GasCoin::type_().into()));
    assert!(ty.is_timelock());
    assert!(!ty.is_timelocked_balance());
    let ty = assert_consistent(&TimelockedStakedIota::type_());
    assert_eq!(ty, MoveObjectType::timelocked_staked_iota());
    assert!(ty.is_timelocked_staked_iota());
    assert_consistent(&UID::type_());
    assert_consistent(&ID::type_());
}
