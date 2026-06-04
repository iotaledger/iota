// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Static decoders — no Move VM, no store, no chain context.
//!
//! These free functions answer the "what does this blob reference?" questions
//! a wallet or explorer asks before it has fetched any state: which objects a
//! transaction touches, whether a signature is a [`MoveAuthenticator`], and
//! which dynamic-field IDs back an abstract account. They are pure functions of
//! their byte inputs.

use fastcrypto::traits::ToFromBytes;
use iota_sdk_types::{Address as IotaAddress, ObjectId, TypeTag};
use iota_types::{
    account_abstraction::account::AuthenticatorFunctionRefV1Key,
    dynamic_field::{DynamicFieldInfo, derive_dynamic_field_id},
    move_authenticator::MoveAuthenticator,
    signature::GenericSignature,
    transaction::{TransactionData, TransactionDataAPI},
};

use crate::error::{DecodeError, VmSdkError};

/// The static shape of a decoded transaction: who sends it, its gas
/// parameters, and every object ID it references (inputs, gas payments,
/// receiving objects). Returned without touching the VM or a store.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DecodedTransaction {
    pub sender: IotaAddress,
    pub gas_budget: u64,
    pub gas_price: u64,
    /// All distinct object IDs the transaction references, sorted and
    /// de-duplicated.
    pub required_objects: Vec<ObjectId>,
}

/// Decode BCS-encoded [`TransactionData`] into its [`DecodedTransaction`]
/// summary.
pub fn decode_transaction(bytes: &[u8]) -> Result<DecodedTransaction, VmSdkError> {
    let tx: TransactionData =
        bcs::from_bytes(bytes).map_err(|e| DecodeError::new("bcs decode TransactionData", e))?;

    let input_object_kinds = tx
        .input_objects()
        .map_err(|e| DecodeError::new("collect input objects", e))?;

    let mut ids: Vec<ObjectId> = input_object_kinds
        .iter()
        .map(|kind| kind.object_id())
        .collect();
    ids.extend(tx.gas().iter().map(|gas_ref| gas_ref.object_id));
    ids.extend(tx.receiving_objects().iter().map(|r| r.object_id));
    ids.sort();
    ids.dedup();

    Ok(DecodedTransaction {
        sender: tx.sender(),
        gas_budget: tx.gas_budget(),
        gas_price: tx.gas_price(),
        required_objects: ids,
    })
}

/// Decode a raw `[flag || …]` signature blob, returning the
/// [`MoveAuthenticator`] if and only if the blob is one. Standard signature
/// schemes (Ed25519, Secp256k1, …) decode to `Ok(None)` — they carry no
/// authenticator.
pub fn decode_move_authenticator(
    signature: &[u8],
) -> Result<Option<MoveAuthenticator>, VmSdkError> {
    let sig = GenericSignature::from_bytes(signature)
        .map_err(|e| DecodeError::new("decode signature", e))?;
    match sig {
        GenericSignature::MoveAuthenticator(auth) => Ok(Some(auth)),
        _ => Ok(None),
    }
}

/// Derive the on-chain dynamic-field ID that stores the
/// `AuthenticatorFunctionRefV1` for `account_object_id`.
///
/// A `MoveAuthenticator` verifier needs this object loaded to know which Move
/// function authenticates the account; this tells a caller which extra object
/// to fetch. Mirrors the node's own derivation.
pub fn auth_function_field_id(account_object_id: ObjectId) -> Result<ObjectId, VmSdkError> {
    derive_dynamic_field_id(
        account_object_id,
        &AuthenticatorFunctionRefV1Key::tag().into(),
        &AuthenticatorFunctionRefV1Key::default().to_bcs_bytes(),
    )
    .map_err(|e| DecodeError::new("derive auth function field id", e).into())
}

/// Derive the on-chain ID of a `Field<K, V>` wrapper object from its parent ID,
/// the key's type, and the key's BCS bytes. Set `is_dynamic_object_field` when
/// the field is a dynamic *object* field (the key type is then wrapped).
///
/// Mirrors `iota_types::dynamic_field::derive_dynamic_field_id`; lets a caller
/// walk a dynamic-field tree offline.
pub fn derive_field_id(
    parent: ObjectId,
    key_type: TypeTag,
    key_bcs: &[u8],
    is_dynamic_object_field: bool,
) -> Result<ObjectId, VmSdkError> {
    let wrapper_type = if is_dynamic_object_field {
        DynamicFieldInfo::dynamic_object_field_wrapper(key_type).into()
    } else {
        key_type
    };
    derive_dynamic_field_id(parent, &wrapper_type, key_bcs)
        .map_err(|e| DecodeError::new("derive field id", e).into())
}
