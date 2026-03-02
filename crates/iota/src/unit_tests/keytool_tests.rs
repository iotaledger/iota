// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::Ok;
use fastcrypto::{
    ed25519::{Ed25519KeyPair, Ed25519PublicKey, Ed25519Signature},
    encoding::{Base64, Encoding, Hex},
    traits::{ToFromBytes, VerifyingKey},
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, InMemKeystore, Keystore, StoredKey};
use iota_sdk_types::crypto::{Intent, IntentScope};
use iota_types::{
    base_types::{IotaAddress, ObjectDigest, ObjectID, SequenceNumber},
    crypto::{
        AuthorityKeyPair, Ed25519IotaSignature, EncodeDecodeBase64, IotaKeyPair,
        IotaSignatureInner, PublicKey, Secp256k1IotaSignature, Secp256r1IotaSignature, Signature,
        SignatureScheme, get_key_pair, get_key_pair_from_rng,
    },
    signature::GenericSignature,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionData},
};
use rand::{SeedableRng, rngs::StdRng};
use tempfile::TempDir;
use tokio::test;

use super::{KeyToolCommand, write_keypair_to_file};
use crate::{
    key_identity::KeyIdentity,
    keytool::{CommandOutput, read_authority_keypair_from_file, read_keypair_from_file},
    signing::sign_secure,
};

const TEST_MNEMONIC: &str = "result crisp session latin must fruit genuine question prevent start coconut brave speak student dismiss";

#[test]
async fn test_addresses_command() -> Result<(), anyhow::Error> {
    // Add 3 Ed25519 KeyPairs as default
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(3));

    // Add another 3 Secp256k1 KeyPairs
    for _ in 0..3 {
        keystore.add_key(None, IotaKeyPair::Secp256k1(get_key_pair().1))?;
    }

    // List all addresses with flag
    KeyToolCommand::List {
        sort_by_alias: true,
    }
    .execute(&mut keystore)
    .await
    .unwrap();
    Ok(())
}

#[test]
async fn test_flag_in_signature_and_keypair() -> Result<(), anyhow::Error> {
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));

    keystore.add_key(None, IotaKeyPair::Secp256k1(get_key_pair().1))?;
    keystore.add_key(None, IotaKeyPair::Ed25519(get_key_pair().1))?;

    for key in keystore
        .keys()
        .into_iter()
        .filter(|k| matches!(k, StoredKey::KeyPair(_)))
    {
        let pk = key.public();
        let sig = sign_secure(
            &keystore,
            &key.address(),
            b"hello",
            Intent::iota_transaction(),
        )?;
        match sig {
            Signature::Ed25519IotaSignature(_) => {
                // signature contains corresponding flag
                assert_eq!(
                    *sig.as_ref().first().unwrap(),
                    Ed25519IotaSignature::SCHEME.flag()
                );
                // keystore stores pubkey with corresponding flag
                assert!(pk.flag() == Ed25519IotaSignature::SCHEME.flag())
            }
            Signature::Secp256k1IotaSignature(_) => {
                assert_eq!(
                    *sig.as_ref().first().unwrap(),
                    Secp256k1IotaSignature::SCHEME.flag()
                );
                assert!(pk.flag() == Secp256k1IotaSignature::SCHEME.flag())
            }
            Signature::Secp256r1IotaSignature(_) => {
                assert_eq!(
                    *sig.as_ref().first().unwrap(),
                    Secp256r1IotaSignature::SCHEME.flag()
                );
                assert!(pk.flag() == Secp256r1IotaSignature::SCHEME.flag())
            }
        }
    }
    Ok(())
}

#[test]
async fn test_read_write_keystore_with_flag() {
    let dir = tempfile::TempDir::new().unwrap();

    // create Secp256k1 keypair
    let kp_secp = IotaKeyPair::Secp256k1(get_key_pair().1);
    let addr_secp: IotaAddress = (&kp_secp.public()).into();
    let fp_secp = dir.path().join(format!("{addr_secp}.key"));
    let fp_secp_2 = fp_secp.clone();

    // write Secp256k1 keypair to file
    let res = write_keypair_to_file(&kp_secp, &fp_secp);
    assert!(res.is_ok());

    // read from file as enum KeyPair success
    let kp_secp_read = read_keypair_from_file(fp_secp);
    assert!(kp_secp_read.is_ok());

    // KeyPair wrote into file is the same as read
    assert_eq!(
        kp_secp_read.unwrap().public().as_ref(),
        kp_secp.public().as_ref()
    );

    // read as AuthorityKeyPair fails
    let kp_secp_read = read_authority_keypair_from_file(fp_secp_2);
    assert!(kp_secp_read.is_err());

    // create Ed25519 keypair
    let kp_ed = IotaKeyPair::Ed25519(get_key_pair().1);
    let addr_ed: IotaAddress = (&kp_ed.public()).into();
    let fp_ed = dir.path().join(format!("{addr_ed}.key"));
    let fp_ed_2 = fp_ed.clone();

    // write Ed25519 keypair to file
    let res = write_keypair_to_file(&kp_ed, &fp_ed);
    assert!(res.is_ok());

    // read from file as enum KeyPair success
    let kp_ed_read = read_keypair_from_file(fp_ed);
    assert!(kp_ed_read.is_ok());

    // KeyPair wrote into file is the same as read
    assert_eq!(
        kp_ed_read.unwrap().public().as_ref(),
        kp_ed.public().as_ref()
    );

    // read from file as AuthorityKeyPair success
    let kp_ed_read = read_authority_keypair_from_file(fp_ed_2);
    assert!(kp_ed_read.is_err());
}

#[test]
async fn test_iota_operations_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("iota.keystore");
    let path1 = path.clone();
    // This is the hardcoded keystore in iota-operation: https://github.com/iotaledger/iota-operations/blob/af04c9d3b61610dbb36401aff6bef29d06ef89f8/docker/config/generate/static/iota.keystore
    // If this test fails, address hardcoded in iota-operations is likely needed be
    // updated.
    let kp = IotaKeyPair::decode(
        "iotaprivkey1qr2x8cgu0y2egh5x4s4h9kytsxgvltv0776gul4cjtp8tfw0pglgyye70lu",
    )
    .unwrap();
    let contents = vec![kp.encode().unwrap()];
    let res = std::fs::write(path, serde_json::to_string_pretty(&contents).unwrap());
    assert!(res.is_ok());
    let read = FileBasedKeystore::new(&path1);
    assert!(read.is_ok());
    assert_eq!(
        IotaAddress::from_str("bc14937ffd5874a57afa10edf2d267d8eaaaf61081d718d9ba19cae85c00c6e8")
            .unwrap(),
        read.unwrap().addresses()[0]
    );

    // This is the hardcoded keystore in iota-operation: https://github.com/iotaledger/iota-operations/blob/af04c9d3b61610dbb36401aff6bef29d06ef89f8/docker/config/generate/static/iota-benchmark.keystore
    // If this test fails, address hardcoded in iota-operations is likely needed be
    // updated.
    let path2 = temp_dir.path().join("iota-benchmark.keystore");
    let path3 = path2.clone();
    let kp = IotaKeyPair::decode(
        "iotaprivkey1qrcfd38ngfhqrvfes20rrul28ej7dswn2hy6h0wtsgkvs9expd0qqy38y3q",
    )
    .unwrap();
    let contents = vec![kp.encode().unwrap()];
    let res = std::fs::write(path2, serde_json::to_string_pretty(&contents).unwrap());
    assert!(res.is_ok());
    let read = FileBasedKeystore::new(&path3);
    assert_eq!(
        IotaAddress::from_str("e988a8fb85944173237d287e98e542ae50c119c02644856ed8db17fe9f528b13")
            .unwrap(),
        read.unwrap().addresses()[0]
    );
}

#[test]
async fn test_load_keystore_err() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("iota.keystore");
    let path2 = path.clone();

    // write encoded AuthorityKeyPair without flag byte to file
    let kp: AuthorityKeyPair = get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1;
    let contents = kp.encode_base64();
    let res = std::fs::write(path, serde_json::to_string(&[contents]).unwrap());
    assert!(res.is_ok());

    // cannot load keypair due to wrong format
    assert!(FileBasedKeystore::new(&path2).is_err());
}

#[test]
async fn test_private_keys_import_export() -> Result<(), anyhow::Error> {
    // private key in Bech32, private key in Hex, private key in Base64, derived
    // IOTA address in Hex
    const TEST_CASES: &[(&str, &str, &str, &str)] = &[
        (
            "iotaprivkey1qzwant3kaegmjy4qxex93s0jzvemekkjmyv3r2sjwgnv2y479pgsyj3mjxj",
            "0x9dd9ae36ee51b912a0364c58c1f21333bcdad2d91911aa127226c512be285102",
            "AJ3ZrjbuUbkSoDZMWMHyEzO82tLZGRGqEnImxRK+KFEC",
            "0x80ebb793af9b40569ed2c3be16e5bd76358997e28cc35ff48681157224c8b038",
        ),
        (
            "iotaprivkey1qrh2sjl88rze74hwjndw3l26dqyz63tea5u9frtwcsqhmfk9vxdlxm7k85r",
            "0xeea84be738c59f56ee94dae8fd5a68082d4579ed38548d6ec4017da6c5619bf3",
            "AO6oS+c4xZ9W7pTa6P1aaAgtRXntOFSNbsQBfabFYZvz",
            "0xee64bf4dedc0d6d853e156b04b4adae937da0b549d740b9de26d0360813f8f1c",
        ),
        (
            "iotaprivkey1qzg73qyvfz0wpnyectkl08nrhe4pgnu0vqx8gydu96qx7uj4wyr8gy995v6",
            "0x91e8808c489ee0cc99c2edf79e63be6a144f8f600c7411bc2e806f7255710674",
            "AJHogIxInuDMmcLt955jvmoUT49gDHQRvC6Ab3JVcQZ0",
            "0x3a3bf7803ccb4903e2d9c4c11f80a24fe0c57a3a298f4ebd52667849554021b5",
        ),
    ];
    // assert correctness
    for (private_key, private_key_hex, private_key_base64, address) in TEST_CASES {
        let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
        KeyToolCommand::Import {
            alias: None,
            input_string: private_key.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: None,
        }
        .execute(&mut keystore)
        .await?;
        let kp = IotaKeyPair::decode(private_key).unwrap();
        let kp_from_hex = IotaKeyPair::Ed25519(
            Ed25519KeyPair::from_bytes(&Hex::decode(private_key_hex).unwrap()).unwrap(),
        );
        assert_eq!(kp, kp_from_hex);

        let kp_from_base64 = IotaKeyPair::decode_base64(private_key_base64).unwrap();
        assert_eq!(kp, kp_from_base64);

        let addr = IotaAddress::from_str(address).unwrap();
        assert_eq!(IotaAddress::from(&kp.public()), addr);
        assert!(keystore.addresses().contains(&addr));

        // Export output shows the private key in Bech32
        let output = KeyToolCommand::Export {
            key_identity: KeyIdentity::Address(addr),
        }
        .execute(&mut keystore)
        .await?;
        match output {
            CommandOutput::Export(exported) => {
                assert_eq!(exported.exported_private_key, private_key.to_string());
            }
            _ => panic!("unexpected output"),
        }
    }

    for (private_key, _, _, addr) in TEST_CASES {
        let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
        // assert failure when private key is malformed
        let output = KeyToolCommand::Import {
            alias: None,
            input_string: private_key[1..].to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: None,
        }
        .execute(&mut keystore)
        .await;
        assert!(output.is_err());

        // importing an hex encoded string should fail
        let output = KeyToolCommand::Import {
            alias: None,
            input_string: addr.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: None,
        }
        .execute(&mut keystore)
        .await;
        assert!(output.is_err());
    }

    Ok(())
}

#[test]
async fn test_mnemonics_ed25519() -> Result<(), anyhow::Error> {
    #[rustfmt::skip]
    // Test case matches with
    // /iotaledger/iota/sdk/typescript/test/unit/cryptography/ed25519-keypair.test.ts
    const TEST_CASES: [[&str; 3]; 3] = [
        [
            "film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm",
            "iotaprivkey1qrqqxhsu3ndp96644fjk4z5ams5ulgmvprklngt2jhvg2ujn5w4q2d2vplv",
            "0x9f8e5379678525edf768d7b507dc1ba9016fc4f0eac976ab7f74077d95fba312",
        ],
        [
            "require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level",
            "iotaprivkey1qqcxaf57fnenvflpacacaumf6vl0rt0edddhytanvzhkqhwnjk0zspg902d",
            "0x862738192e40540e0a5c9a5aca636f53b0cd76b0a9bef3386e05647feb4914ac",
        ],
        [
            "organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake",
            "iotaprivkey1qzq39vxzm0gq7l8dc5dj5allpuww4mavhwhg8mua4cl3lj2c3fvhcv5l2vn",
            "0x2391788ca49c7f0f00699bc2bad45f80c343b4d1df024285c132259433d7ff31",
        ],
    ];

    for t in TEST_CASES {
        let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
        KeyToolCommand::Import {
            alias: None,
            input_string: t[0].to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: None,
        }
        .execute(&mut keystore)
        .await?;
        let kp = IotaKeyPair::decode(t[1]).unwrap();
        let addr = IotaAddress::from_str(t[2]).unwrap();
        assert_eq!(IotaAddress::from(&kp.public()), addr);
        assert!(keystore.addresses().contains(&addr));
    }
    Ok(())
}

#[test]
async fn test_mnemonics_secp256k1() -> Result<(), anyhow::Error> {
    #[rustfmt::skip]
    // Test case matches with
    // /iotaledger/iota/sdk/typescript/test/unit/cryptography/secp256k1-keypair.test.ts
    const TEST_CASES: [[&str; 3]; 3] = [
        [
            "film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm",
            "iotaprivkey1q8cy2ll8a0dmzzzwn9zavrug0qf47cyuj6k2r4r6rnjtpjhrdh52vpegd4f",
            "0x8520d58dde1ab268349b9a46e5124ae6fe7e4c61df4ca2bc9c97d3c4d07b0b55",
        ],
        [
            "require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level",
            "iotaprivkey1q9hm330d05jcxfvmztv046p8kclyaj39hk6elqghgpq4sz4x23hk2wd6cfz",
            "0x3740d570eefba29dfc0fdd5829848902064e31ecd059ca05c401907fa8646f61",
        ],
        [
            "organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake",
            "iotaprivkey1qx2dnch6363h7gdqqfkzmmlequzj4ul3x4fq6dzyajk7wc2c0jgcx32axh5",
            "0x943b852c37fef403047e06ff5a2fa216557a4386212fb29554babdd3e1899da5",
        ],
    ];

    for t in TEST_CASES {
        let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
        KeyToolCommand::Import {
            alias: None,
            input_string: t[0].to_string(),
            key_scheme: SignatureScheme::Secp256k1,
            derivation_path: None,
        }
        .execute(&mut keystore)
        .await?;
        let kp = IotaKeyPair::decode(t[1]).unwrap();
        let addr = IotaAddress::from_str(t[2]).unwrap();
        assert_eq!(IotaAddress::from(&kp.public()), addr);
        assert!(keystore.addresses().contains(&addr));
    }
    Ok(())
}

#[test]
async fn test_mnemonics_secp256r1() -> Result<(), anyhow::Error> {
    #[rustfmt::skip]
    // Test case matches with
    // /iotaledger/iota/sdk/typescript/test/unit/cryptography/secp256r1-keypair.test.ts
    const TEST_CASES: [[&str; 3]; 3] = [
        [
            "act wing dilemma glory episode region allow mad tourist humble muffin oblige",
            "iotaprivkey1qtt65ua2lhal76zg4cxd6umdqynv2rj2gzrntp5rwlnyj370jg3pwtqlwdn",
            "0x779a63b28528210a5ec6c4af5a70382fa3f0c2d3f98dcbe4e3a4ae2f8c39cc9c",
        ],
        [
            "flag rebel cabbage captain minimum purpose long already valley horn enrich salt",
            "iotaprivkey1qtcjgmue7q8u4gtutfvfpx3zj3aa2r9pqssuusrltxfv68eqhzsgjc3p4z7",
            "0x8b45523042933aa55f57e2ccc661304baed292529b6e67a0c9857c1f3f871806",
        ],
        [
            "area renew bar language pudding trial small host remind supreme cabbage era",
            "iotaprivkey1qtxafg26qxeqy7f56gd2rvsup0a5kl4cre7nt2rtcrf0p3v5pwd4cgrrff2",
            "0x8528ef86150ec331928a8b3edb8adbe2fb523db8c84679aa57a931da6a4cdb25",
        ],
    ];

    for [mnemonics, sk, address] in TEST_CASES {
        let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
        KeyToolCommand::Import {
            alias: None,
            input_string: mnemonics.to_string(),
            key_scheme: SignatureScheme::Secp256r1,
            derivation_path: None,
        }
        .execute(&mut keystore)
        .await?;

        let kp = IotaKeyPair::decode(sk).unwrap();
        let addr = IotaAddress::from_str(address).unwrap();
        assert_eq!(IotaAddress::from(&kp.public()), addr);
        assert!(keystore.addresses().contains(&addr));
    }

    Ok(())
}

#[test]
async fn test_invalid_derivation_path() -> Result<(), anyhow::Error> {
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: Some("m/44'/1'/0'/0/0".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_err()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: Some("m/0'/4218'/0'/0/0".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_err()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: Some("m/54'/4218'/0'/0/0".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_err()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::Secp256k1,
            derivation_path: Some("m/54'/4218'/0'/0'/0'".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_err()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::Secp256k1,
            derivation_path: Some("m/44'/4218'/0'/0/0".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_err()
    );

    Ok(())
}

#[test]
async fn test_valid_derivation_path() -> Result<(), anyhow::Error> {
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: Some("m/44'/4218'/0'/0'/0'".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_ok()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: Some("m/44'/4218'/0'/0'/1'".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_ok()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::ED25519,
            derivation_path: Some("m/44'/4218'/1'/0'/1'".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_ok()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::Secp256k1,
            derivation_path: Some("m/54'/4218'/0'/0/1".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_ok()
    );

    assert!(
        KeyToolCommand::Import {
            alias: None,
            input_string: TEST_MNEMONIC.to_string(),
            key_scheme: SignatureScheme::Secp256k1,
            derivation_path: Some("m/54'/4218'/1'/0/1".parse().unwrap()),
        }
        .execute(&mut keystore)
        .await
        .is_ok()
    );
    Ok(())
}

#[test]
async fn test_keytool_bls12381() -> Result<(), anyhow::Error> {
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
    KeyToolCommand::Generate {
        key_scheme: SignatureScheme::BLS12381,
        derivation_path: None,
        word_length: None,
    }
    .execute(&mut keystore)
    .await?;
    Ok(())
}

#[test]
async fn test_sign_command() -> Result<(), anyhow::Error> {
    // Add a keypair
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(1));
    let addresses = keystore.addresses();
    let sender = addresses.first().unwrap();
    let alias = keystore.get_alias_by_address(sender).unwrap();

    // Create a dummy TransactionData
    let gas = (
        ObjectID::random(),
        SequenceNumber::new(),
        ObjectDigest::random(),
    );
    let gas_price = 1;
    let tx_data = TransactionData::new_pay_iota(
        *sender,
        vec![gas],
        vec![IotaAddress::random_for_testing_only()],
        vec![10000],
        gas,
        gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        gas_price,
    )
    .unwrap();

    // Sign an intent message for the transaction data and a passed-in intent with
    // scope as PersonalMessage.
    KeyToolCommand::Sign {
        address: KeyIdentity::Address(*sender),
        data: Base64::encode(bcs::to_bytes(&tx_data)?),
        intent: Some(Intent::iota_app(IntentScope::PersonalMessage)),
    }
    .execute(&mut keystore)
    .await?;

    // Sign an intent message for the transaction data without intent passed in, so
    // default is used.
    KeyToolCommand::Sign {
        address: KeyIdentity::Address(*sender),
        data: Base64::encode(bcs::to_bytes(&tx_data)?),
        intent: None,
    }
    .execute(&mut keystore)
    .await?;

    // Sign an intent message for the transaction data without intent passed in, so
    // default is used. Use alias for signing instead of the address
    KeyToolCommand::Sign {
        address: KeyIdentity::Alias(alias),
        data: Base64::encode(bcs::to_bytes(&tx_data)?),
        intent: None,
    }
    .execute(&mut keystore)
    .await?;
    Ok(())
}

#[test]
async fn test_sign_raw_command() -> Result<(), anyhow::Error> {
    // Add a keypair
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(1));
    let addresses = keystore.addresses();
    let sender = addresses.first().unwrap();
    let alias = keystore.get_alias_by_address(sender).unwrap();

    let raw_data = Hex::encode_with_format("IOTA");

    let verify_sign_raw_output =
        |output: CommandOutput, expected_address: &IotaAddress, expected_data: &str| {
            let CommandOutput::SignRaw(sign_raw_data) = output else {
                panic!("Expected SignRaw output");
            };
            assert_eq!(sign_raw_data.iota_address, *expected_address);
            assert_eq!(sign_raw_data.raw_data, expected_data);
            // Verify the signature with actual Ed25519 verification
            let ed_sig =
                Ed25519Signature::from_bytes(&Hex::decode(&sign_raw_data.signature_hex).unwrap())
                    .expect("Invalid Ed25519 signature bytes");
            let ed_pk =
                Ed25519PublicKey::from_bytes(&Hex::decode(&sign_raw_data.public_key_hex).unwrap())
                    .expect("Invalid Ed25519 public key bytes");
            let data_bytes = Hex::decode(&sign_raw_data.raw_data).unwrap();
            ed_pk
                .verify(&data_bytes, &ed_sig)
                .expect("Ed25519 signature verification failed");
        };

    // Test with address
    let output = KeyToolCommand::SignRaw {
        address: KeyIdentity::Address(*sender),
        data: raw_data.to_string(),
    }
    .execute(&mut keystore)
    .await?;
    verify_sign_raw_output(output, sender, &raw_data);

    // Test with alias
    let output_alias = KeyToolCommand::SignRaw {
        address: KeyIdentity::Alias(alias),
        data: raw_data.to_string(),
    }
    .execute(&mut keystore)
    .await?;
    verify_sign_raw_output(output_alias, sender, &raw_data);

    Ok(())
}

#[test]
async fn test_show() -> Result<(), anyhow::Error> {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("iota.key");

    // First create a .key file with a private key
    std::fs::write(
        &path,
        "iotaprivkey1qp3asak8fsdwcrxc8fys02mhsg3fs35d7fe45s5zcyg6x3sp9zsw5wqnj5v",
    )
    .unwrap();

    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));
    let output = KeyToolCommand::Show { file: path }
        .execute(&mut keystore)
        .await?;
    match output {
        CommandOutput::Show(key) => {
            assert_eq!(
                &key.iota_address.to_string(),
                "0x5f60f23c01486c6af8540144cf9fa74c167257b93c08fc33b74b8f173a885038"
            );
            assert_eq!(
                key.public_base64_key.unwrap(),
                "svUb1I94/15y2k6LKaEWqNLFf1rNMHq0hcWFAJynu0g="
            );
            assert_eq!(
                key.public_base64_key_with_flag.unwrap(),
                "ALL1G9SPeP9ectpOiymhFqjSxX9azTB6tIXFhQCcp7tI"
            );
            assert_eq!(key.key_scheme.unwrap(), "ed25519");
            assert_eq!(key.flag.unwrap(), 0);
            assert_eq!(
                &key.peer_id.unwrap(),
                "b2f51bd48f78ff5e72da4e8b29a116a8d2c57f5acd307ab485c585009ca7bb48"
            );
        }
        _ => panic!("unexpected output: {output:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn test_multi_sig_combine_partial_sig() -> Result<(), anyhow::Error> {
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));

    // Public keys (Base64)
    let pk1 = PublicKey::decode_base64("AIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87l").unwrap();
    let pk2 = PublicKey::decode_base64("AIA4z3cY/7bzUz/Kj1mPe5I9k82gpL3J/WppWjnB53SI").unwrap();
    let pk3 = PublicKey::decode_base64("APBL9QuKI1MjSNn5Jt0w0zOUWdCQxbn84UlKmJtGbuU4").unwrap();
    let pks = vec![pk1, pk2, pk3];
    let weights = vec![1, 1, 1];
    let threshold = 2;

    // Signatures (Base64)
    let sig1 = GenericSignature::decode_base64("AP58oYBpNZRsR8ReDL05R/37o8l5t89e+RdBDId7yA0+Oxt/F/jlfCw8bnFR596zhVi9CN19bb0aWpn8U0cENQqCjNPlu8Lz+qYrU4CUFQe2H59qDDt2mXd76LZG+sfO5Q==").unwrap();
    let sig2 = GenericSignature::decode_base64("AIG+CPPEfpfJC/1AMSXrfPGmJ4hK7n2nGRp7ZTrYW3mPgM6zGJ+vepGk+CL0F9ihnzdA++CM2DUUCYOv4rHrQAqAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iA==").unwrap();
    let sigs = vec![sig1, sig2];

    let output = KeyToolCommand::MultiSigCombinePartialSig {
        sigs,
        pks,
        weights,
        threshold,
    }
    .execute(&mut keystore)
    .await?;

    let CommandOutput::MultiSigCombinePartialSig(data) = output else {
        panic!("unexpected output: {output:?}");
    };

    assert_eq!(
        data.multisig_address.to_string(),
        "0x9c3d1202a483f33cc340183df29ae9ffa55697947be431c963be78917e7fc538"
    );
    // Check parsed structure
    let parsed_json = serde_json::to_value(&data.multisig_parsed).unwrap();
    let expected_json = serde_json::json!({
        "sigs": [
            {"Ed25519": "/nyhgGk1lGxHxF4MvTlH/fujyXm3z175F0EMh3vIDT47G38X+OV8LDxucVHn3rOFWL0I3X1tvRpamfxTRwQ1Cg=="},
            {"Ed25519": "gb4I88R+l8kL/UAxJet88aYniErufacZGntlOthbeY+AzrMYn696kaT4IvQX2KGfN0D74IzYNRQJg6/isetACg=="}
        ],
        "bitmap": 3,
        "multisig_pk": {
            "pk_map": [
                [{"Ed25519": "gozT5bvC8/qmK1OAlBUHth+fagw7dpl3e+i2RvrHzuU="}, 1],
                [{"Ed25519": "gDjPdxj/tvNTP8qPWY97kj2TzaCkvcn9amlaOcHndIg="}, 1],
                [{"Ed25519": "8Ev1C4ojUyNI2fkm3TDTM5RZ0JDFufzhSUqYm0Zu5Tg="}, 1]
            ],
            "threshold": 2
        }
    });
    assert_eq!(parsed_json, expected_json);
    assert_eq!(
        data.multisig_serialized,
        "AwIA/nyhgGk1lGxHxF4MvTlH/fujyXm3z175F0EMh3vIDT47G38X+OV8LDxucVHn3rOFWL0I3X1tvRpamfxTRwQ1CgCBvgjzxH6XyQv9QDEl63zxpieISu59pxkae2U62Ft5j4DOsxifr3qRpPgi9BfYoZ83QPvgjNg1FAmDr+Kx60AKAwADAIKM0+W7wvP6pitTgJQVB7Yfn2oMO3aZd3votkb6x87lAQCAOM93GP+281M/yo9Zj3uSPZPNoKS9yf1qaVo5wed0iAEA8Ev1C4ojUyNI2fkm3TDTM5RZ0JDFufzhSUqYm0Zu5TgBAgA="
    );

    Ok(())
}

#[test]
async fn test_tx_digest() -> Result<(), anyhow::Error> {
    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));

    // Test unsigned transaction data
    let result = KeyToolCommand::TxDigest {
        tx_bytes: "AAACAAgAypo7AAAAAAAgERERERUE6TUOY11lzTjM0sApQ0xqOkgNiUepumoVshUCAgABAQAAAQEDAAAAAAEBABEREREVBOk1DmNdZc04zNLAKUNMajpIDYlHqbpqFbIVAU4PMbTXGjSXQkjCr1LNtHK9EpH/1O8JRuHyWt6uUtBZP247KQAAAAAgFOO+ZFsHJj7S4YIF5O9JdCdReidVJ0ky484jB8YJn/gRERERFQTpNQ5jXWXNOMzSwClDTGo6SA2JR6m6ahWyFegDAAAAAAAA4G88AAAAAAAA".to_string(),
    }
    .execute(&mut keystore)
    .await?;

    match result {
        CommandOutput::TxDigest(output) => {
            assert_eq!(
                &output.digest,
                "Fv6odr6tuuVmpDw5tyheRBQ2oivAnmudLtKBDv4T4MPE"
            );
            assert_eq!(
                &output.digest_hex,
                "0xdd9df6678f2fcdac1b1e13751afb74b0f81c9993699954ee2f0f459dd0a0da11"
            );
            assert_eq!(
                &output.signing_digest_hex,
                "0x7cfef332628f699c2aac858c5566a5bab8c7c43407038a5a76561df5c33f1eba"
            );
        }
        _ => panic!("Wrong output type"),
    }

    // Test signed transaction data
    let result = KeyToolCommand::TxDigest {
        tx_bytes: "AQAAAAAAAgAIAMqaOwAAAAAAIBEREREVBOk1DmNdZc04zNLAKUNMajpIDYlHqbpqFbIVAgIAAQEAAAEBAwAAAAABAQARERERFQTpNQ5jXWXNOMzSwClDTGo6SA2JR6m6ahWyFQFODzG01xo0l0JIwq9SzbRyvRKR/9TvCUbh8lrerlLQWT9uOykAAAAAIBTjvmRbByY+0uGCBeTvSXQnUXonVSdJMuPOIwfGCZ/4ERERERUE6TUOY11lzTjM0sApQ0xqOkgNiUepumoVshXoAwAAAAAAAOBvPAAAAAAAAAFhAKFqV1NustAADKOOOfAZIA/9HrnmA9PqwAmOrqTs7OKjaEXylfywifj2XZyBmEJYodGE89xlkDOthe+bpBIrkwEoe8lptdiMUw3h3rcxQJf3bWp9zFLP4Eq3rpQOam52cw==".to_string(),
    }
    .execute(&mut keystore)
    .await?;

    match result {
        CommandOutput::TxDigest(output) => {
            assert_eq!(
                &output.digest,
                "Fv6odr6tuuVmpDw5tyheRBQ2oivAnmudLtKBDv4T4MPE"
            );
            assert_eq!(
                &output.digest_hex,
                "0xdd9df6678f2fcdac1b1e13751afb74b0f81c9993699954ee2f0f459dd0a0da11"
            );
            assert_eq!(
                &output.signing_digest_hex,
                "0x7cfef332628f699c2aac858c5566a5bab8c7c43407038a5a76561df5c33f1eba"
            );
        }
        _ => panic!("Wrong output type"),
    }

    Ok(())
}

#[test]
async fn test_decode_sig() -> Result<(), anyhow::Error> {
    use crate::keytool::DecodedSigOutput;

    let mut keystore = Keystore::from(InMemKeystore::new_insecure_for_tests(0));

    // Test 1: Decode a direct Ed25519 signature
    let ed25519_sig = "AG4IVInVEl5QyY97PaJfvwrPi4vI4EUP6ZSKLx2oVQeAx6D6Kgc/dwj/cuc1z6J38kfKNGmXUuBz21d1Gmc+gAMNHamrF2PHX08fsHkxUQxLY8tTL3sINA2AMLW9tCo9dA==";
    let output = KeyToolCommand::DecodeSig {
        sig: ed25519_sig.to_string(),
    }
    .execute(&mut keystore)
    .await?;
    let CommandOutput::DecodeSig(decoded) = output else {
        panic!("unexpected output: {output:?}");
    };
    match decoded {
        DecodedSigOutput::Signature {
            scheme,
            public_key_base64,
            address,
            signature_hex,
        } => {
            assert_eq!(scheme, "ed25519");
            assert_eq!(
                public_key_base64,
                "AA0dqasXY8dfTx+weTFRDEtjy1Mvewg0DYAwtb20Kj10"
            );
            assert_eq!(
                address,
                "0x2a771079fddd6faed3b4ff8062112ca1cf3a33c98b027c0685b8fbff720ad261"
            );
            assert_eq!(
                signature_hex,
                "0x6e085489d5125e50c98f7b3da25fbf0acf8b8bc8e0450fe9948a2f1da8550780c7a0fa2a073f7708ff72e735cfa277f247ca34699752e073db57751a673e8003"
            );
        }
        _ => panic!("Expected Signature variant"),
    }

    // Test 2: Decode a MoveAuthenticator signature
    let move_auth_sig = "BwABAIIBgAFkYzU3M2E1OGM3ZGEwMTZhZmM2MDJkMmFlNTVhYmRhYTZkZGMzZTc5YzJmNGMwODUzOTRjNmMyNGI0ODM5ZWM0ZTAxMjBkYjY5MDJiMDk3NzcyMTE4YTc4YWJhNjIwODk4ODIwMDUxMDlmMWZhM2E1YzA3OGQ5MTY0NDQ1NjYwYwABAci6Nb73TH/9ujbVCgfZI9D7t+eEPyE5UbGeY2IpqAkeBAAAAAAAAAAA";
    let output = KeyToolCommand::DecodeSig {
        sig: move_auth_sig.to_string(),
    }
    .execute(&mut keystore)
    .await?;
    let CommandOutput::DecodeSig(decoded) = output else {
        panic!("unexpected output: {output:?}");
    };
    match decoded {
        DecodedSigOutput::MoveAuthenticator {
            call_arguments,
            type_arguments,
            object_to_authenticate,
        } => {
            assert_eq!(
                call_arguments,
                vec![
                    "0x80016463353733613538633764613031366166633630326432616535356162646161366464633365373963326634633038353339346336633234623438333965633465303132306462363930326230393737373231313861373861626136323038393838323030353130396631666133613563303738643931363434343536363063"
                ]
            );
            assert_eq!(type_arguments, serde_json::json!([]));
            assert_eq!(
                object_to_authenticate,
                serde_json::json!({"Object": {"SharedObject": {"id": "0xc8ba35bef74c7ffdba36d50a07d923d0fbb7e7843f213951b19e636229a8091e", "initial_shared_version": 4, "mutable": false}}})
            );
        }
        _ => panic!("Expected MoveAuthenticator variant"),
    }

    // Test 3: Decode signature from a full SenderSignedData (transaction with
    // signature) The fallback decodes the transaction and extracts the first
    // signature
    let full_tx = "AQAAAAAAAgAIAMqaOwAAAAAAIBEREREVBOk1DmNdZc04zNLAKUNMajpIDYlHqbpqFbIVAgIAAQEAAAEBAwAAAAABAQARERERFQTpNQ5jXWXNOMzSwClDTGo6SA2JR6m6ahWyFQFODzG01xo0l0JIwq9SzbRyvRKR/9TvCUbh8lrerlLQWT9uOykAAAAAIBTjvmRbByY+0uGCBeTvSXQnUXonVSdJMuPOIwfGCZ/4ERERERUE6TUOY11lzTjM0sApQ0xqOkgNiUepumoVshXoAwAAAAAAAOBvPAAAAAAAAAFhAKFqV1NustAADKOOOfAZIA/9HrnmA9PqwAmOrqTs7OKjaEXylfywifj2XZyBmEJYodGE89xlkDOthe+bpBIrkwEoe8lptdiMUw3h3rcxQJf3bWp9zFLP4Eq3rpQOam52cw==";
    let output = KeyToolCommand::DecodeSig {
        sig: full_tx.to_string(),
    }
    .execute(&mut keystore)
    .await?;
    let CommandOutput::DecodeSig(decoded) = output else {
        panic!("unexpected output: {output:?}");
    };
    // Verify we successfully decoded an ed25519 signature from the transaction
    match decoded {
        DecodedSigOutput::Signature {
            scheme,
            public_key_base64,
            address,
            signature_hex,
        } => {
            assert_eq!(scheme, "ed25519");
            assert_eq!(
                public_key_base64,
                "ACh7yWm12IxTDeHetzFAl/dtan3MUs/gSreulA5qbnZz"
            );
            assert_eq!(
                address,
                "0x111111111504e9350e635d65cd38ccd2c029434c6a3a480d8947a9ba6a15b215"
            );
            assert_eq!(
                signature_hex,
                "0xa16a57536eb2d0000ca38e39f019200ffd1eb9e603d3eac0098eaea4ecece2a36845f295fcb089f8f65d9c81984258a1d184f3dc659033ad85ef9ba4122b9301"
            );
        }
        _ => panic!("Expected Signature variant"),
    }

    Ok(())
}
