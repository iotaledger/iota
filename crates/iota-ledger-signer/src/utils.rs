// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_sdk::{
    IotaClient,
    rpc_types::{IotaObjectData, IotaObjectDataOptions, IotaObjectResponse},
    types::{
        base_types::{ObjectID, ObjectType},
        object::{MoveObject, Object},
        transaction::{InputObjectKind, TransactionData, TransactionDataAPI},
    },
};

use crate::LedgerSignerError;

pub(crate) async fn load_objects_with_client(
    client: &IotaClient,
    transaction: &TransactionData,
) -> Result<Vec<Object>, LedgerSignerError> {
    let object_ids = object_ids_from_transaction(transaction)?;

    if object_ids.is_empty() {
        return Ok(vec![]);
    }

    let responses = client
        .read_api()
        .multi_get_object_with_options(object_ids, IotaObjectDataOptions::bcs_lossless())
        .await?;

    let objects: Vec<Object> = responses
        .into_iter()
        .filter_map(object_from_response)
        .collect();

    Ok(objects)
}

fn object_ids_from_transaction(
    transaction: &TransactionData,
) -> Result<Vec<ObjectID>, LedgerSignerError> {
    let object_ids = transaction
        .gas_data()
        .payment
        .iter()
        .map(|payment| payment.0);

    let input_objects = transaction
        .input_objects()?
        .into_iter()
        .filter_map(|input| match input {
            InputObjectKind::ImmOrOwnedMoveObject(id) => Some(id.0),
            _ => None,
        });

    let mut unique_ids = HashSet::new();
    unique_ids.extend(object_ids);
    unique_ids.extend(input_objects);

    Ok(unique_ids.into_iter().collect())
}

// Convert IotaObjectResponse to supported clear-sign Objects returning None if
// the conversion fails
fn object_from_response(resp: IotaObjectResponse) -> Option<Object> {
    let data: IotaObjectData = resp.data?;

    let move_object_type = match data.type_? {
        ObjectType::Struct(t) => t,
        _ => return None,
    };

    let bcs_bytes = match data.bcs? {
        iota_sdk::rpc_types::IotaRawData::MoveObject(move_obj) => move_obj.bcs_bytes,
        _ => return None,
    };

    let move_object = MoveObject::new_from_execution_with_limit(
        move_object_type,
        data.version,
        bcs_bytes,
        250 * 1024, // The limit is not important here, it is copied from the protocol config
    )
    .ok()?;
    let owner = data.owner?;
    let previous_transaction = data.previous_transaction?;

    let object = Object::new_move(move_object, owner, previous_transaction);

    // We need to get the inner object to modify the storage rebate
    let mut inner = object.into_inner();
    inner.storage_rebate = data.storage_rebate.unwrap_or(0);

    Some(inner.into())
}
