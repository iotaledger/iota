// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_sdk_types::{ObjectId, Transaction};
use iota_types::transaction::{InputObjectKind, TransactionAPI};

use crate::LedgerSignerError;

pub(crate) fn object_ids_from_transaction(
    transaction: &Transaction,
) -> Result<Vec<ObjectId>, LedgerSignerError> {
    let object_ids = transaction
        .gas_data()
        .objects
        .iter()
        .map(|object| object.object_id);

    let input_objects = transaction
        .input_objects()?
        .into_iter()
        .filter_map(|input| match input {
            InputObjectKind::ImmOrOwnedMoveObject(id) => Some(id.object_id),
            _ => None,
        });

    let mut unique_ids = HashSet::new();
    unique_ids.extend(object_ids);
    unique_ids.extend(input_objects);

    Ok(unique_ids.into_iter().collect())
}
