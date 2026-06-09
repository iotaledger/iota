// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `wasm-bindgen` surface exported to JavaScript (`feature = "wasm-bindgen"`,
//! `target_arch = "wasm32"`).
//!
//! The browser flow:
//!
//! 1. JS hands us the base-64 BCS-encoded [`TransactionData`] (and, for signed
//!    simulation, the raw signature blobs the wallet holds).
//! 2. [`decode_transaction`](decode::decode_transaction) returns the object IDs
//!    the transaction touches. For a [`MoveAuthenticator`] signature, JS
//!    additionally calls
//!    [`decode_move_authenticator_objects`](decode::decode_move_authenticator_objects)
//!    to learn which auth-related objects to fetch.
//! 3. JS fetches those objects via the node's GraphQL/JSON-RPC, base-64
//!    encoding the BCS objects.
//! 4. JS calls [`simulate`](simulate::simulate) with the chain info + objects +
//!    (optionally) signatures; the wasm side runs them through the local Move
//!    VM.
//!
//! Every [`VmSdkError`] is mapped to a thrown JS exception via [`JsError`].
//!
//! The surface is split across submodules:
//! - [`decode`] — the static, VM-free decode helpers.
//! - [`types`] — the serde request/result types for
//!   [`simulate`](simulate::simulate).
//! - [`simulate`] — the [`simulate`](simulate::simulate) entry point.

mod decode;
mod simulate;
mod types;

use base64::Engine;
use wasm_bindgen::prelude::*;

use crate::error::VmSdkError;

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
