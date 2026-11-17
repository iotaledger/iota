// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::borrow::Cow;

use iota_sdk_types::{Address, Identifier, crypto::PersonalMessage};
use serde::Serialize;

pub const SMART_ACCOUNT_BUILTIN_AUTH_MODULE_NAME: Identifier =
    Identifier::from_static("smart_account_builtin_auth");

pub const KEY_PROOF_MESSAGE_STRUCT_NAME: Identifier = Identifier::from_static("KeyProofMessage");

/// The message a key holder signs, as a personal message, to approve
/// attaching the key to a `SmartAccount`. Mirrors the Move struct
/// `smart_account_builtin_auth::KeyProofMessage`, field for field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyProofMessage {
    /// The 32-byte chain identifier of the network.
    pub chain_identifier: Vec<u8>,
    /// The account the key is attached to. For a new account, the
    /// transaction sender.
    pub account: Address,
    /// `true` for a new account, `false` for an existing one.
    pub new_account: bool,
    /// The nonce of the transaction that attaches the key.
    pub nonce: Vec<u8>,
    /// The last epoch in which the proof is valid.
    pub valid_until_epoch: u64,
}

impl KeyProofMessage {
    /// Returns the bytes the key holder signs: the BCS bytes of the Move type
    /// name of `KeyProofMessage`, taken with original IDs, followed by the
    /// BCS bytes of the message.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes =
            bcs::to_bytes(&type_name()).expect("a type name is always BCS-serializable");
        bytes.extend(bcs::to_bytes(self).expect("KeyProofMessage is always BCS-serializable"));
        bytes
    }

    /// Returns the personal message to sign, for example with
    /// `IotaSigner::sign_personal_message`.
    pub fn personal_message(&self) -> PersonalMessage<'static> {
        PersonalMessage(Cow::Owned(self.signing_bytes()))
    }
}

/// The Move type name of `KeyProofMessage`, as `std::type_name` writes it:
/// the full 64-digit hex address without `0x`, the module and the struct.
fn type_name() -> String {
    format!(
        "{}::{SMART_ACCOUNT_BUILTIN_AUTH_MODULE_NAME}::{KEY_PROOF_MESSAGE_STRUCT_NAME}",
        Address::FRAMEWORK.to_raw_hex()
    )
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/key_proof_tests.rs"]
mod key_proof_tests;
