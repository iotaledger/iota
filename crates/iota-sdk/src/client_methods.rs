// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use iota_json_rpc_types::{
    DevInspectArgs, IotaArgument, IotaExecutionResult, IotaExecutionStatus, IotaObjectData,
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery, IotaPastObjectResponse,
    IotaRawData, IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponseOptions, IotaTypeTag,
};
use iota_sdk_transaction_builder::{
    ClientMethods, DryRunEffect, DryRunMutation, DryRunResult, DryRunReturn, TransactionArgument,
    WaitForTx,
};
use iota_sdk_types::{
    Address, Digest, Object, ObjectId, SignedTransaction, Transaction, TransactionEffects, TypeTag,
    UserSignature, Version,
    effects::TransactionEffectsV1,
    execution_status::{ExecutionError, ExecutionStatus},
    gas::GasCostSummary as SdkGasCostSummary,
};
use iota_types::{
    base_types::SequenceNumber,
    coin::Coin,
    iota_serde::BigInt,
    object::Object as CoreObject,
    transaction::{SenderSignedData, Transaction as CoreTransaction, TransactionDataAPI},
};

use crate::{IotaClient, error::Error};

impl ClientMethods for IotaClient {
    type Error = crate::error::Error;
    type DryRunResult = DryRunResult;

    async fn object(
        &self,
        object_id: ObjectId,
        version: impl Into<Option<Version>>,
    ) -> Result<Option<Object>, Self::Error> {
        let data = if let Some(v) = version.into() {
            match self
                .read_api()
                .try_get_object_before_version(object_id, SequenceNumber::from_u64(v.as_u64()))
                .await?
            {
                IotaPastObjectResponse::VersionFound(d) => Some(d),
                _ => None,
            }
        } else {
            self.read_api()
                .get_object_with_options(object_id, IotaObjectDataOptions::bcs_lossless())
                .await?
                .data
        };

        data.map(data_to_sdk_object).transpose()
    }

    async fn objects(
        &self,
        type_tag: Option<TypeTag>,
        owner: Option<Address>,
        object_ids: Option<Vec<ObjectId>>,
        _ascending: bool,
        _cursor: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<Object>, Self::Error> {
        if let Some(ids) = object_ids {
            let responses = self
                .read_api()
                .multi_get_object_with_options(ids, IotaObjectDataOptions::bcs_lossless())
                .await?;
            responses
                .into_iter()
                .filter_map(|r| r.data)
                .map(data_to_sdk_object)
                .collect()
        } else if let Some(owner) = owner {
            // Only struct types can be used as a filter on this endpoint.
            let filter = type_tag.and_then(|tt| match tt {
                TypeTag::Struct(s) => Some(IotaObjectDataFilter::StructType(*s)),
                _ => None,
            });
            let query = Some(IotaObjectResponseQuery {
                filter,
                options: Some(IotaObjectDataOptions::bcs_lossless()),
            });
            let page = self
                .read_api()
                .get_owned_objects(owner, query, None, limit)
                .await?;
            page.data
                .into_iter()
                .filter_map(|r| r.data)
                .map(data_to_sdk_object)
                .collect()
        } else {
            Ok(vec![])
        }
    }

    async fn transaction(&self, digest: Digest) -> Result<Option<SignedTransaction>, Self::Error> {
        let options = IotaTransactionBlockResponseOptions {
            show_raw_input: true,
            ..Default::default()
        };
        let response = self
            .read_api()
            .get_transaction_with_options(digest, options)
            .await?;

        if response.raw_transaction.is_empty() {
            return Ok(None);
        }

        let sender_signed: SenderSignedData = bcs::from_bytes(&response.raw_transaction)
            .map_err(|e| Error::Data(format!("failed to deserialize transaction: {e}")))?;

        sender_signed
            .try_into()
            .map(Some)
            .map_err(|e| Error::Data(format!("failed to convert transaction: {e:?}")))
    }

    async fn transaction_effects(
        &self,
        digest: Digest,
    ) -> Result<Option<TransactionEffects>, Self::Error> {
        let options = IotaTransactionBlockResponseOptions {
            show_raw_effects: true,
            ..Default::default()
        };
        let response = self
            .read_api()
            .get_transaction_with_options(digest, options)
            .await?;

        if response.raw_effects.is_empty() {
            return Ok(None);
        }

        bcs::from_bytes(&response.raw_effects)
            .map(Some)
            .map_err(|e| Error::Data(format!("failed to deserialize effects: {e}")))
    }

    async fn reference_gas_price(
        &self,
        _epoch: impl Into<Option<u64>>,
    ) -> Result<Option<u64>, Self::Error> {
        // `iota_getReferenceGasPrice` only returns the current epoch's value.
        // Per-epoch RGP lives in `iotax_getEpochs`, which requires an indexer,
        // so the epoch parameter is ignored here.
        Ok(Some(self.read_api().get_reference_gas_price().await?))
    }

    async fn estimate_tx_budget(&self, tx: &Transaction) -> Result<Option<u64>, Self::Error> {
        let res = self.dry_run_tx(tx, true).await?;
        Ok(res.effects.map(|e| e.gas_summary().gas_used()))
    }

    async fn dry_run_tx(
        &self,
        tx: &Transaction,
        skip_checks: bool,
    ) -> Result<DryRunResult, Self::Error> {
        let mut tx_for_estimation = tx.clone();
        let Transaction::V1(v1) = &mut tx_for_estimation else {
            unimplemented!("a new Transaction enum variant was added and needs to be handled")
        };

        // Set a temporary gas budget so the dry run doesn't fail for lack of
        // funds: if gas coins are listed, cap at their total balance; otherwise
        // use the default.
        if !v1.gas_payment.objects.is_empty() {
            let mut total_balance = 0u64;
            for coin_ref in &v1.gas_payment.objects {
                let obj = self
                    .read_api()
                    .get_object_with_options(
                        coin_ref.object_id,
                        IotaObjectDataOptions::new().with_bcs(),
                    )
                    .await?;
                if let Some(IotaObjectData {
                    bcs: Some(IotaRawData::MoveObject(raw)),
                    ..
                }) = obj.data
                {
                    let coin: Coin = bcs::from_bytes(&raw.bcs_bytes)?;
                    total_balance += coin.balance.value();
                }
            }
            // Reject up front when the supplied gas coins can't cover the
            // network's minimum budget; otherwise the dry run would just
            // surface the same condition as an opaque OutOfGas effect.
            let min_budget = v1
                .gas_payment
                .price
                .saturating_mul(MIN_GAS_BUDGET_MULTIPLIER);
            if total_balance < min_budget {
                return Err(Error::InsufficientFunds {
                    address: v1.gas_payment.owner,
                    amount: min_budget as u128,
                });
            }
            v1.gas_payment.budget = total_balance.min(DEFAULT_DRY_RUN_BUDGET_NANOS);
        } else if v1.gas_payment.budget == 0 {
            v1.gas_payment.budget = DEFAULT_DRY_RUN_BUDGET_NANOS;
        }

        let gas_objects =
            (!v1.gas_payment.objects.is_empty()).then(|| v1.gas_payment.objects.clone());
        let gas_sponsor = v1.gas_payment.owner;
        let gas_budget = v1.gas_payment.budget;
        let gas_price = v1.gas_payment.price;

        if skip_checks {
            let sender = tx_for_estimation.sender();

            let dev_inspect_args = DevInspectArgs {
                gas_sponsor: Some(gas_sponsor),
                gas_budget: Some(gas_budget),
                gas_objects,
                skip_checks: Some(true),
                show_raw_txn_data_and_effects: Some(false),
            };

            let dev_inspect = self
                .read_api()
                .dev_inspect_transaction_block(
                    sender,
                    tx_for_estimation.into_kind(),
                    Some(BigInt::from(gas_price)),
                    None,
                    Some(dev_inspect_args),
                )
                .await?;

            let results = dev_inspect
                .results
                .unwrap_or_default()
                .into_iter()
                .map(convert_effect)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(DryRunResult {
                error: dev_inspect.error,
                results,
                transaction: None,
                effects: Some(rpc_effects_to_sdk(&dev_inspect.effects)),
            })
        } else {
            let response = self
                .read_api()
                .dry_run_transaction_block(tx_for_estimation)
                .await?;

            Ok(DryRunResult {
                error: response.execution_error_source,
                results: vec![],
                transaction: None,
                effects: Some(rpc_effects_to_sdk(&response.effects)),
            })
        }
    }

    async fn execute_tx(
        &self,
        signatures: &[UserSignature],
        tx: &Transaction,
        wait_for: impl Into<Option<WaitForTx>>,
    ) -> Result<TransactionEffects, Self::Error> {
        let signed_tx = SignedTransaction {
            transaction: tx.clone(),
            signatures: signatures.to_vec(),
        };

        let core_tx: CoreTransaction = signed_tx
            .try_into()
            .map_err(|e| Error::Data(format!("failed to convert signed transaction: {e:?}")))?;

        let options = IotaTransactionBlockResponseOptions {
            show_effects: true,
            show_raw_effects: true,
            ..Default::default()
        };
        let response = self
            .quorum_driver_api()
            .execute_transaction_block(core_tx, options, None)
            .await?;

        if let Some(wait) = wait_for.into() {
            self.wait_for_tx(response.digest, wait).await?;
        }

        if response.raw_effects.is_empty() {
            return Err(Error::Data(
                "no effects returned from execution".to_string(),
            ));
        }

        bcs::from_bytes(&response.raw_effects)
            .map_err(|e| Error::Data(format!("failed to deserialize effects: {e}")))
    }

    async fn wait_for_tx(&self, digest: Digest, wait_for: WaitForTx) -> Result<(), Self::Error> {
        let timeout = Duration::from_secs(60);
        let poll_interval = Duration::from_millis(100);

        tokio::time::timeout(timeout, async {
            let mut interval = tokio::time::interval(poll_interval);
            loop {
                interval.tick().await;

                let options = IotaTransactionBlockResponseOptions {
                    show_effects: true,
                    ..Default::default()
                };
                let resp = self
                    .read_api()
                    .get_transaction_with_options(digest, options)
                    .await?;

                let is_ready = match wait_for {
                    // Checkpoint inclusion indicates finalization.
                    WaitForTx::Finalized => resp.checkpoint.is_some(),
                    // Otherwise wait until effects are queryable.
                    _ => resp.effects.is_some(),
                };

                if is_ready {
                    break Ok(());
                }
            }
        })
        .await
        .map_err(|_| Error::Data("timeout waiting for transaction".to_string()))?
    }
}

/// Default dry-run gas budget when none is supplied: 50 IOTA in nanos.
const DEFAULT_DRY_RUN_BUDGET_NANOS: u64 = 50_000_000_000;

/// Network-enforced minimum gas budget multiplier: `base_tx_cost_fixed`.
const MIN_GAS_BUDGET_MULTIPLIER: u64 = 1000;

fn data_to_sdk_object(data: IotaObjectData) -> Result<Object, Error> {
    let core: CoreObject = data
        .try_into()
        .map_err(|e| Error::Data(format!("object conversion failed: {e}")))?;
    core.try_into()
        .map_err(|e| Error::Data(format!("object sdk conversion failed: {e}")))
}

fn rpc_effects_to_sdk(effects: &IotaTransactionBlockEffects) -> TransactionEffects {
    let rpc_gas = effects.gas_cost_summary();
    let gas_used = SdkGasCostSummary::new(
        rpc_gas.computation_cost,
        rpc_gas.computation_cost_burned,
        rpc_gas.storage_cost,
        rpc_gas.storage_rebate,
        rpc_gas.non_refundable_storage_fee,
    );

    let status = match effects.status() {
        IotaExecutionStatus::Success => ExecutionStatus::Success,
        // The RPC response carries the error as a string only; the structured
        // ExecutionError variant isn't reconstructible from it.
        IotaExecutionStatus::Failure { .. } => ExecutionStatus::Failure {
            error: ExecutionError::InvariantViolation,
            command: None,
        },
    };

    TransactionEffects::V1(Box::new(TransactionEffectsV1 {
        status,
        epoch: effects.executed_epoch(),
        gas_used,
        transaction_digest: *effects.transaction_digest(),
        gas_object_index: None,
        events_digest: effects.events_digest().copied(),
        dependencies: effects.dependencies().to_vec(),
        lamport_version: Version::from_u64(0),
        changed_objects: vec![],
        unchanged_shared_objects: vec![],
        auxiliary_data_digest: None,
    }))
}

fn convert_argument(arg: IotaArgument) -> TransactionArgument {
    match arg {
        IotaArgument::GasCoin => TransactionArgument::GasCoin,
        IotaArgument::Input(ix) => TransactionArgument::Input { index: ix as u32 },
        IotaArgument::Result(cmd) => TransactionArgument::Result {
            cmd: cmd as u32,
            index: None,
        },
        IotaArgument::NestedResult(cmd, ix) => TransactionArgument::Result {
            cmd: cmd as u32,
            index: Some(ix as u32),
        },
    }
}

fn convert_type_tag(type_tag: IotaTypeTag) -> Result<TypeTag, Error> {
    TypeTag::from_str(type_tag.as_ref())
        .map_err(|e| Error::Data(format!("failed to parse type tag: {e:?}")))
}

fn convert_mutation(
    (arg, bcs, type_tag): (IotaArgument, Vec<u8>, IotaTypeTag),
) -> Result<DryRunMutation, Error> {
    Ok(DryRunMutation {
        input: convert_argument(arg),
        type_tag: convert_type_tag(type_tag)?,
        bcs,
    })
}

fn convert_return((bcs, type_tag): (Vec<u8>, IotaTypeTag)) -> Result<DryRunReturn, Error> {
    Ok(DryRunReturn {
        type_tag: convert_type_tag(type_tag)?,
        bcs,
    })
}

fn convert_effect(exec: IotaExecutionResult) -> Result<DryRunEffect, Error> {
    Ok(DryRunEffect {
        mutated_references: exec
            .mutable_reference_outputs
            .into_iter()
            .map(convert_mutation)
            .collect::<Result<_, _>>()?,
        return_values: exec
            .return_values
            .into_iter()
            .map(convert_return)
            .collect::<Result<_, _>>()?,
    })
}
