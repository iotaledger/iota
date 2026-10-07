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
