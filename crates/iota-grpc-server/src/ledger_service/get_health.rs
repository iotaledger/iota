// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::{SystemTime, UNIX_EPOCH};

use iota_grpc_types::v1::ledger_service::{GetHealthRequest, GetHealthResponse};
use tonic::Code;

use crate::{error::RpcError, ledger_service::LedgerGrpcService};

/// Default health check threshold: 5 seconds.
const DEFAULT_THRESHOLD_MS: u64 = 5_000;

#[tracing::instrument(skip(service))]
pub fn get_health(
    service: &LedgerGrpcService,
    request: GetHealthRequest,
) -> Result<GetHealthResponse, RpcError> {
    let latest_checkpoint = service.reader.get_latest_checkpoint()?;

    let threshold_ms = request.threshold_ms.unwrap_or(DEFAULT_THRESHOLD_MS);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RpcError::new(Code::Internal, e))?
        .as_millis() as u64;

    let checkpoint_ts = latest_checkpoint.timestamp_ms;

    if now_ms.saturating_sub(checkpoint_ts) > threshold_ms {
        return Err(RpcError::new(
            Code::Unavailable,
            format!(
                "Latest checkpoint timestamp is beyond the threshold of {}ms",
                threshold_ms
            ),
        ));
    }

    Ok(GetHealthResponse::default()
        .with_executed_checkpoint_height(latest_checkpoint.sequence_number))
}
