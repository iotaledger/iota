// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Collections of common functions and methods used by the JSON-RPC APIs.

use iota_json_rpc::error::IotaRpcInputError;
use iota_json_rpc_api::QUERY_MAX_RESULT_LIMIT;
use iota_json_rpc_types::OwnedObjectCursor;
use iota_sdk_types::ObjectId;
use jsonrpsee::core::RpcResult;

/// Rejects a batch request whose input has more entries than
/// [`QUERY_MAX_RESULT_LIMIT`].
pub(crate) fn validate_input_limit(len: usize) -> RpcResult<()> {
    if len > *QUERY_MAX_RESULT_LIMIT {
        return Err(
            IotaRpcInputError::SizeLimitExceeded(QUERY_MAX_RESULT_LIMIT.to_string()).into(),
        );
    }
    Ok(())
}

/// The object a coin or owned-object cursor names. This store pages by object
/// id alone, so it refuses a cursor that holds a full node's position, whose
/// pages are ordered by type and balance: continuing such a page by object id
/// would skip or repeat objects.
pub(crate) fn object_id_cursor(cursor: Option<OwnedObjectCursor>) -> RpcResult<Option<ObjectId>> {
    match cursor {
        None => Ok(None),
        Some(cursor) if cursor.position().is_none() => Ok(Some(cursor.object_id())),
        Some(_) => Err(IotaRpcInputError::GenericInvalid(
            "the cursor comes from a full node, which orders its pages differently; pass a \
             cursor this indexer returned"
                .into(),
        )
        .into()),
    }
}

/// Checks if the provided cursor does not exceed the Postgres `Int8` column represented as a `i64`.
pub(crate) fn validate_cursor(cursor: Option<u64>) -> RpcResult<()> {
    if cursor.is_some_and(|c| i64::try_from(c).is_err()) {
        return Err(IotaRpcInputError::GenericInvalid("cursor out of range".into()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_json_rpc_types::OwnedObjectCursor;
    use iota_sdk_types::ObjectId;
    use iota_types::storage::OwnedObjectCursor as IndexCursor;

    use super::{object_id_cursor, validate_cursor};

    #[test]
    fn an_object_id_cursor_is_followed_and_a_full_node_position_refused() {
        let id = ObjectId::random();
        assert_eq!(object_id_cursor(None).unwrap(), None);
        assert_eq!(
            object_id_cursor(Some(OwnedObjectCursor::from_object_id(id))).unwrap(),
            Some(id)
        );
        let position = OwnedObjectCursor::from_position(IndexCursor {
            object_type_identifier: 1,
            object_type_params: 2,
            inverted_balance: Some(3),
            object_id: id,
        });
        assert!(object_id_cursor(Some(position)).is_err());
    }

    #[test]
    fn cursor_within_int8_range_is_accepted() {
        assert!(validate_cursor(None).is_ok());
        assert!(validate_cursor(Some(0)).is_ok());
        assert!(validate_cursor(Some(i64::MAX as u64)).is_ok());
    }

    #[test]
    fn cursor_above_int8_range_is_rejected() {
        assert!(validate_cursor(Some(i64::MAX as u64 + 1)).is_err());
        assert!(validate_cursor(Some(u64::MAX)).is_err());
    }
}
