// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use iota_types::crypto::{Ed25519IotaSignature, IotaSignatureInner, SignatureScheme};
use tempfile::TempDir;

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
        .import_from_mnemonic(&phrase, SignatureScheme::ED25519, None)
        .unwrap();
    assert_eq!(scheme.flag(), Ed25519IotaSignature::SCHEME.flag());
    assert_eq!(address, imported_address);
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
