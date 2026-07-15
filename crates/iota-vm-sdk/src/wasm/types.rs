// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Serde request/result types for the JS
//! [`simulate`](super::simulate::simulate) surface.

use move_core_types::{annotated_value::MoveValue, identifier::Identifier};
use serde::{Deserialize, Serialize};

/// A BCS-encoded `Object`, base-64 encoded.
#[derive(Serialize, Deserialize)]
pub struct BcsObject {
    /// Base-64 of the object's BCS bytes.
    pub bcs_b64: String,
}

/// Input to [`simulate`](super::simulate::simulate): the transaction, the chain
/// parameters, the objects it touches, and optional signatures.
#[derive(Serialize, Deserialize)]
pub struct SimulateRequest {
    /// Base-64 BCS [`TransactionData`] to run.
    pub tx_b64: String,
    /// Chain the transaction targets: `"mainnet"`, `"testnet"`, or absent for
    /// an unknown chain. Selects the chain-specific protocol configuration.
    #[serde(default)]
    pub chain: Option<String>,
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
pub enum Owner {
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

impl From<&iota_sdk_types::Owner> for Owner {
    fn from(owner: &iota_sdk_types::Owner) -> Self {
        match owner {
            iota_sdk_types::Owner::Address(address) => Owner::Address {
                address: address.to_string(),
            },
            iota_sdk_types::Owner::Object(object_id) => Owner::Object {
                object_id: object_id.to_string(),
            },
            iota_sdk_types::Owner::Shared(version) => Owner::Shared {
                initial_shared_version: version.as_u64(),
            },
            iota_sdk_types::Owner::Immutable => Owner::Immutable,
            _ => Owner::Unknown,
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
    pub owner: Owner,
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

/// Output of [`simulate`](super::simulate::simulate): the run's status, a
/// flattened gas summary, the objects and events the transaction produced, and
/// per-command dev-inspect results.
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
    /// Why signature verification failed, when it did.
    pub signature_error: Option<String>,
}

/// Render a decoded Move value as JSON. `u64`/`u128`/`u256` become strings —
/// they exceed the 2^53 precision of a JS number (the result round-trips
/// through `JSON.parse`) — matching the JSON-RPC convention; addresses render
/// as `0x…` hex.
pub(super) fn move_value_to_json(value: &MoveValue) -> serde_json::Value {
    use serde_json::{Value, json};
    match value {
        MoveValue::Bool(v) => json!(v),
        MoveValue::U8(v) => json!(v),
        MoveValue::U16(v) => json!(v),
        MoveValue::U32(v) => json!(v),
        MoveValue::U64(v) => json!(v.to_string()),
        MoveValue::U128(v) => json!(v.to_string()),
        MoveValue::U256(v) => json!(v.to_string()),
        MoveValue::Address(a) | MoveValue::Signer(a) => json!(a.to_canonical_string(true)),
        MoveValue::Vector(items) => Value::Array(items.iter().map(move_value_to_json).collect()),
        MoveValue::Struct(s) => fields_to_json(&s.fields),
        MoveValue::Variant(v) => json!({
            "variant": v.variant_name.to_string(),
            "fields": fields_to_json(&v.fields),
        }),
    }
}

fn fields_to_json(fields: &[(Identifier, MoveValue)]) -> serde_json::Value {
    serde_json::Value::Object(
        fields
            .iter()
            .map(|(name, value)| (name.to_string(), move_value_to_json(value)))
            .collect(),
    )
}
