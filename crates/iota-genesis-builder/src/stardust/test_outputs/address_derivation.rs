// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Address derivation utilities for test outputs.
//!
//! This module provides functions to derive Stardust Ed25519Addresses from
//! mnemonics using the BIP44 derivation path format.

use fastcrypto::hash::{Blake2b256, HashFunction};
use iota_sdk_crypto::{FromMnemonic, ed25519::Ed25519PrivateKey};
use iota_stardust_types::block::address::Ed25519Address;

/// Derive an Ed25519Address from a mnemonic phrase using BIP44 derivation path.
///
/// The derivation path format for Ed25519 is:
/// `m/44'/{coin_type}'/{account}'/{change}'/{address_index}'`
///
/// # Arguments
/// * `mnemonic` - The BIP39 mnemonic phrase
/// * `coin_type` - The SLIP-44 coin type (e.g., 4218 for IOTA)
/// * `account_index` - The account index in the derivation path
/// * `address_index` - The address index in the derivation path
/// * `internal` - Whether this is an internal (change) address (0 = public, 1 =
///   internal)
///
/// # Returns
/// The derived Ed25519Address
pub fn derive_address(
    mnemonic: &str,
    coin_type: u32,
    account_index: u32,
    address_index: u32,
    internal: bool,
) -> anyhow::Result<Ed25519Address> {
    let change = if internal { 1 } else { 0 };
    let path = format!(
        "m/44'/{}'/{}'/{}'/{}'",
        coin_type, account_index, change, address_index
    );

    let private_key = Ed25519PrivateKey::from_mnemonic_with_path(mnemonic, path, None)?;
    let public_key = private_key.public_key();
    let public_key_bytes: [u8; 32] = public_key.into_inner();

    // Stardust Ed25519Address is the Blake2b-256 hash of the public key
    let address_bytes: [u8; 32] = Blake2b256::digest(public_key_bytes).into();

    Ok(Ed25519Address::new(address_bytes))
}

/// Derive multiple Ed25519Addresses from a mnemonic phrase.
///
/// # Arguments
/// * `mnemonic` - The BIP39 mnemonic phrase
/// * `coin_type` - The SLIP-44 coin type (e.g., 4218 for IOTA)
/// * `account_index` - The account index in the derivation path
/// * `address_range` - The range of address indices to derive
/// * `internal` - Whether these are internal (change) addresses
///
/// # Returns
/// A vector of derived Ed25519Addresses
pub fn derive_addresses(
    mnemonic: &str,
    coin_type: u32,
    account_index: u32,
    address_range: std::ops::Range<u32>,
    internal: bool,
) -> anyhow::Result<Vec<Ed25519Address>> {
    address_range
        .map(|address_index| {
            derive_address(mnemonic, coin_type, account_index, address_index, internal)
        })
        .collect()
}
