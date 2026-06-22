// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use fastcrypto::encoding::{Base64, Encoding};
use iota_sdk_types::crypto::PublicKey as SdkPublicKey;
use iota_types::{
    crypto::{EncodeDecodeBase64, PublicKey, SignatureScheme},
    multisig::{MultiSigPublicKey, MultisigMember},
};

/// Read a string as a Base64 encoded ED25519 public key.
pub(crate) fn public_key_from_base64(base64: &str) -> Result<PublicKey> {
    let bytes = Base64::decode(base64).map_err(|_| anyhow!("Failed to decode base64"))?;

    PublicKey::try_from_bytes(SignatureScheme::ED25519, &bytes)
        .map_err(|_| anyhow!("Failed to read public key"))
}

/// Combine public keys into a MultiSig. Keys are deduplicated before generation
/// as multisigs cannot contain the same public key twice.
/// TODO remove conversion https://github.com/iotaledger/iota/issues/11590
pub(crate) fn combine_keys(keys: impl IntoIterator<Item = PublicKey>) -> Result<MultiSigPublicKey> {
    let members: Vec<_> = keys
        .into_iter()
        .map(|key| {
            let b64 = key.encode_base64();
            let pk = SdkPublicKey::from_base64(&b64)
                .map_err(|e| anyhow!("Failed to convert public key: {e}"))?;
            Ok((b64, MultisigMember::new(pk, 1)))
        })
        .collect::<Result<BTreeMap<_, _>>>()?
        .into_values()
        .collect();

    Ok(MultiSigPublicKey::new(members, 1)?)
}
