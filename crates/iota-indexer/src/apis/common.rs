// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Collections of common functions and methods used by the JSON-RPC APIs.

use iota_json_rpc::error::IotaRpcInputError;
use iota_json_rpc_api::QUERY_MAX_RESULT_LIMIT;
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

/// Checks if the provided cursor does not exceed the Postgres `Int8` column represented as a `i64`.
pub(crate) fn validate_cursor(cursor: Option<u64>) -> RpcResult<()> {
    if cursor.is_some_and(|c| i64::try_from(c).is_err()) {
        return Err(IotaRpcInputError::GenericInvalid("cursor out of range".into()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_cursor;

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
