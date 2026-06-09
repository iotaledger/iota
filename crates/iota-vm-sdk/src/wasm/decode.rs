// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Static, VM-free decode helpers exposed to JS: turn a base-64 BCS transaction
//! or signature blob into the object IDs the JS side must fetch before
//! [`simulate`](super::simulate::simulate).

use fastcrypto::traits::ToFromBytes;
use iota_types::signature::GenericSignature;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::{b64_decode, err_to_js};
use crate::decode::{auth_function_field_id, decode_transaction as decode_transaction_inner};

/// Result of decoding a transaction: the object IDs the JS side must fetch.
#[derive(Serialize, Deserialize)]
pub struct DecodedTransactionJs {
    /// Transaction sender address, hex-encoded.
    pub sender: String,
    /// Gas budget declared by the transaction.
    pub gas_budget: u64,
    /// Gas price declared by the transaction.
    pub gas_price: u64,
    /// All distinct object IDs the transaction references, hex-encoded.
    pub required_objects: Vec<String>,
}

/// Decode a base-64 BCS [`TransactionData`] into the sender, gas parameters,
/// and the set of object IDs the JS side must fetch before
/// [`simulate`](super::simulate::simulate).
#[wasm_bindgen]
pub fn decode_transaction(tx_b64: &str) -> Result<JsValue, JsError> {
    let bytes = b64_decode(tx_b64)?;
    let decoded = decode_transaction_inner(&bytes).map_err(err_to_js)?;
    let out = DecodedTransactionJs {
        sender: decoded.sender.to_string(),
        gas_budget: decoded.gas_budget,
        gas_price: decoded.gas_price,
        required_objects: decoded
            .required_objects
            .iter()
            .map(|id| id.to_hex())
            .collect(),
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsError::new(&e.to_string()))
}

/// Derive the on-chain ID of a `Field<K, V>` wrapper object. Mirrors
/// [`crate::derive_field_id`].
#[wasm_bindgen]
pub fn derive_field_id(
    parent_id_hex: &str,
    key_type_repr: &str,
    key_bcs_b64: &str,
    is_dynamic_object_field: bool,
) -> Result<String, JsError> {
    let parent = parent_id_hex
        .parse()
        .map_err(|e| JsError::new(&format!("parent id: {e}")))?;
    let key_bytes = b64_decode(key_bcs_b64)?;
    let key_type = iota_types::parse_iota_type_tag(key_type_repr)
        .map_err(|e| JsError::new(&format!("key type: {e}")))?;
    let id = crate::derive_field_id(parent, key_type, &key_bytes, is_dynamic_object_field)
        .map_err(err_to_js)?;
    Ok(id.to_hex())
}

/// Auth-related object IDs the JS side must fetch to verify a
/// `MoveAuthenticator` signature. `null` for non-`MoveAuthenticator` blobs.
#[derive(Serialize, Deserialize)]
pub struct MoveAuthenticatorObjects {
    /// Hex-encoded IDs of the authenticator's own input objects.
    pub input_object_ids: Vec<String>,
    /// Hex-encoded ID of the account object being authenticated.
    pub account_object_id: String,
    /// Hex-encoded ID of the dynamic field holding the authenticator function
    /// reference.
    pub auth_function_field_id: String,
}

/// Inspect a base-64 signature blob and, when it is a `MoveAuthenticator`,
/// return the auth-related object IDs the JS side must fetch. Returns `null`
/// for any other signature scheme.
#[wasm_bindgen]
pub fn decode_move_authenticator_objects(sig_b64: &str) -> Result<JsValue, JsError> {
    let bytes = b64_decode(sig_b64)?;
    let sig = GenericSignature::from_bytes(&bytes)
        .map_err(|e| JsError::new(&format!("decode signature: {e}")))?;
    let GenericSignature::MoveAuthenticator(auth) = sig else {
        return Ok(JsValue::NULL);
    };

    let (account_id, _, _) = auth
        .object_to_authenticate_components()
        .map_err(|e| JsError::new(&format!("object_to_authenticate: {e}")))?;
    let input_object_ids: Vec<String> = auth
        .input_objects()
        .iter()
        .map(|kind| kind.object_id().to_hex())
        .collect();
    let field_id = auth_function_field_id(account_id).map_err(err_to_js)?;

    let out = MoveAuthenticatorObjects {
        input_object_ids,
        account_object_id: account_id.to_hex(),
        auth_function_field_id: field_id.to_hex(),
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsError::new(&e.to_string()))
}
