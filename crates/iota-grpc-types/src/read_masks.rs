// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Read mask constants for the gRPC API.
//!
//! This module provides two categories of constants:
//!
//! ## Endpoint defaults
//!
//! Constants like [`GET_CHECKPOINT_READ_MASK`] are the canonical defaults used
//! by both the server (as fallback when no mask is provided) and the client
//! (when `None` is passed as the mask).
//!
//! ## Per-method field constants
//!
//! Constants like [`CHECKPOINT_RESPONSE_SUMMARY`] or
//! [`EXECUTED_TRANSACTION_EFFECTS`] represent the read mask field(s) required
//! by a specific accessor method on a response type. Pass one or more of these
//! to the endpoint's `read_mask` parameter to ensure the accessor will succeed.
//!
//! ### Context-dependent paths
//!
//! [`ExecutedTransaction`](crate::v1::transaction::ExecutedTransaction) appears
//! in multiple endpoints with different field-path prefixes:
//!
//! | Endpoint | Prefix |
//! |---|---|
//! | `get_transactions` / `execute_transactions` | *(none — direct)* |
//! | Checkpoint queries | `transactions.` |
//! | `simulate_transactions` | `executed_transaction.` |
//!
//! The `EXECUTED_TRANSACTION_*` constants use the **direct** (unprefixed)
//! paths. For checkpoint or simulate contexts, prepend the appropriate prefix.
//! The `CHECKPOINT_RESPONSE_*` constants already include the `transactions.`
//! or `checkpoint.` prefix for convenience.
//!
//! Individual fields can be requested using dot notation. Pass a custom mask
//! to narrow or widen the response; the endpoint defaults serve as a baseline
//! that covers the most commonly needed fields.

use crate::field_mask;

// ---------------------------------------------------------------------------
// Endpoint defaults
// ---------------------------------------------------------------------------

/// Default read mask for `get_service_info`.
pub const GET_SERVICE_INFO_READ_MASK: &str = field_mask!(
    "chain_id",
    "epoch",
    "executed_checkpoint_height",
    "executed_checkpoint_timestamp",
    "lowest_available_checkpoint",
    "lowest_available_checkpoint_objects",
);

/// Default read mask for `get_epoch`.
pub const GET_EPOCH_READ_MASK: &str = field_mask!(
    "epoch",
    "first_checkpoint",
    "last_checkpoint",
    "start",
    "end",
    "reference_gas_price",
    "protocol_config.protocol_version",
);

/// Default read mask for `get_transactions`.
pub const GET_TRANSACTIONS_READ_MASK: &str =
    field_mask!("transaction", "signatures", "checkpoint", "timestamp",);

/// Default read mask for `get_objects`.
pub const GET_OBJECTS_READ_MASK: &str = field_mask!("reference", "bcs");

/// Default read mask for `get_checkpoint` / `stream_checkpoints`.
pub const GET_CHECKPOINT_READ_MASK: &str = field_mask!("checkpoint.summary");

/// Default read mask for `list_dynamic_fields`.
pub const LIST_DYNAMIC_FIELDS_READ_MASK: &str = field_mask!("parent", "field_id");

/// Default read mask for `list_owned_objects`.
pub const LIST_OWNED_OBJECTS_READ_MASK: &str = field_mask!("reference", "bcs");

/// Default read mask for `execute_transactions`.
///
/// These paths apply to each `ExecutedTransaction` within the
/// `transaction_results` of the response.
pub const EXECUTE_TRANSACTIONS_READ_MASK: &str = field_mask!(
    "transaction.digest",
    "effects",
    "events",
    "input_objects",
    "output_objects",
);

/// Default read mask for `simulate_transactions`.
pub const SIMULATE_TRANSACTIONS_READ_MASK: &str = field_mask!(
    "executed_transaction.transaction",
    "executed_transaction.effects",
    "executed_transaction.events",
    "executed_transaction.input_objects",
    "executed_transaction.output_objects",
    "suggested_gas_price",
    "execution_result",
);

// ---------------------------------------------------------------------------
// CheckpointResponse — per-method field constants
//
// These use the full paths expected by the checkpoint endpoints
// (get_checkpoint_*, stream_checkpoints).
// ---------------------------------------------------------------------------

/// Read mask for `CheckpointResponse::summary()`.
///
/// Includes the checkpoint summary (digest + BCS).
pub const CHECKPOINT_RESPONSE_SUMMARY: &str = field_mask!("checkpoint.summary");

/// Read mask for `CheckpointResponse::signature()`.
///
/// Includes the validator aggregated signature for the checkpoint.
pub const CHECKPOINT_RESPONSE_SIGNATURE: &str = field_mask!("checkpoint.signature");

/// Read mask for `CheckpointResponse::contents()`.
///
/// Includes the checkpoint contents (digest + BCS).
pub const CHECKPOINT_RESPONSE_CONTENTS: &str = field_mask!("checkpoint.contents");

/// Read mask for `CheckpointResponse::executed_transactions()`.
///
/// Includes all fields of every executed transaction in the checkpoint.
pub const CHECKPOINT_RESPONSE_EXECUTED_TRANSACTIONS: &str = field_mask!("transactions");

/// Read mask for `CheckpointResponse::events()`.
///
/// Includes all top-level event fields for the checkpoint.
pub const CHECKPOINT_RESPONSE_EVENTS: &str = field_mask!("events");

/// Read mask for `CheckpointResponse::signed_summary()`.
///
/// Contains the minimum fields required to build a
/// `SignedCheckpointSummary`: checkpoint summary BCS and validator
/// signature.
pub const CHECKPOINT_RESPONSE_SIGNED_SUMMARY: &str =
    field_mask!("checkpoint.summary.bcs", "checkpoint.signature",);

/// Read mask for `CheckpointResponse::checkpoint_data()`.
///
/// Contains the minimum set of fields required to build a full
/// `CheckpointData`: checkpoint summary/signature/contents BCS and
/// per-transaction BCS for the transaction, signatures, effects,
/// events, and input/output objects.
pub const CHECKPOINT_RESPONSE_CHECKPOINT_DATA: &str = field_mask!(
    "checkpoint.summary.bcs",
    "checkpoint.signature",
    "checkpoint.contents.bcs",
    "transactions.transaction.bcs",
    "transactions.signatures",
    "transactions.effects.bcs",
    "transactions.events.events.bcs",
    "transactions.input_objects.bcs",
    "transactions.output_objects.bcs",
);

// ---------------------------------------------------------------------------
// CheckpointSummary / CheckpointContents — sub-field constants
//
// Full paths from the checkpoint endpoint root.
// ---------------------------------------------------------------------------

/// Read mask for
/// [`CheckpointSummary::digest()`](crate::v1::checkpoint::CheckpointSummary::digest).
pub const CHECKPOINT_SUMMARY_DIGEST: &str = "checkpoint.summary.digest";

/// Read mask for
/// [`CheckpointSummary::summary()`](crate::v1::checkpoint::CheckpointSummary::summary).
pub const CHECKPOINT_SUMMARY_BCS: &str = "checkpoint.summary.bcs";

/// Read mask for
/// [`CheckpointContents::digest()`](crate::v1::checkpoint::CheckpointContents::digest).
pub const CHECKPOINT_CONTENTS_DIGEST: &str = "checkpoint.contents.digest";

/// Read mask for
/// [`CheckpointContents::contents()`](crate::v1::checkpoint::CheckpointContents::contents).
pub const CHECKPOINT_CONTENTS_BCS: &str = "checkpoint.contents.bcs";

// ---------------------------------------------------------------------------
// ExecutedTransaction — per-method field constants
//
// Direct (unprefixed) paths, usable with get_transactions and
// execute_transactions. For checkpoint context prefix with "transactions.",
// for simulate_transactions prefix with "executed_transaction.".
// ---------------------------------------------------------------------------

/// Read mask for
/// [`ExecutedTransaction::transaction()`](crate::v1::transaction::ExecutedTransaction::transaction).
///
/// Includes the transaction digest and BCS.
pub const EXECUTED_TRANSACTION_TRANSACTION: &str = "transaction";

/// Read mask for
/// [`ExecutedTransaction::signatures()`](crate::v1::transaction::ExecutedTransaction::signatures).
pub const EXECUTED_TRANSACTION_SIGNATURES: &str = "signatures";

/// Read mask for
/// [`ExecutedTransaction::effects()`](crate::v1::transaction::ExecutedTransaction::effects).
///
/// Includes the effects digest and BCS.
pub const EXECUTED_TRANSACTION_EFFECTS: &str = "effects";

/// Read mask for
/// [`ExecutedTransaction::events()`](crate::v1::transaction::ExecutedTransaction::events).
///
/// Includes the events digest and all individual event fields.
pub const EXECUTED_TRANSACTION_EVENTS: &str = "events";

/// Read mask for
/// [`ExecutedTransaction::checkpoint_sequence_number()`](crate::v1::transaction::ExecutedTransaction::checkpoint_sequence_number).
pub const EXECUTED_TRANSACTION_CHECKPOINT: &str = "checkpoint";

/// Read mask for
/// [`ExecutedTransaction::timestamp_ms()`](crate::v1::transaction::ExecutedTransaction::timestamp_ms).
pub const EXECUTED_TRANSACTION_TIMESTAMP: &str = "timestamp";

/// Read mask for
/// [`ExecutedTransaction::input_objects()`](crate::v1::transaction::ExecutedTransaction::input_objects).
///
/// Includes object references and BCS for all input objects.
pub const EXECUTED_TRANSACTION_INPUT_OBJECTS: &str = "input_objects";

/// Read mask for
/// [`ExecutedTransaction::output_objects()`](crate::v1::transaction::ExecutedTransaction::output_objects).
///
/// Includes object references and BCS for all output objects.
pub const EXECUTED_TRANSACTION_OUTPUT_OBJECTS: &str = "output_objects";

// ---------------------------------------------------------------------------
// Transaction — sub-field constants (relative to ExecutedTransaction)
// ---------------------------------------------------------------------------

/// Read mask for
/// [`Transaction::digest()`](crate::v1::transaction::Transaction::digest).
pub const TRANSACTION_DIGEST: &str = "transaction.digest";

/// Read mask for
/// [`Transaction::transaction()`](crate::v1::transaction::Transaction::transaction)
/// (BCS deserialization).
pub const TRANSACTION_BCS: &str = "transaction.bcs";

// ---------------------------------------------------------------------------
// TransactionEffects — sub-field constants (relative to ExecutedTransaction)
// ---------------------------------------------------------------------------

/// Read mask for
/// [`TransactionEffects::digest()`](crate::v1::transaction::TransactionEffects::digest).
pub const TRANSACTION_EFFECTS_DIGEST: &str = "effects.digest";

/// Read mask for
/// [`TransactionEffects::effects()`](crate::v1::transaction::TransactionEffects::effects)
/// (BCS deserialization).
pub const TRANSACTION_EFFECTS_BCS: &str = "effects.bcs";

// ---------------------------------------------------------------------------
// TransactionEvents — sub-field constants (relative to ExecutedTransaction)
// ---------------------------------------------------------------------------

/// Read mask for
/// [`TransactionEvents::digest()`](crate::v1::transaction::TransactionEvents::digest).
pub const TRANSACTION_EVENTS_DIGEST: &str = "events.digest";

/// Read mask for
/// [`TransactionEvents::events()`](crate::v1::transaction::TransactionEvents::events)
/// (BCS deserialization of all events).
pub const TRANSACTION_EVENTS_BCS: &str = "events.events.bcs";

// ---------------------------------------------------------------------------
// Event — per-method field constants
//
// Relative paths. The full path depends on context:
// - Checkpoint top-level events: prefix with "events."
// - Transaction events (get_transactions): prefix with "events.events."
// - Checkpoint transaction events: prefix with "transactions.events.events."
// ---------------------------------------------------------------------------

/// Read mask for
/// [`Event::event()`](crate::v1::event::Event::event)
/// (full BCS deserialization).
pub const EVENT_BCS: &str = "bcs";

/// Read mask for
/// [`Event::package_id()`](crate::v1::event::Event::package_id).
pub const EVENT_PACKAGE_ID: &str = "package_id";

/// Read mask for
/// [`Event::module_name()`](crate::v1::event::Event::module_name).
pub const EVENT_MODULE: &str = "module";

/// Read mask for
/// [`Event::sender()`](crate::v1::event::Event::sender).
pub const EVENT_SENDER: &str = "sender";

/// Read mask for
/// [`Event::type_name()`](crate::v1::event::Event::type_name).
pub const EVENT_TYPE: &str = "event_type";

/// Read mask for
/// [`Event::bcs_contents()`](crate::v1::event::Event::bcs_contents).
pub const EVENT_BCS_CONTENTS: &str = "bcs_contents";

/// Read mask for
/// [`Event::json_contents()`](crate::v1::event::Event::json_contents).
pub const EVENT_JSON_CONTENTS: &str = "json_contents";

// ---------------------------------------------------------------------------
// Object — per-method field constants (for get_objects)
// ---------------------------------------------------------------------------

/// Read mask for
/// [`Object::object_reference()`](crate::v1::object::Object::object_reference).
///
/// Includes object_id, version, and digest.
pub const OBJECT_REFERENCE: &str = "reference";

/// Read mask for
/// [`Object::object()`](crate::v1::object::Object::object)
/// (BCS deserialization).
pub const OBJECT_BCS: &str = "bcs";

// ---------------------------------------------------------------------------
// SimulatedTransaction — per-method field constants
// ---------------------------------------------------------------------------

/// Read mask for
/// [`SimulatedTransaction::executed_transaction()`](crate::v1::transaction_execution_service::SimulatedTransaction::executed_transaction).
///
/// Includes all ExecutedTransaction sub-fields. To request specific
/// sub-fields, use paths like `"executed_transaction.effects"`.
pub const SIMULATED_TRANSACTION_EXECUTED_TRANSACTION: &str = "executed_transaction";

/// Read mask for
/// [`SimulatedTransaction::gas_price_suggested()`](crate::v1::transaction_execution_service::SimulatedTransaction::gas_price_suggested).
pub const SIMULATED_TRANSACTION_SUGGESTED_GAS_PRICE: &str = "suggested_gas_price";

/// Read mask for
/// [`SimulatedTransaction::execution_result()`](crate::v1::transaction_execution_service::SimulatedTransaction::execution_result),
/// [`SimulatedTransaction::command_results()`](crate::v1::transaction_execution_service::SimulatedTransaction::command_results),
/// and
/// [`SimulatedTransaction::execution_error()`](crate::v1::transaction_execution_service::SimulatedTransaction::execution_error).
pub const SIMULATED_TRANSACTION_EXECUTION_RESULT: &str = "execution_result";

// ---------------------------------------------------------------------------
// ExecutionError — sub-field constants (relative to simulate_transactions)
// ---------------------------------------------------------------------------

/// Read mask for
/// [`ExecutionError::error_kind()`](crate::v1::transaction_execution_service::ExecutionError::error_kind).
pub const EXECUTION_ERROR_BCS_KIND: &str = "execution_result.execution_error.bcs_kind";

/// Read mask for
/// [`ExecutionError::error_source()`](crate::v1::transaction_execution_service::ExecutionError::error_source).
pub const EXECUTION_ERROR_SOURCE: &str = "execution_result.execution_error.source";

/// Read mask for
/// [`ExecutionError::error_command_index()`](crate::v1::transaction_execution_service::ExecutionError::error_command_index).
pub const EXECUTION_ERROR_COMMAND_INDEX: &str = "execution_result.execution_error.command_index";
