// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::{
    signatures::UserSignature,
    transaction::{ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents},
};

use crate::impl_field_presence_checker;

// Generate FieldPresenceChecker implementations for nested types
// These are defined from leaf to root to satisfy dependency order

// Leaf types (containing only primitive fields) - treated as simple fields
// Note: Digest, BcsData have non-Option fields (bytes) so we don't implement
// FieldPresenceChecker for them. Field presence is checked at their parent
// level.

// UserSignature has one optional field
impl_field_presence_checker!(UserSignature { bcs });

// UserSignatures is a collection type - treated as simple field at parent level

// Transaction has optional nested fields
impl_field_presence_checker!(Transaction { digest, bcs });

// TransactionEffects has optional nested fields
impl_field_presence_checker!(TransactionEffects { digest, bcs });

// TransactionEvents has optional nested fields
// Note: Events is a collection (Vec<Event>) so we treat it as a simple field
impl_field_presence_checker!(TransactionEvents { digest, events });

// Objects and Events are collection types - treated as simple fields at parent
// level

// ExecutedTransaction with full nested type annotations
impl_field_presence_checker!(ExecutedTransaction {
    digest,
    transaction: Transaction,
    signatures,
    effects: TransactionEffects,
    events: TransactionEvents,
    checkpoint,
    timestamp,
    input_objects,
    output_objects,
});

mod execute;
mod simulate;
