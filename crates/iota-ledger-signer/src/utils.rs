// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use iota_sdk::{
    IotaClient,
    rpc_types::{IotaObjectDataOptions, IotaObjectResponse, IotaRawData},
    types::{
        base_types::ObjectType,
        object::{MoveStructExt, Object},
        transaction::{InputObjectKind, TransactionAPI},
    },
};
use iota_sdk_types::{MoveStruct, ObjectData, ObjectReference, Transaction};

use crate::LedgerSignerError;

/// Loads every owned or immutable object the transaction references, so the
/// device can verify them against the inputs the transaction commits to.
///
/// Fails if any of them cannot be loaded or does not match the reference in
/// the transaction, so the device never gets a partial or altered set.
pub(crate) async fn load_objects_with_client(
    client: &IotaClient,
    transaction: &Transaction,
) -> Result<Vec<Object>, LedgerSignerError> {
    let object_refs = object_refs_from_transaction(transaction)?;

    if object_refs.is_empty() {
        return Ok(vec![]);
    }

    let responses = client
        .read_api()
        .multi_get_object_with_options(
            object_refs.iter().map(|r| r.object_id).collect(),
            IotaObjectDataOptions::bcs_lossless(),
        )
        .await?;

    objects_from_responses(object_refs, responses)
}

fn object_refs_from_transaction(
    transaction: &Transaction,
) -> Result<Vec<ObjectReference>, LedgerSignerError> {
    let input_objects = transaction
        .input_objects()?
        .into_iter()
        .filter_map(|input| match input {
            InputObjectKind::ImmOrOwnedMoveObject(object_ref) => Some(object_ref),
            _ => None,
        });

    let object_refs: BTreeSet<ObjectReference> = transaction
        .gas_data()
        .objects
        .iter()
        .copied()
        .chain(input_objects)
        .collect();

    Ok(object_refs.into_iter().collect())
}

fn objects_from_responses(
    object_refs: Vec<ObjectReference>,
    responses: Vec<IotaObjectResponse>,
) -> Result<Vec<Object>, LedgerSignerError> {
    if responses.len() != object_refs.len() {
        return Err(LedgerSignerError::ObjectCount {
            expected: object_refs.len(),
            actual: responses.len(),
        });
    }

    object_refs
        .into_iter()
        .zip(responses)
        .map(|(object_ref, response)| {
            object_from_response(&object_ref, response).map_err(|reason| {
                LedgerSignerError::ObjectLoad {
                    object_id: object_ref.object_id,
                    reason,
                }
            })
        })
        .collect()
}

fn object_from_response(
    object_ref: &ObjectReference,
    response: IotaObjectResponse,
) -> Result<Object, String> {
    let data = response.into_object().map_err(|e| e.to_string())?;

    let ObjectType::Struct(move_object_type) = data.object_type.ok_or("missing type")? else {
        return Err("not a Move object".to_owned());
    };
    let IotaRawData::MoveObject(move_object) = data.bcs.ok_or("missing BCS")? else {
        return Err("not a Move object".to_owned());
    };

    let move_struct = MoveStruct::new_from_execution_with_limit(
        move_object_type.into(),
        data.version,
        move_object.bcs_bytes,
        250 * 1024, // The limit is not important here, it is copied from the protocol config
    )
    .map_err(|e| e.to_string())?;

    let object: Object = iota_sdk_types::Object::new(
        ObjectData::Struct(move_struct),
        data.owner.ok_or("missing owner")?,
        data.previous_transaction
            .ok_or("missing previous transaction")?,
        data.storage_rebate.ok_or("missing storage rebate")?,
    )
    .into();

    let loaded_ref = object.object_ref();
    if loaded_ref != *object_ref {
        return Err(format!(
            "loaded {loaded_ref:?} does not match the transaction's {object_ref:?}"
        ));
    }

    Ok(object)
}

#[cfg(test)]
mod tests {
    use iota_sdk::{
        rpc_types::{IotaObjectData, IotaObjectResponseError},
        types::gas_coin::GasCoin,
    };
    use iota_sdk_types::{Address, ObjectId};

    use super::*;

    fn gas_object(id: ObjectId, value: u64) -> Object {
        Object::with_id_owner_gas_for_testing(id, Address::ZERO, value)
    }

    fn response(object: &Object) -> IotaObjectResponse {
        IotaObjectResponse::new_with_data(
            IotaObjectData::new(
                object.object_ref(),
                object.clone(),
                GasCoin::layout(),
                &IotaObjectDataOptions::bcs_lossless(),
                None,
            )
            .unwrap(),
        )
    }

    #[test]
    fn returns_objects_matching_the_transaction() {
        let object = gas_object(ObjectId::random(), 100);

        let objects =
            objects_from_responses(vec![object.object_ref()], vec![response(&object)]).unwrap();

        assert_eq!(objects, vec![object]);
    }

    #[test]
    fn fails_on_error_response() {
        let object_id = ObjectId::random();
        let object = gas_object(object_id, 100);

        let err = objects_from_responses(
            vec![object.object_ref()],
            vec![IotaObjectResponse::new_with_error(
                IotaObjectResponseError::NotExists { object_id },
            )],
        )
        .unwrap_err();

        assert!(
            matches!(err, LedgerSignerError::ObjectLoad { object_id: id, .. } if id == object_id)
        );
    }

    #[test]
    fn fails_on_missing_bcs() {
        let object = gas_object(ObjectId::random(), 100);
        let mut response = response(&object);
        response.data.as_mut().unwrap().bcs = None;

        let err = objects_from_responses(vec![object.object_ref()], vec![response]).unwrap_err();

        assert!(matches!(err, LedgerSignerError::ObjectLoad { .. }));
    }

    #[test]
    fn fails_on_contents_not_matching_the_digest() {
        let object_id = ObjectId::random();
        let referenced = gas_object(object_id, 100);
        let returned = gas_object(object_id, 1_000_000);

        let err = objects_from_responses(vec![referenced.object_ref()], vec![response(&returned)])
            .unwrap_err();

        assert!(
            matches!(err, LedgerSignerError::ObjectLoad { object_id: id, .. } if id == object_id)
        );
    }

    #[test]
    fn fails_on_missing_response() {
        let first = gas_object(ObjectId::random(), 100);
        let second = gas_object(ObjectId::random(), 100);

        let err = objects_from_responses(
            vec![first.object_ref(), second.object_ref()],
            vec![response(&first)],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            LedgerSignerError::ObjectCount {
                expected: 2,
                actual: 1
            }
        ));
    }
}
