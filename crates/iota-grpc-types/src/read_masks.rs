// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Default read mask constants for each gRPC endpoint.
//!
//! These are the canonical defaults used by both the server (as fallback when
//! no mask is provided) and the client (when `None` is passed as the mask).
//!
//! Individual fields can be requested using dot notation. Pass a custom mask
//! to narrow or widen the response; these constants serve as the baseline
//! that covers the most commonly needed fields.

use crate::field_mask;

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

/// Default read mask for `get_checkpoint` / `stream_checkpoint_data`.
pub const GET_CHECKPOINT_READ_MASK: &str = field_mask!("checkpoint.summary");

/// Read mask for fetching full checkpoint data suitable for conversion to
/// `iota_sdk_types::CheckpointData` via
/// `CheckpointResponse::checkpoint_data()`.
///
/// Requests only the `.bcs` sub-fields needed for the conversion, avoiding
/// unnecessary data transfer.
pub const CHECKPOINT_DATA_READ_MASK: &str = field_mask!(
    "checkpoint.summary.bcs",
    "checkpoint.contents.bcs",
    "checkpoint.signature",
    "transactions.transaction.bcs",
    "transactions.signatures.bcs",
    "transactions.effects.bcs",
    "transactions.events.events.bcs",
    "transactions.input_objects.bcs",
    "transactions.output_objects.bcs",
);

/// Default read mask for `execute_transaction`.
///
/// `ExecuteTransactionResponse` is transparent, so these paths apply directly
/// to `ExecutedTransaction` fields.
pub const EXECUTE_TRANSACTION_READ_MASK: &str = field_mask!(
    "transaction.digest",
    "effects",
    "events",
    "input_objects",
    "output_objects",
);

/// Default read mask for `simulate_transaction`.
pub const SIMULATE_TRANSACTION_READ_MASK: &str = field_mask!(
    "executed_transaction.transaction",
    "executed_transaction.effects",
    "executed_transaction.events",
    "executed_transaction.input_objects",
    "executed_transaction.output_objects",
    "suggested_gas_price",
    "execution_result",
);
