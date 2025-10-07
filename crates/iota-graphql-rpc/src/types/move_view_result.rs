// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_json_rpc_types::{IotaMoveValue, IotaMoveViewCallResults};
use serde::Serialize;

use crate::{error::Error, types::json::Json};

/// The result of a move-view function call.
///
/// Execution errors are captured in the `error` field, in which
/// case the `results` field will be `None`.
///
/// On success, the `results` field will contain the return values of the
/// move view function, and the `error` field will be `None`.
#[derive(Clone, Debug, Serialize, Default, SimpleObject)]
pub(crate) struct MoveViewResult {
    /// Execution error from executing the move view call.
    error: Option<String>,
    /// The return values of the move view function.
    results: Option<Vec<Json>>,
}

impl TryFrom<IotaMoveViewCallResults> for MoveViewResult {
    type Error = Error;

    fn try_from(value: IotaMoveViewCallResults) -> Result<Self, Error> {
        let mut result = Self::default();
        match value {
            IotaMoveViewCallResults::Error(e) => result.error = Some(e),
            IotaMoveViewCallResults::Results(results) => {
                result.results = Some(
                    results
                        .into_iter()
                        .map(TryInto::try_into)
                        .collect::<Result<_, _>>()?,
                );
            }
        }
        Ok(result)
    }
}

impl TryFrom<IotaMoveValue> for Json {
    type Error = Error;

    fn try_from(value: IotaMoveValue) -> Result<Self, Error> {
        let json = serde_json::to_value(value).map_err(|e| Error::Internal(e.to_string()))?;

        Value::try_from(json)
            .map_err(|e| Error::Internal(e.to_string()))
            .map(Self)
    }
}
