// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use async_graphql::*;
use iota_indexer::{apis::RawSimulationOutput, types::IndexedBalanceChange};
use iota_json_rpc_types::{DevInspectResults, IotaExecutionResult};
use iota_sdk_types::{
    Transaction as NativeTransactionData, TransactionEffects as NativeTransactionEffects,
    TransactionEvents, TypeTag,
};

use crate::{
    consistency::UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER,
    error::Error,
    types::{
        base64::Base64,
        big_int::BigInt,
        move_type::MoveType,
        transaction_block::{TransactionBlock, TransactionBlockInner},
        transaction_block_kind::programmable::TransactionArgument,
    },
};

#[derive(Clone, Debug, SimpleObject)]
pub(crate) struct DryRunResult {
    /// The error that occurred during dry run execution, if any.
    pub error: Option<String>,
    /// The intermediate results for each command of the dry run execution,
    /// including contents of mutated references and return values.
    pub results: Option<Vec<DryRunEffect>>,
    /// The transaction block representing the dry run execution.
    pub transaction: Option<TransactionBlock>,
    /// If an input object is congested, suggest a gas price to use.
    pub suggested_gas_price: Option<BigInt>,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct DryRunEffect {
    /// Changes made to arguments that were mutably borrowed by each command in
    /// this transaction.
    pub mutated_references: Option<Vec<DryRunMutation>>,

    /// Return results of each command in this transaction.
    pub return_values: Option<Vec<DryRunReturn>>,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct DryRunMutation {
    pub input: TransactionArgument,

    pub type_: MoveType,

    pub bcs: Base64,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct DryRunReturn {
    pub type_: MoveType,

    pub bcs: Base64,
}
impl TryFrom<IotaExecutionResult> for DryRunEffect {
    type Error = crate::error::Error;

    fn try_from(result: IotaExecutionResult) -> Result<Self, Self::Error> {
        let mutated_references = result
            .mutable_reference_outputs
            .iter()
            .map(|(argument, bcs, type_)| {
                let tag: TypeTag = type_.clone().try_into()?;
                Ok(DryRunMutation {
                    input: (*argument).into(),
                    type_: tag.into(),
                    bcs: bcs.into(),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()
            .map_err(|e| {
                Error::Internal(format!(
                    "Failed to parse results returned from dev inspect: {e:?}"
                ))
            })?;
        let return_values = result
            .return_values
            .iter()
            .map(|(bcs, type_)| {
                let tag: TypeTag = type_.clone().try_into()?;
                Ok(DryRunReturn {
                    type_: tag.into(),
                    bcs: bcs.into(),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()
            .map_err(|e| {
                Error::Internal(format!(
                    "Failed to parse results returned from dev inspect: {e:?}"
                ))
            })?;
        Ok(Self {
            mutated_references: Some(mutated_references),
            return_values: Some(return_values),
        })
    }
}

impl TryFrom<DevInspectResults> for DryRunResult {
    type Error = crate::error::Error;
    fn try_from(dev_inspect_results: DevInspectResults) -> Result<Self, Self::Error> {
        // Results might be None in the event of a transaction failure.
        let results = if let Some(results) = dev_inspect_results.results {
            Some(
                results
                    .into_iter()
                    .map(DryRunEffect::try_from)
                    .collect::<Result<Vec<_>, Error>>()?,
            )
        } else {
            None
        };
        let events = dev_inspect_results
            .events
            .data
            .into_iter()
            .map(|e| e.into())
            .collect();
        let effects: NativeTransactionEffects = bcs::from_bytes(&dev_inspect_results.raw_effects)
            .map_err(|e| {
            Error::Internal(format!("Unable to deserialize transaction effects: {e}"))
        })?;
        let tx_data: NativeTransactionData = bcs::from_bytes(&dev_inspect_results.raw_txn_data)
            .map_err(|e| Error::Internal(format!("Unable to deserialize transaction data: {e}")))?;
        let transaction = Some(TransactionBlock {
            inner: TransactionBlockInner::Simulated {
                tx_data,
                effects,
                events,
                // We do not return balance changes or object changes for dev-inspect.
                balance_changes: vec![],
                input_objects: Arc::new(BTreeMap::new()),
                output_objects: Arc::new(BTreeMap::new()),
            },
            // A simulated transaction uses the fullnode's state, which is typically ahead of
            // the indexed state, so it is not tied to a checkpoint.
            checkpoint_viewed_at: UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER,
        });
        Ok(Self {
            error: dev_inspect_results.error,
            results,
            transaction,
            // Dev inspect does not report a suggested gas price.
            suggested_gas_price: None,
        })
    }
}

impl TryFrom<RawSimulationOutput> for DryRunResult {
    type Error = crate::error::Error;

    fn try_from(simulation: RawSimulationOutput) -> Result<Self, Self::Error> {
        // Take the fields GraphQL needs from the raw simulation, ignoring the ones we
        // are not requesting for graphql.
        let RawSimulationOutput {
            transaction,
            effects,
            events: TransactionEvents(events),
            balance_changes,
            input_objects,
            output_objects,
            command_results,
            suggested_gas_price,
            execution_error,
            ..
        } = simulation;

        // The node returns per-command results or an execution error, never both,
        // so `command_results` is `None` for a failed dry run.
        let results = command_results
            .map(|results| {
                results
                    .into_iter()
                    .map(DryRunEffect::try_from)
                    .collect::<Result<Vec<_>, Error>>()
            })
            .transpose()?;

        let create_objects_map = |objects: Vec<iota_types::object::Object>| {
            Arc::new(
                objects
                    .into_iter()
                    .map(|object| (object.id(), object))
                    .collect::<BTreeMap<_, _>>(),
            )
        };

        let transaction = Some(TransactionBlock {
            inner: TransactionBlockInner::Simulated {
                tx_data: transaction,
                effects,
                events,
                balance_changes: balance_changes
                    .into_iter()
                    .map(IndexedBalanceChange::from)
                    .collect(),
                input_objects: create_objects_map(input_objects),
                output_objects: create_objects_map(output_objects),
            },
            // A simulated transaction uses the fullnode's state, which is typically ahead of
            // the indexed state, so it is not tied to a checkpoint.
            checkpoint_viewed_at: UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER,
        });
        Ok(Self {
            error: execution_error,
            results,
            transaction,
            suggested_gas_price: suggested_gas_price.map(BigInt::from),
        })
    }
}
