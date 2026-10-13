// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The messages a key holder signs, as an IOTA personal message, to approve
//! attaching a public key to an account. The Move module
//! `iota::public_key_proof` rebuilds the same bytes and checks the signature.

use iota_sdk_types::{Address, ObjectId, SignatureScheme};

/// The first line of every proof message: the type name of
/// `iota::public_key::PublicKey`, as `type_name::get_with_original_ids`
/// prints it.
const MESSAGE_PREFIX: &str =
    "0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey";

/// Returns the message that approves attaching the `scheme` key
/// `raw_public_key` to the existing `account` at `key_change_count`, valid
/// until `valid_until_epoch`.
pub fn account_proof_message(
    scheme: SignatureScheme,
    raw_public_key: &[u8],
    account: Address,
    key_change_count: u64,
    valid_until_epoch: u64,
) -> Vec<u8> {
    format!(
        "{}\naccount: 0x{}\nkey changes on this account: {key_change_count}\nvalid until epoch: {valid_until_epoch}",
        message_head(scheme, raw_public_key),
        hex::encode(account.as_bytes()),
    )
    .into_bytes()
}

/// Returns the message that approves attaching the `scheme` key
/// `raw_public_key` to a new account created by a transaction from `sender`
/// under the `AccountFactory` `factory`, its nonce key `nonce_key` and that
/// key's sequence number `sequence`, valid until `valid_until_epoch`.
pub fn creation_proof_message(
    scheme: SignatureScheme,
    raw_public_key: &[u8],
    factory: ObjectId,
    sender: Address,
    nonce_key: u64,
    sequence: u64,
    valid_until_epoch: u64,
) -> Vec<u8> {
    format!(
        "{}\nfactory: 0x{}\nsender: 0x{}\nnonce key: {nonce_key}\nsequence: {sequence}\nvalid until epoch: {valid_until_epoch}",
        message_head(scheme, raw_public_key),
        hex::encode(factory.as_bytes()),
        hex::encode(sender.as_bytes()),
    )
    .into_bytes()
}

fn message_head(scheme: SignatureScheme, raw_public_key: &[u8]) -> String {
    let mut prefixed_key = vec![scheme.to_u8()];
    prefixed_key.extend_from_slice(raw_public_key);
    format!(
        "{MESSAGE_PREFIX}\npublic key: {}",
        hex::encode(prefixed_key)
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    const ED25519_PUBLIC_KEY: &str =
        "d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f";

    // The messages below are the ones the Move tests in
    // `public_key_proof_tests.move` build and sign.

    #[test]
    fn account_proof_message_matches_move() {
        let message = account_proof_message(
            SignatureScheme::Ed25519,
            &hex::decode(ED25519_PUBLIC_KEY).unwrap(),
            Address::from_str("0xacc").unwrap(),
            0,
            5,
        );
        assert_eq!(
            String::from_utf8(message).unwrap(),
            "0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey\n\
             public key: 00d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f\n\
             account: 0x0000000000000000000000000000000000000000000000000000000000000acc\n\
             key changes on this account: 0\n\
             valid until epoch: 5"
        );
    }

    #[test]
    fn creation_proof_message_matches_move() {
        let message = creation_proof_message(
            SignatureScheme::Ed25519,
            &hex::decode(ED25519_PUBLIC_KEY).unwrap(),
            ObjectId::from_str(
                "0x034401905bebdf8c04f3cd5f04f442a39372c8dc321c29edfb4f9cb30b23ab96",
            )
            .unwrap(),
            Address::ZERO,
            0,
            0,
            5,
        );
        assert_eq!(
            String::from_utf8(message).unwrap(),
            "0000000000000000000000000000000000000000000000000000000000000002::public_key::PublicKey\n\
             public key: 00d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f\n\
             factory: 0x034401905bebdf8c04f3cd5f04f442a39372c8dc321c29edfb4f9cb30b23ab96\n\
             sender: 0x0000000000000000000000000000000000000000000000000000000000000000\n\
             nonce key: 0\n\
             sequence: 0\n\
             valid until epoch: 5"
        );
    }
}
