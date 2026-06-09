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
use iota_sdk_types::Owner;
use iota_types::{
    base_types::ObjectRef,
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
    wasm_store::CallbackStore,
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

/// The owner of an object, in a JS-friendly tagged form.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OwnerInfo {
    /// Exclusively owned by an address.
    Address {
        /// Owner address, hex `0x…`.
        address: String,
    },
    /// Owned by another object (a dynamic field or wrapped object).
    Object {
        /// Parent object ID, hex `0x…`.
        object_id: String,
    },
    /// Shared and usable by any address.
    Shared {
        /// Version at which the object became shared.
        initial_shared_version: u64,
    },
    /// Immutable; ownership doesn't apply.
    Immutable,
    /// An owner kind this build doesn't recognise.
    Unknown,
}

impl From<&Owner> for OwnerInfo {
    fn from(owner: &Owner) -> Self {
        match owner {
            Owner::Address(address) => OwnerInfo::Address {
                address: address.to_string(),
            },
            Owner::Object(object_id) => OwnerInfo::Object {
                object_id: object_id.to_string(),
            },
            Owner::Shared(version) => OwnerInfo::Shared {
                initial_shared_version: version.as_u64(),
            },
            Owner::Immutable => OwnerInfo::Immutable,
            _ => OwnerInfo::Unknown,
        }
    }
}

/// One object created or mutated by the transaction.
#[derive(Serialize, Deserialize)]
pub struct ChangedObject {
    /// Object ID, hex `0x…`.
    pub object_id: String,
    /// Version (sequence number) after the change.
    pub version: u64,
    /// Object digest, base58.
    pub digest: String,
    /// The object's owner after the change.
    pub owner: OwnerInfo,
}

/// One object deleted (or wrapped) by the transaction.
#[derive(Serialize, Deserialize)]
pub struct DeletedObject {
    /// Object ID, hex `0x…`.
    pub object_id: String,
    /// Version at which the object was deleted.
    pub version: u64,
    /// Object digest, base58.
    pub digest: String,
}

/// One Move event emitted by the transaction.
#[derive(Serialize, Deserialize)]
pub struct EventOut {
    /// Package that emitted the event, hex `0x…`.
    pub package_id: String,
    /// Module inside that package.
    pub module: String,
    /// Event struct name.
    pub name: String,
    /// Fully-qualified event type, e.g. `0x2::coin::CoinEvent`.
    pub type_tag: String,
    /// Decoded event payload, when decoding succeeded.
    pub value: Option<serde_json::Value>,
    /// Decode error, when the event couldn't be annotated against the store.
    pub decode_error: Option<String>,
}

/// One BCS value produced by a PTB command (a dev-inspect return value or a
/// mutably-borrowed argument's output), with its type and decoded payload.
#[derive(Serialize, Deserialize)]
pub struct MoveCallValue {
    /// The value's Move type, e.g. `u64` or `0x2::coin::Coin<0x2::iota::IOTA>`.
    pub type_tag: String,
    /// Base-64 of the value's BCS bytes (the raw value, always present).
    pub bcs: String,
    /// Decoded value, when the type layout could be resolved from the store.
    pub value: Option<serde_json::Value>,
    /// Decode error, when the value couldn't be decoded.
    pub decode_error: Option<String>,
}

/// Outputs of one PTB command, surfaced for dev-inspect runs. Empty for
/// commands that neither return a value nor mutate a borrowed argument.
#[derive(Serialize, Deserialize)]
pub struct CommandResultOut {
    /// Values the command returned.
    pub return_values: Vec<MoveCallValue>,
    /// Values of the arguments the command mutably borrowed.
    pub mutable_reference_outputs: Vec<MoveCallValue>,
}

/// Output of [`simulate`]: the run's status, a flattened gas summary, the
/// objects and events the transaction produced, and per-command dev-inspect
/// results.
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
    /// Objects mutated by the transaction.
    pub mutated: Vec<ChangedObject>,
    /// Objects created by the transaction.
    pub created: Vec<ChangedObject>,
    /// Objects deleted by the transaction.
    pub deleted: Vec<DeletedObject>,
    /// Events emitted by the transaction, decoded against the loaded objects.
    pub events: Vec<EventOut>,
    /// Per-PTB-command dev-inspect results (return values and mutable-reference
    /// outputs). Populated in dev-inspect; empty otherwise.
    pub command_results: Vec<CommandResultOut>,
    /// Debug rendering of the execution error, when the run failed.
    pub error: Option<String>,
    /// `true` when signatures were supplied and verification (incl. any
    /// `MoveAuthenticator` function) succeeded.
    pub signature_verified: bool,
}

/// Run a [`SimulateRequest`] through the local Move VM and return a
/// [`SimulateResult`].
///
/// Objects are resolved on demand: `fetch_object(id_hex: string) -> string |
/// null` is called for any object the VM needs that isn't already cached, and
/// must return the object's base-64 BCS (synchronously, since the VM is
/// synchronous). `req.objects` may pre-seed the cache but can be empty. The
/// transaction is run in dry-run or dev-inspect mode, verifying any signatures.
#[wasm_bindgen]
pub fn simulate(req: JsValue, fetch_object: js_sys::Function) -> Result<JsValue, JsError> {
    let req: SimulateRequest =
        serde_wasm_bindgen::from_value(req).map_err(|e| JsError::new(&e.to_string()))?;

    let tx_bytes = b64_decode(&req.tx_b64)?;
    let tx: TransactionData =
        bcs::from_bytes(&tx_bytes).map_err(|e| JsError::new(&format!("bcs decode tx: {e}")))?;

    let store = CallbackStore::new(fetch_object);
    let mut seed = Vec::with_capacity(req.objects.len());
    for (i, o) in req.objects.iter().enumerate() {
        let bytes =
            b64_decode(&o.bcs_b64).map_err(|_| JsError::new(&format!("object[{i}] base64")))?;
        let obj: Object =
            bcs::from_bytes(&bytes).map_err(|e| JsError::new(&format!("object[{i}] bcs: {e}")))?;
        seed.push(obj);
    }
    store.seed(seed);

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
    let signature_verified =
        signed && matches!(result.signature_status, crate::SignatureStatus::Verified);

    fn changed((obj, owner): (ObjectRef, Owner)) -> ChangedObject {
        ChangedObject {
            object_id: obj.object_id().to_string(),
            version: obj.version().as_u64(),
            digest: obj.digest().to_string(),
            owner: OwnerInfo::from(&owner),
        }
    }
    let mutated: Vec<ChangedObject> = result.effects.mutated().into_iter().map(changed).collect();
    let created: Vec<ChangedObject> = result.effects.created().into_iter().map(changed).collect();
    let deleted: Vec<DeletedObject> = result
        .effects
        .deleted()
        .into_iter()
        .map(|obj| DeletedObject {
            object_id: obj.object_id().to_string(),
            version: obj.version().as_u64(),
            digest: obj.digest().to_string(),
        })
        .collect();

    // Decode each event against the loaded objects; keep going on a per-event
    // failure so one undecodable event doesn't drop the rest.
    let events: Vec<EventOut> = match &result.events {
        Some(evs) => vm
            .decode_events(evs)
            .into_iter()
            .map(|dec| match dec {
                Ok(d) => EventOut {
                    package_id: d.package_id.to_string(),
                    module: d.module.to_string(),
                    name: d.name.to_string(),
                    type_tag: d.type_tag.to_string(),
                    value: serde_json::to_value(&d.value).ok(),
                    decode_error: None,
                },
                Err(e) => EventOut {
                    package_id: String::new(),
                    module: String::new(),
                    name: String::new(),
                    type_tag: String::new(),
                    value: None,
                    decode_error: Some(e.to_string()),
                },
            })
            .collect(),
        None => Vec::new(),
    };

    // Decode the per-command dev-inspect values (return values and mutable
    // reference outputs), each a raw `(bytes, type)` pair, against the store.
    let decode_call_value = |bytes: &[u8], type_tag: &iota_sdk_types::TypeTag| {
        let (value, decode_error) = match vm.decode_value(bytes, type_tag) {
            Ok(v) => (serde_json::to_value(&v).ok(), None),
            Err(e) => (None, Some(e.to_string())),
        };
        MoveCallValue {
            type_tag: type_tag.to_string(),
            bcs: base64::engine::general_purpose::STANDARD.encode(bytes),
            value,
            decode_error,
        }
    };
    let command_results: Vec<CommandResultOut> = result
        .command_results
        .iter()
        .map(|(mut_refs, returns)| CommandResultOut {
            return_values: returns
                .iter()
                .map(|(bytes, tt)| decode_call_value(bytes, tt))
                .collect(),
            mutable_reference_outputs: mut_refs
                .iter()
                .map(|(_arg, bytes, tt)| decode_call_value(bytes, tt))
                .collect(),
        })
        .collect();

    let out = SimulateResult {
        success,
        status: format!("{status:?}"),
        gas_used: gas.gas_used(),
        computation_cost: gas.computation_cost,
        storage_cost: gas.storage_cost,
        storage_rebate: gas.storage_rebate,
        non_refundable_storage_fee: gas.non_refundable_storage_fee,
        mutated,
        created,
        deleted,
        events,
        command_results,
        error: status.error().map(|e| format!("{e:?}")),
        signature_verified,
    };
    // Round-trip through a JSON string rather than `serde_wasm_bindgen`: the
    // decoded event payloads are `serde_json::Value`s, and `serde_json` renders
    // its own maps and (arbitrary-precision) numbers faithfully, whereas
    // `serde_wasm_bindgen` would turn maps into JS `Map`s (stringifying to
    // `{}`) and leak serde_json's number token. `JSON.parse` then yields a
    // plain JS object.
    let json = serde_json::to_string(&out).map_err(|e| JsError::new(&e.to_string()))?;
    js_sys::JSON::parse(&json).map_err(|e| JsError::new(&format!("{e:?}")))
}
