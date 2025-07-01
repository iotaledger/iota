// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Ok, Result};
use fastcrypto::{
    encoding::{Encoding, Hex},
    traits::ToFromBytes,
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::types::{
    base_types::IotaAddress,
    multisig::{MultiSig, MultiSigPublicKey},
    signature::GenericSignature,
};

/// Reconstructs a `GenericSignature` from a hex-encoded pure signature string.
///
/// The IOTA `GenericSignature` format includes a flag byte (usually 0x00),
/// followed by the 64-byte (128 hex character) pure signature and then
/// the signer's public key.
///
/// # Arguments
/// * `keystore` - The keystore containing the signer's public key.
/// * `addr` - The address whose public key should be used.
/// * `encoded_signature` - A hex-encoded string of the 64-byte pure signature.
///
/// # Returns
/// A `GenericSignature` with reconstructed flag + signature + public key
/// format.
pub fn restore_signagure_bytes_to_generic(
    keystore: &FileBasedKeystore,
    addr: IotaAddress,
    encoded_signature: &str,
) -> Result<GenericSignature> {
    let flag_prefix_bytes = &[0u8][..]; // Indicates signature scheme (0 = Ed25519)
    let pure_signatures_bytes = &Hex::decode(encoded_signature)?[..];
    let pub_key = keystore.get_key(&addr)?.public();
    let pub_key_bytes = pub_key.as_ref();
    Ok(GenericSignature::from_bytes(
        &[flag_prefix_bytes, pure_signatures_bytes, pub_key_bytes].concat(),
    )?)
}

/// Extracts the pure (64-byte) signature portion from a `GenericSignature`
/// as a 128-character hex string, skipping the first byte (flag prefix) and the
/// last 32 bytes (64 chars) of public key.
///
/// # Arguments
/// * `signature` - The full `GenericSignature` which includes flag + signature
///   + pubkey.
///
/// # Returns
/// A `String` containing the 128-character hex representation of the pure
/// signature bytes.
///
/// # Notes
/// This function assumes the Ed25519 scheme (flag byte 0x00) and extracts only
/// the signature.
pub fn extract_pure_signature(signature: &GenericSignature) -> String {
    let flag_prefix = 2;
    let pure_signature_length = 128;
    let hex_encoded_signature = Hex::encode(signature);
    hex_encoded_signature
        .chars()
        .skip(flag_prefix)
        .take(pure_signature_length)
        .collect()
}

/// Helper to construct a multisig from two signers (with weighted threshold).
pub fn build_multisig(
    keystore: &FileBasedKeystore,
    signers: &[IotaAddress],
    weights: &[u8],
    threshold: u16,
    signatures: Vec<GenericSignature>,
) -> Result<GenericSignature> {
    Ok(MultiSig::combine(
        signatures,
        build_multisig_pub_key(keystore, signers, weights, threshold)?,
    )?
    .into())
}

/// Helper to construct a multisig from two signers (with weighted threshold).
pub fn build_multisig_pub_key(
    keystore: &FileBasedKeystore,
    signers: &[IotaAddress],
    weights: &[u8],
    threshold: u16,
) -> Result<MultiSigPublicKey> {
    let public_keys = signers
        .iter()
        .map(|addr| keystore.get_key(addr).map(|k| k.public()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MultiSigPublicKey::new(
        public_keys,
        weights.to_vec(),
        threshold,
    )?)
}
