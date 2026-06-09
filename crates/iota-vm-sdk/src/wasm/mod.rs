// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `wasm-bindgen` surface exported to JavaScript (`feature = "wasm-bindgen"`,
//! `target_arch = "wasm32"`).
//!
//! The browser flow:
//!
//! 1. JS builds the transaction with the TS SDK and serializes it to base-64
//!    BCS (and, for signed simulation, holds the raw signature blobs).
//! 2. JS calls [`simulate`](simulate::simulate) with the chain info and
//!    (optionally) signatures, handing it a `fetch_object` callback.
//! 3. The wasm side runs the transaction through the local Move VM, calling
//!    back into `fetch_object` for any object it needs — resolved on demand
//!    (e.g. from the node's GraphQL/JSON-RPC) as base-64 BCS.
//!
//! Every [`VmSdkError`] is mapped to a thrown JS exception via [`JsError`].
//!
//! The surface is split across submodules:
//! - [`types`] — the serde request/result types for
//!   [`simulate`](simulate::simulate).
//! - [`simulate`] — the [`simulate`](simulate::simulate) entry point.

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
