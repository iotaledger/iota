// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::transaction::{
    ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents,
};

use crate::impl_field_presence_checker;

// Generate FieldPresenceChecker implementations for ExecutedTransaction and its
// nested types.
// Leaf types (containing only primitive fields) - treated as simple fields, and
// we don't implement FieldPresenceChecker for them. Field presence is checked
// at their parent level.
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
impl_field_presence_checker!(Transaction { digest, bcs });
impl_field_presence_checker!(TransactionEffects { digest, bcs });
impl_field_presence_checker!(TransactionEvents { digest, events });

mod execute;
mod simulate;
