// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use anyhow::{anyhow, bail};
use fastcrypto::{
    encoding::{Base64, Encoding, Hex},
    traits::EncodeDecodeBase64,
};
use iota_sdk_crypto::{
    ToFromBase64, ToFromBech32, ToFromBytes as _, secp256k1::Secp256k1PrivateKey,
    simple::SimpleKeypair,
};
use iota_sdk_types::SignatureScheme;
use iota_types::crypto::{AuthorityKeyPair, NetworkKeyPair, simple_to_network_keypair};

/// Write Bech32 encoded `flag || privkey` to file.
pub fn write_keypair_to_file<P: AsRef<std::path::Path>>(
    keypair: &SimpleKeypair,
    path: P,
) -> anyhow::Result<()> {
    let contents = keypair.to_bech32().map_err(|e| anyhow!(e))?;
    std::fs::write(path, contents)?;
    Ok(())
}

/// Write Base64 encoded `privkey` to file.
pub fn write_authority_keypair_to_file<P: AsRef<std::path::Path>>(
    keypair: &AuthorityKeyPair,
    path: P,
) -> anyhow::Result<()> {
    let contents = keypair.encode_base64();
    std::fs::write(path, contents)?;
    Ok(())
}

/// Read from file as Base64 encoded `privkey` and return a AuthorityKeyPair.
pub fn read_authority_keypair_from_file<P: AsRef<std::path::Path>>(
    path: P,
) -> anyhow::Result<AuthorityKeyPair> {
    let contents = std::fs::read_to_string(path)?;
    AuthorityKeyPair::decode_base64(contents.as_str().trim()).map_err(|e| anyhow!(e))
}

/// Read from file as Bech32 encoded `flag || privkey` and return a
/// SimpleKeypair.
pub fn read_keypair_from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<SimpleKeypair> {
    let contents = std::fs::read_to_string(path)?;
    SimpleKeypair::from_bech32(contents.as_str().trim()).map_err(|e| anyhow!(e))
}

/// Read from file as Base64 encoded `flag || privkey` and return a
/// NetworkKeyPair.
pub fn read_network_keypair_from_file<P: AsRef<std::path::Path>>(
    path: P,
) -> anyhow::Result<NetworkKeyPair> {
    let kp = read_keypair_from_file(path)?;
    simple_to_network_keypair(&kp)
}

/// Read a SimpleKeypair from a file. The content could be any of the
/// following:
/// - Base64 encoded `flag || privkey` for ECDSA key
/// - Base64 encoded `privkey` for Raw key
/// - Bech32 encoded private key prefixed with `iotaprivkey`
/// - Hex encoded `privkey` for Raw key
///
/// If `require_secp256k1` is true, it will return an error if the key is not
/// Secp256k1.
pub fn read_key(path: &PathBuf, require_secp256k1: bool) -> Result<SimpleKeypair, anyhow::Error> {
    if !path.exists() {
        bail!("Key file not found at path: {path:?}");
    }
    let file_contents = std::fs::read_to_string(path)?;
    let contents = file_contents.as_str().trim();

    // Try base64 encoded SimpleKeypair `flag || privkey`
    if let Some(key) = Base64::decode(contents)
        .ok()
        .and_then(|bytes| SimpleKeypair::from_bytes(&bytes).ok())
    {
        if require_secp256k1 && key.scheme() != SignatureScheme::Secp256k1 {
            bail!("Key is not Secp256k1");
        }
        return Ok(key);
    }

    // Try base64 encoded Raw Secp256k1 key `privkey`
    if let Ok(key) = Secp256k1PrivateKey::from_base64(contents) {
        return Ok(SimpleKeypair::from(key));
    }

    // Try Bech32 encoded 33-byte `flag || private key` starting with `iotaprivkey`
    // prefix. This is the format of a private key exported from IOTA Wallet or
    // iota.keystore.
    if let Ok(key) = SimpleKeypair::from_bech32(contents) {
        if require_secp256k1 && key.scheme() != SignatureScheme::Secp256k1 {
            bail!("Key is not Secp256k1");
        }
        return Ok(key);
    }

    // Try hex encoded Raw key `privkey`
    if let Ok(bytes) = Hex::decode(contents).map_err(|e| anyhow!("Error decoding hex: {e:?}")) {
        if let Ok(key) = Secp256k1PrivateKey::from_bytes(&bytes) {
            return Ok(SimpleKeypair::from(key));
        }
    }

    Err(anyhow!("Error decoding key from {path:?}"))
}
