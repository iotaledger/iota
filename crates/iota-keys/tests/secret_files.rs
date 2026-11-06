// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt};

use fastcrypto::traits::EncodeDecodeBase64;
use iota_keys::{
    key_derive::generate_new_key,
    keypair_file::{
        read_authority_keypair_from_file, read_keypair_from_file, write_authority_keypair_to_file,
        write_keypair_to_file,
    },
    keystore::{AccountKeystore, FileBasedKeystore},
};
use iota_sdk_crypto::ToFromBech32;
use iota_sdk_types::SignatureScheme;
use iota_types::crypto::get_authority_key_pair;

fn assert_private(path: &std::path::Path) {
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn keystore_save_is_private_and_loads_existing_permissive_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("iota.keystore");
    let mut store = FileBasedKeystore::new(&path).unwrap();
    let (address, _, _) = store
        .generate_and_add_new_key(SignatureScheme::Ed25519, Some("test".into()), None, None)
        .unwrap();
    assert_private(&path);

    // Existing keystores remain readable; their next save tightens permissions
    // without changing the stored keys or serialization format.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let previous = fs::read(&path).unwrap();
    let store = FileBasedKeystore::new(&path).unwrap();
    assert!(store.addresses().contains(&address));
    store.save().unwrap();
    assert_private(&path);
    assert_eq!(fs::read(path).unwrap(), previous);
}

#[test]
fn account_keypair_save_is_private_and_preserves_encoding() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("account.key");
    let (_, key, _, _) = generate_new_key(SignatureScheme::Ed25519, None, None).unwrap();
    write_keypair_to_file(&key, &path).unwrap();
    assert_private(&path);
    assert_eq!(fs::read_to_string(&path).unwrap(), key.to_bech32().unwrap());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    write_keypair_to_file(&key, &path).unwrap();
    assert_private(&path);
    assert_eq!(
        read_keypair_from_file(path).unwrap().to_bech32().unwrap(),
        key.to_bech32().unwrap()
    );
}

#[test]
fn authority_keypair_save_is_private_and_preserves_encoding() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.key");
    let (_, key) = get_authority_key_pair();
    write_authority_keypair_to_file(&key, &path).unwrap();
    assert_private(&path);
    assert_eq!(fs::read_to_string(&path).unwrap(), key.encode_base64());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    write_authority_keypair_to_file(&key, &path).unwrap();
    assert_private(&path);
    assert_eq!(
        read_authority_keypair_from_file(path)
            .unwrap()
            .encode_base64(),
        key.encode_base64()
    );
}
