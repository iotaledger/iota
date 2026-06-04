// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `wasm-bindgen` surface exported to JavaScript (`feature = "wasm-bindgen"`,
//! `target_arch = "wasm32"`).
//!
//! The browser flow:
//!   1. JS hands us the base-64 BCS-encoded [`TransactionData`] (and, for
//!      signed simulation, the raw signature blobs the wallet holds).
//!   2. [`decode_transaction`] returns the object IDs the transaction touches.
//!      For a [`MoveAuthenticator`] signature, JS additionally calls
//!      [`decode_move_authenticator_objects`] to learn which auth-related
//!      objects to fetch.
//!   3. JS fetches those objects via the node's GraphQL/JSON-RPC, base-64
//!      encoding the BCS objects.
//!   4. JS calls [`simulate`] with the chain info + objects + (optionally)
//!      signatures; the wasm side runs them through the local Move VM.
//!
//! Every [`VmSdkError`] is mapped to a thrown JS exception via [`JsError`].

use base64::Engine;
use fastcrypto::traits::ToFromBytes;
use iota_protocol_config::{Chain, ProtocolVersion};
use iota_types::{
    effects::TransactionEffectsAPI,
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData},
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::{
    ChainContext, ExecuteOptions, LocalVm,
    decode::{auth_function_field_id, decode_transaction as decode_transaction_inner},
    error::VmSdkError,
    store::InMemoryStore,
};

/// Module entry point: install a panic hook that surfaces Rust panics in the
/// browser console. Runs automatically when the wasm module is instantiated.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Decode a standard-base-64 string into raw bytes, mapping errors to a
/// JS exception.
fn b64_decode(s: &str) -> Result<Vec<u8>, JsError> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| JsError::new(&format!("base64 decode: {e}")))
}

/// Map a [`VmSdkError`] to a JS exception. The variant name is prefixed so the
/// JS side can branch on the failure phase without parsing the message body.
fn err_to_js(e: VmSdkError) -> JsError {
    // `VmSdkError` is `#[non_exhaustive]`; the wildcard keeps this mapping
    // compiling (and degrading to "Unknown") if a variant is added later.
    #[allow(unreachable_patterns)]
    let tag = match &e {
        VmSdkError::Decode(_) => "Decode",
        VmSdkError::Validation(_) => "Validation",
        VmSdkError::SignatureVerification(_) => "SignatureVerification",
        VmSdkError::MissingObject { .. } => "MissingObject",
        VmSdkError::Execution(_) => "Execution",
        VmSdkError::Vm(_) => "Vm",
        _ => "Unknown",
    };
    JsError::new(&format!("{tag}: {e}"))
}

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
/// and the set of object IDs the JS side must fetch before [`simulate`].
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

/// A BCS-encoded `Object`, base-64 encoded.
#[derive(Serialize, Deserialize)]
pub struct BcsObject {
    /// Base-64 of the object's BCS bytes.
    pub bcs_b64: String,
}

/// Input to [`simulate`]: the transaction, the chain parameters, the objects it
/// touches, and optional signatures.
#[derive(Serialize, Deserialize)]
pub struct SimulateRequest {
    /// Base-64 BCS [`TransactionData`] to run.
    pub tx_b64: String,
    /// Protocol version to configure the VM for.
    pub protocol_version: u64,
    /// Reference gas price for the epoch.
    pub reference_gas_price: u64,
    /// Current epoch ID.
    pub epoch_id: u64,
    /// Current epoch start timestamp, in milliseconds.
    pub epoch_timestamp_ms: u64,
    /// The objects the transaction reads/writes, pre-fetched by the JS side.
    pub objects: Vec<BcsObject>,
    /// When true, run with full sign-time checks (dry-run); otherwise
    /// dev-inspect.
    pub strict: bool,
    /// Optional raw signature blobs (base-64). When present, signatures are
    /// verified before execution.
    #[serde(default)]
    pub signatures: Vec<String>,
}

/// Output of [`simulate`]: the run's status and a flattened gas/effects
/// summary.
#[derive(Serialize, Deserialize)]
pub struct SimulateResult {
    /// Whether the transaction executed successfully.
    pub success: bool,
    /// Debug rendering of the execution status.
    pub status: String,
    /// Total gas consumed (net of rebate).
    pub gas_used: u64,
    /// Computation portion of the gas cost.
    pub computation_cost: u64,
    /// Storage portion of the gas cost.
    pub storage_cost: u64,
    /// Storage rebate credited back.
    pub storage_rebate: u64,
    /// Non-refundable storage fee.
    pub non_refundable_storage_fee: u64,
    /// Number of objects mutated by the transaction.
    pub mutated_count: usize,
    /// Number of objects created by the transaction.
    pub created_count: usize,
    /// Number of objects deleted by the transaction.
    pub deleted_count: usize,
    /// Number of events emitted.
    pub event_count: usize,
    /// Debug rendering of the execution error, when the run failed.
    pub error: Option<String>,
    /// `true` when signatures were supplied and verification (incl. any
    /// `MoveAuthenticator` function) succeeded.
    pub signature_verified: bool,
}

/// Run a [`SimulateRequest`] through the local Move VM and return a
/// [`SimulateResult`]. Loads the supplied objects into an in-memory store,
/// verifies any signatures, and executes in dry-run or dev-inspect mode.
#[wasm_bindgen]
pub fn simulate(req: JsValue) -> Result<JsValue, JsError> {
    let req: SimulateRequest =
        serde_wasm_bindgen::from_value(req).map_err(|e| JsError::new(&e.to_string()))?;

    let tx_bytes = b64_decode(&req.tx_b64)?;
    let tx: TransactionData =
        bcs::from_bytes(&tx_bytes).map_err(|e| JsError::new(&format!("bcs decode tx: {e}")))?;

    let mut store = InMemoryStore::with_framework();
    for (i, o) in req.objects.iter().enumerate() {
        let bytes =
            b64_decode(&o.bcs_b64).map_err(|_| JsError::new(&format!("object[{i}] base64")))?;
        let obj: Object =
            bcs::from_bytes(&bytes).map_err(|e| JsError::new(&format!("object[{i}] bcs: {e}")))?;
        crate::store::Store::insert(&mut store, obj);
    }

    let ctx = ChainContext {
        protocol_version: ProtocolVersion::new(req.protocol_version),
        reference_gas_price: req.reference_gas_price,
        epoch_id: req.epoch_id,
        epoch_timestamp_ms: req.epoch_timestamp_ms,
        chain: Chain::Unknown,
    };
    let mut vm = LocalVm::new(ctx, store).map_err(err_to_js)?;

    let opts = if req.strict {
        ExecuteOptions::dry_run()
    } else {
        ExecuteOptions::dev_inspect()
    };

    let signed = !req.signatures.is_empty();
    let result = if signed {
        let mut sigs: Vec<GenericSignature> = Vec::with_capacity(req.signatures.len());
        for (i, s) in req.signatures.iter().enumerate() {
            let bytes =
                b64_decode(s).map_err(|_| JsError::new(&format!("signature[{i}] base64")))?;
            let sig = GenericSignature::from_bytes(&bytes)
                .map_err(|e| JsError::new(&format!("signature[{i}] decode: {e}")))?;
            sigs.push(sig);
        }
        let signed_data = SenderSignedData::new(tx, sigs);
        vm.execute_signed(signed_data, opts).map_err(err_to_js)?
    } else {
        vm.execute(tx, opts).map_err(err_to_js)?
    };

    let status = result.effects.status();
    let success = status.is_success();
    let gas = &result.gas_summary;
    let event_count = result.events.as_ref().map(|e| e.0.len()).unwrap_or(0);
    let signature_verified =
        signed && matches!(result.signature_status, crate::SignatureStatus::Verified);

    let out = SimulateResult {
        success,
        status: format!("{status:?}"),
        gas_used: gas.gas_used(),
        computation_cost: gas.computation_cost,
        storage_cost: gas.storage_cost,
        storage_rebate: gas.storage_rebate,
        non_refundable_storage_fee: gas.non_refundable_storage_fee,
        mutated_count: result.effects.mutated().len(),
        created_count: result.effects.created().len(),
        deleted_count: result.effects.deleted().len(),
        event_count,
        error: status.error().map(|e| format!("{e:?}")),
        signature_verified,
    };
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsError::new(&e.to_string()))
}
