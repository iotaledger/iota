// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::{self},
    str::FromStr,
};

use fastcrypto::{hash::HashFunction, traits::EncodeDecodeBase64};
use iota_keys::{
    key_derive::generate_new_key,
    keystore::{AccountKeystore, FileBasedKeystore, InMemKeystore, Keystore, LegacyAlias},
};
use iota_types::{
    base_types::{IOTA_ADDRESS_LENGTH, IotaAddress},
    crypto::{DefaultHash, Ed25519IotaSignature, IotaSignatureInner, SignatureScheme},
};
use tempfile::TempDir;

#[test]
fn alias_exists_test() {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    keystore
        .generate_and_add_new_key(
            SignatureScheme::ED25519,
            Some("my_alias_test".to_string()),
            None,
            None,
        )
        .unwrap();
    let aliases = keystore.alias_names();
    assert_eq!(1, aliases.len());
    assert_eq!(vec!["my_alias_test"], aliases);
    assert!(!aliases.contains(&"alias_does_not_exist"));
}

#[test]
fn create_alias_if_not_exists_test() {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());

    let alias = Some("my_alias_test".to_string());
    keystore
        .generate_and_add_new_key(SignatureScheme::ED25519, alias.clone(), None, None)
        .unwrap();

    // test error first
    let create_alias_result = keystore.create_alias(alias);
    assert!(create_alias_result.is_err());
    // test expected result
    let create_alias_result = keystore.create_alias(Some("test".to_string()));
    assert_eq!("test".to_string(), create_alias_result.unwrap());
    assert!(keystore.create_alias(Some("_test".to_string())).is_err());
    assert!(keystore.create_alias(Some("-A".to_string())).is_err());
    assert!(keystore.create_alias(Some("1A".to_string())).is_err());
    assert!(keystore.create_alias(Some("&&AA".to_string())).is_err());
}

#[test]
fn update_alias_test() {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    keystore
        .generate_and_add_new_key(
            SignatureScheme::ED25519,
            Some("my_alias_test".to_string()),
            None,
            None,
        )
        .unwrap();
    let aliases = keystore.alias_names();
    assert_eq!(1, aliases.len());
    assert_eq!(vec!["my_alias_test"], aliases);

    // read the alias file again and check if it was saved
    let keystore1 = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    let aliases1 = keystore1.alias_names();
    assert_eq!(vec!["my_alias_test"], aliases1);

    let update = keystore.update_alias("alias_does_not_exist", None);
    assert!(update.is_err());

    let _ = keystore.update_alias("my_alias_test", Some("new_alias"));
    let aliases = keystore.alias_names();
    assert_eq!(vec!["new_alias"], aliases);

    // check that it errors on empty alias
    assert!(keystore.update_alias("new_alias", Some(" ")).is_err());
    assert!(keystore.update_alias("new_alias", Some("   ")).is_err());
    // check that alias is trimmed
    assert!(keystore.update_alias("new_alias", Some("  o ")).is_ok());
    assert_eq!(vec!["o"], keystore.alias_names());
    // check the regex works and new alias can be only [A-Za-z][A-Za-z0-9-_]*
    assert!(keystore.update_alias("o", Some("_alias")).is_err());
    assert!(keystore.update_alias("o", Some("-alias")).is_err());
    assert!(keystore.update_alias("o", Some("123")).is_err());

    let update = keystore.update_alias("o", None).unwrap();
    let aliases = keystore.alias_names();
    assert_eq!(vec![&update], aliases);

    // check that updating alias does not allow duplicates
    keystore
        .generate_and_add_new_key(
            SignatureScheme::ED25519,
            Some("my_alias_test".to_string()),
            None,
            None,
        )
        .unwrap();
    assert!(
        keystore
            .update_alias("my_alias_test", Some(&update))
            .is_err()
    );
}

#[test]
fn update_alias_in_memory_test() {
    let mut keystore = Keystore::InMem(InMemKeystore::new_insecure_for_tests(0));
    keystore
        .generate_and_add_new_key(
            SignatureScheme::ED25519,
            Some("my_alias_test".to_string()),
            None,
            None,
        )
        .unwrap();
    let aliases = keystore.alias_names();
    assert_eq!(1, aliases.len());
    assert_eq!(vec!["my_alias_test"], aliases);

    let update = keystore.update_alias("alias_does_not_exist", None);
    assert!(update.is_err());

    let _ = keystore.update_alias("my_alias_test", Some("new_alias"));
    let aliases = keystore.alias_names();
    assert_eq!(vec!["new_alias"], aliases);

    let update = keystore.update_alias("new_alias", None).unwrap();
    let aliases = keystore.alias_names();
    assert_eq!(vec![&update], aliases);
}

#[test]
fn mnemonic_test() {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    let (address, phrase, scheme) = keystore
        .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
        .unwrap();

    let keystore_path_2 = temp_dir.path().join("iota2.keystore");
    let mut keystore2 = Keystore::from(FileBasedKeystore::new(&keystore_path_2).unwrap());
    let imported_address = keystore2
        .import_from_mnemonic(&phrase, SignatureScheme::ED25519, None, None)
        .unwrap();
    assert_eq!(scheme.flag(), Ed25519IotaSignature::SCHEME.flag());
    assert_eq!(address, imported_address);
}

/// This test confirms rust's implementation of mnemonic is the same with the
/// IOTA Wallet
#[test]
fn iota_wallet_address_mnemonic_test() -> Result<(), anyhow::Error> {
    let phrase = "result crisp session latin must fruit genuine question prevent start coconut brave speak student dismiss";
    let expected_address = IotaAddress::from_str(
        "0x61d6b774051d92c8c4863782933e915f88c433e9542ca534b233dc8ef1155137",
    )?;

    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());

    keystore
        .import_from_mnemonic(phrase, SignatureScheme::ED25519, None, None)
        .unwrap();

    let pubkey = keystore.keys()[0].public();
    assert_eq!(pubkey.flag(), Ed25519IotaSignature::SCHEME.flag());

    let mut hasher = DefaultHash::default();
    hasher.update(pubkey);
    let g_arr = hasher.finalize();
    let mut res = [0u8; IOTA_ADDRESS_LENGTH];
    res.copy_from_slice(&AsRef::<[u8]>::as_ref(&g_arr)[..IOTA_ADDRESS_LENGTH]);
    let address = IotaAddress::try_from(res.as_slice())?;

    assert_eq!(expected_address, address);

    Ok(())
}

#[test]
fn keystore_display_test() -> Result<(), anyhow::Error> {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    assert!(keystore.to_string().contains("iota.keystore"));
    assert!(!keystore.to_string().contains("keys:"));
    Ok(())
}

#[test]
fn get_alias_by_address_test() {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    let alias = "my_alias_test".to_string();
    let keypair = keystore
        .generate_and_add_new_key(SignatureScheme::ED25519, Some(alias.clone()), None, None)
        .unwrap();
    assert_eq!(alias, keystore.get_alias_by_address(&keypair.0).unwrap());

    // Test getting an alias of an address that is not in keystore
    let address = generate_new_key(SignatureScheme::ED25519, None, None).unwrap();
    assert!(keystore.get_alias_by_address(&address.0).is_err())
}

#[test]
fn remove_key_test() {
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("iota.keystore");
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());

    let address = keystore
        .generate_and_add_new_key(
            SignatureScheme::ED25519,
            Some("test_key".to_string()),
            None,
            None,
        )
        .unwrap()
        .0;

    let keystore_content = fs::read_to_string(&keystore_path).unwrap();
    assert!(keystore_content.contains("test_key"));
    assert!(keystore.get_key(&address).is_ok());

    keystore.remove_key(&address).unwrap();

    // Verify alias is removed from file
    let keystore_content = fs::read_to_string(&keystore_path).unwrap();
    assert!(!keystore_content.contains("test_key"));
    assert!(keystore.get_key(&address).is_err());
}

#[test]
fn test_migrate_v1_to_v2_no_aliases() {
    // This test creates a v1 keystore file without aliases and migrates it to v2
    // format. It first re-creates the v1 aliases file, then migrates it to v2.
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("test.keystore_extension");

    let (_, keypair, _, _) = generate_new_key(SignatureScheme::ED25519, None, None).unwrap();
    // Create a v1 keystore file with a single key
    let private_keys = vec![keypair.encode().unwrap()];
    let keystore_data = serde_json::to_string_pretty(&private_keys).unwrap();
    fs::write(&keystore_path, keystore_data).unwrap();

    let keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    assert!(keystore_path.exists());
    assert_eq!(1, keystore.aliases().len());
    assert_eq!(
        *keystore
            .get_key(&IotaAddress::from(&keypair.public()))
            .unwrap()
            .as_keypair()
            .unwrap(),
        keypair,
    );

    let mut backup_keystore_path = keystore_path.clone();
    backup_keystore_path.set_extension("keystore_extension.migrated");
    assert!(backup_keystore_path.exists());

    let mut backup_aliases_path = keystore_path;
    backup_aliases_path.set_extension("aliases.migrated");
    assert!(backup_aliases_path.exists());
}

#[test]
fn test_migrate_v1_to_v2_with_aliases() {
    // This test creates a v1 keystore file with the corresponding aliases and
    // migrates it to v2 format.
    let temp_dir = TempDir::new().unwrap();
    let keystore_path = temp_dir.path().join("test"); // No extension since it is not required
    let mut aliases_path = keystore_path.clone();
    aliases_path.set_extension("aliases");

    let (_, keypair, _, _) = generate_new_key(SignatureScheme::ED25519, None, None).unwrap();
    // Create a v1 keystore file with a single key
    let private_keys = vec![keypair.encode().unwrap()];
    let keystore_data = serde_json::to_string_pretty(&private_keys).unwrap();
    fs::write(&keystore_path, keystore_data).unwrap();

    // Create an aliases file with a single alias
    let aliases = vec![LegacyAlias {
        alias: "test_alias".to_string(),
        public_key_base64: keypair.public().encode_base64(),
    }];
    let aliases_data = serde_json::to_string_pretty(&aliases).unwrap();
    fs::write(&aliases_path, aliases_data).unwrap();

    let keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    assert!(keystore_path.exists());
    assert_eq!(1, keystore.aliases().len());
    assert_eq!(
        *keystore
            .get_key(&IotaAddress::from(&keypair.public()))
            .unwrap()
            .as_keypair()
            .unwrap(),
        keypair,
    );
    assert_eq!(
        keystore
            .get_alias_by_address(&IotaAddress::from(&keypair.public()))
            .unwrap(),
        "test_alias"
    );

    let mut backup_keystore_path = keystore_path.clone();
    backup_keystore_path.set_extension("migrated");
    assert!(backup_keystore_path.exists());

    let mut backup_aliases_path = keystore_path;
    backup_aliases_path.set_extension("aliases.migrated");
    assert!(backup_aliases_path.exists());
}
