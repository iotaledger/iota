// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The [`simulate`] entry point: run a transaction through the local Move VM
//! and surface a JS-friendly result.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iota_protocol_config::{Chain, ProtocolVersion};
use iota_sdk_types::{ObjectReference, Owner as SdkOwner};
use iota_types::{
    effects::TransactionEffectsAPI,
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData},
};
use wasm_bindgen::prelude::*;

use super::{
    b64_decode, err_to_js,
    store::CallbackStore,
    types::{
        ChangedObject, CommandResultOut, DeletedObject, EventOut, MoveCallValue, Owner,
        SimulateRequest, SimulateResult, move_value_to_json,
    },
};
use crate::{ChainContext, ExecuteOptions, LocalVm, SignatureStatus};

/// Run a [`SimulateRequest`] through the local Move VM and return a
/// [`SimulateResult`].
///
/// Objects are resolved on demand: `fetch_object(id_hex: string, version:
/// number | null) -> string | null` is called for any object the VM needs that
/// isn't already cached, and must return the object's base-64 BCS
/// (synchronously, since the VM is synchronous) at the given version — latest
/// when `version` is null — or null when the object doesn't exist.
/// `req.objects` may pre-seed the cache but can be empty. The transaction is
/// run in dry-run or dev-inspect mode, verifying any signatures.
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

    let chain = match req.chain.as_deref() {
        None => Chain::Unknown,
        Some("mainnet") => Chain::Mainnet,
        Some("testnet") => Chain::Testnet,
        Some(other) => {
            return Err(JsError::new(&format!(
                "unknown chain \"{other}\": expected \"mainnet\" or \"testnet\", or omit it"
            )));
        }
    };
    let ctx = ChainContext {
        protocol_version: ProtocolVersion::new(req.protocol_version),
        reference_gas_price: req.reference_gas_price,
        epoch_id: req.epoch_id,
        epoch_timestamp_ms: req.epoch_timestamp_ms,
        chain,
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
    let signature_verified = signed && matches!(result.signature_status, SignatureStatus::Verified);
    let signature_error = match &result.signature_status {
        SignatureStatus::Failed(e) => Some(e.to_string()),
        _ => None,
    };

    fn changed((obj, owner): (ObjectReference, SdkOwner)) -> ChangedObject {
        ChangedObject {
            object_id: obj.object_id().to_string(),
            version: obj.version().as_u64(),
            digest: obj.digest().to_string(),
            owner: Owner::from(&owner),
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
        Some(evs) => evs
            .0
            .iter()
            .map(|ev| {
                let (value, decode_error) =
                    match vm.decode_value(&ev.contents, &ev.type_.clone().into()) {
                        Ok(value) => (Some(move_value_to_json(&value)), None),
                        Err(e) => (None, Some(e.to_string())),
                    };
                EventOut {
                    package_id: ev.package_id.to_string(),
                    module: ev.module.to_string(),
                    name: ev.type_.name().to_string(),
                    type_tag: ev.type_.to_string(),
                    value,
                    decode_error,
                }
            })
            .collect(),
        None => Vec::new(),
    };

    // Decode the per-command dev-inspect values (return values and mutable
    // reference outputs), each a raw `(bytes, type)` pair, against the store.
    let decode_call_value = |bytes: &[u8], type_tag: &iota_sdk_types::TypeTag| {
        let (value, decode_error) = match vm.decode_value(bytes, type_tag) {
            Ok(v) => (Some(move_value_to_json(&v)), None),
            Err(e) => (None, Some(e.to_string())),
        };
        MoveCallValue {
            type_tag: type_tag.to_string(),
            bcs: BASE64.encode(bytes),
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
        signature_error,
    };
    // Round-trip through a JSON string rather than `serde_wasm_bindgen`: the
    // decoded payloads are `serde_json::Value`s, and `serde_wasm_bindgen`
    // would turn their maps into JS `Map`s (stringifying to `{}`). `JSON.parse`
    // yields a plain JS object instead; it is lossless here because
    // `move_value_to_json` renders every 64-bit-plus integer as a string.
    let json = serde_json::to_string(&out).map_err(|e| JsError::new(&e.to_string()))?;
    js_sys::JSON::parse(&json).map_err(|e| JsError::new(&format!("{e:?}")))
}
