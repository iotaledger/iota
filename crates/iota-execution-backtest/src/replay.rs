// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Re-execution of one checkpointed user transaction with two executors, compared with the
//! effects the chain recorded.

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};
use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    EpochId, GasCostSummary, InputSharedObject, MoveAuthenticator, ObjectId, TransactionEffects,
    TransactionKind, Version,
};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefForExecution, MoveAuthenticatorForExecution,
        MoveAuthenticatorsForExecution, authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
    },
    auth_context::AuthContextData,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::ExecutionErrorKind,
    full_checkpoint_content::CheckpointTransaction,
    gas::IotaGasStatus,
    metrics::LimitsMetrics,
    object::Object,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult, ObjectReadResultKind,
        SenderSignedTransactionAPI, TransactionAPI,
    },
};
use serde::Serialize;
use tokio::runtime::Handle;

use crate::{
    rpc::Rpc,
    store::{PackageCache, TransactionStore},
};

/// What stays the same for every transaction of the replayed epoch.
pub struct EpochContext {
    pub epoch: EpochId,
    pub epoch_start_timestamp_ms: u64,
    pub reference_gas_price: u64,
    pub protocol_config: ProtocolConfig,
    pub rpc: Arc<Rpc>,
    pub packages: Arc<PackageCache>,
    pub metrics: Arc<LimitsMetrics>,
    pub control: Arc<dyn Executor + Send + Sync>,
    pub candidate: Arc<dyn Executor + Send + Sync>,
}

/// How one execution compares with the effects the chain recorded.
#[derive(Serialize, Clone)]
pub struct ExecutionReport {
    pub matches: bool,
    pub status: String,
    pub gas: GasCostSummary,
    /// Effects fields that differ from the chain, empty when `matches`.
    pub differences: Vec<String>,
    pub store_errors: Vec<String>,
}

#[derive(Serialize)]
pub struct TransactionReport {
    pub checkpoint: u64,
    pub digest: String,
    pub move_authenticators: usize,
    pub chain_status: String,
    pub chain_gas: GasCostSummary,
    pub control: Option<ExecutionReport>,
    pub candidate: Option<ExecutionReport>,
    /// Set when the transaction could not be replayed.
    pub skipped: Option<String>,
    pub objects_fetched_during_execution: usize,
}

/// Whether the transaction is a user programmable transaction.
pub fn is_user_programmable(tx: &CheckpointTransaction) -> bool {
    matches!(
        tx.transaction.data().transaction().kind(),
        TransactionKind::Programmable(_)
    )
}

pub async fn replay_transaction(
    ctx: Arc<EpochContext>,
    checkpoint: u64,
    tx: CheckpointTransaction,
) -> TransactionReport {
    let signed = tx.transaction.data();
    let mut report = TransactionReport {
        checkpoint,
        digest: tx.effects.transaction_digest().to_string(),
        move_authenticators: signed.move_authenticators().len(),
        chain_status: format!("{:?}", tx.effects.status()),
        chain_gas: tx.effects.gas_cost_summary().clone(),
        control: None,
        candidate: None,
        skipped: None,
        objects_fetched_during_execution: 0,
    };
    let prepared = match prepare(&ctx, &tx).await {
        Ok(prepared) => prepared,
        Err(e) => {
            report.skipped = Some(format!("{e:#}"));
            return report;
        }
    };
    let handle = Handle::current();
    let ctx_for_run = ctx.clone();
    let run = tokio::task::spawn_blocking(move || {
        let control = execute(&ctx_for_run, &ctx_for_run.control, &tx, &prepared, &handle);
        let candidate = execute(
            &ctx_for_run,
            &ctx_for_run.candidate,
            &tx,
            &prepared,
            &handle,
        );
        (control, candidate)
    })
    .await;
    match run {
        Ok(((control, control_fetched), (candidate, candidate_fetched))) => {
            report.control = Some(control);
            report.candidate = Some(candidate);
            report.objects_fetched_during_execution = control_fetched.max(candidate_fetched);
        }
        Err(e) => report.skipped = Some(format!("execution panicked: {e}")),
    }
    report
}

/// A Move authenticator with its function ref and inputs, from which each execution builds its
/// own `MoveAuthenticatorForExecution`.
struct AuthenticatorInputs {
    authenticator: MoveAuthenticator,
    function_ref: AuthenticatorFunctionRefForExecution,
    input_objects: InputObjects,
}

/// Everything read before execution starts.
struct Prepared {
    input_objects: InputObjects,
    objects: BTreeMap<ObjectId, Object>,
    authenticators: Option<(Vec<AuthenticatorInputs>, AuthContextData)>,
}

async fn prepare(ctx: &EpochContext, tx: &CheckpointTransaction) -> Result<Prepared> {
    let signed = tx.transaction.data();
    let data = signed.transaction();
    let effects = &tx.effects;
    let max_input_version = Version::from_u64(
        effects
            .lamport_version()
            .as_u64()
            .checked_sub(1)
            .context("lamport version 0")?,
    );

    // Objects the transaction modified, as they were before it ran.
    let mut objects: BTreeMap<ObjectId, Object> = tx
        .input_objects
        .iter()
        .map(|object| (object.id(), object.clone()))
        .collect();

    let shared_inputs: BTreeMap<ObjectId, InputSharedObject> = effects
        .input_shared_objects()
        .into_iter()
        .filter_map(|shared| shared_object_id(&shared).map(|id| (id, shared)))
        .collect();

    let kinds = signed
        .collect_all_input_object_kind_for_reading(
            ctx.protocol_config
                .move_authenticator_type_arg_packages_as_inputs(),
        )
        .map_err(|e| anyhow!("input objects: {e}"))?;

    let mut read_results = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let result_kind = match kind {
            InputObjectKind::MovePackage(id) => {
                let package = ctx
                    .packages
                    .get(&id)
                    .await?
                    .with_context(|| format!("package {id} not found"))?;
                ObjectReadResultKind::Object(package)
            }
            InputObjectKind::ImmOrOwnedMoveObject(object_ref) => {
                let object =
                    object_at(ctx, &mut objects, object_ref.object_id, object_ref.version).await?;
                ObjectReadResultKind::Object(object)
            }
            InputObjectKind::SharedMoveObject { id, .. } => match shared_inputs.get(&id) {
                Some(InputSharedObject::Mutate(object_ref))
                | Some(InputSharedObject::ReadOnly(object_ref)) => {
                    let object = object_at(
                        ctx,
                        &mut objects,
                        *object_ref.object_id(),
                        object_ref.version(),
                    )
                    .await?;
                    ObjectReadResultKind::Object(object)
                }
                Some(InputSharedObject::Canceled(object_version)) => {
                    ObjectReadResultKind::CancelledTransactionObject(object_version.version())
                }
                Some(InputSharedObject::ReadDeleted(_))
                | Some(InputSharedObject::MutateDeleted(_)) => {
                    bail!("shared input {id} was already deleted (not supported by the backtest)")
                }
                Some(other) => bail!("unsupported shared input {other:?}"),
                None => bail!("shared input {id} missing from the effects"),
            },
        };
        read_results.push(ObjectReadResult::new(kind, result_kind));
    }
    let input_objects = InputObjects::new(read_results);

    // Objects the transaction may receive.
    for receiving in data.receiving_objects() {
        if !objects.contains_key(&receiving.object_id)
            && let Some(object) = ctx
                .rpc
                .object_at_version(receiving.object_id, receiving.version)
                .await?
        {
            objects.insert(receiving.object_id, object);
        }
    }

    let authenticators = if signed.move_authenticators().is_empty() {
        None
    } else {
        Some(
            prepare_authenticators(ctx, tx, &input_objects, &mut objects, max_input_version)
                .await?,
        )
    };

    Ok(Prepared {
        input_objects,
        objects,
        authenticators,
    })
}

async fn prepare_authenticators(
    ctx: &EpochContext,
    tx: &CheckpointTransaction,
    input_objects: &InputObjects,
    objects: &mut BTreeMap<ObjectId, Object>,
    max_input_version: Version,
) -> Result<(Vec<AuthenticatorInputs>, AuthContextData)> {
    let signed = tx.transaction.data();
    let (_, per_authenticator_inputs) = signed
        .split_input_objects_into_groups_for_reading(input_objects.clone())
        .map_err(|e| anyhow!("splitting authenticator inputs: {e}"))?;
    let mut authenticators = vec![];
    for (authenticator, (authenticator_inputs, account_object)) in signed
        .move_authenticators()
        .into_iter()
        .zip(per_authenticator_inputs)
    {
        let ObjectReadResultKind::Object(account) = &account_object.object else {
            bail!("account object {} is not available", account_object.id());
        };
        let account_id = account.id();
        let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id)
            .map_err(|e| anyhow!("{e}"))?;
        let field = match objects.get(&field_id) {
            Some(field) => field.clone(),
            None => {
                let bound = account.version().min(max_input_version);
                let field = ctx
                    .rpc
                    .object_at_or_before_version(field_id, bound)
                    .await?
                    .with_context(|| format!("authenticator field {field_id} not found"))?;
                objects.insert(field_id, field.clone());
                field
            }
        };
        let function_ref =
            authenticator_function_ref_v1_from_dynamic_field_object(account_id, &field)
                .map_err(|e| anyhow!("{e}"))?;
        authenticators.push(AuthenticatorInputs {
            authenticator: authenticator.to_owned(),
            function_ref,
            input_objects: authenticator_inputs,
        });
    }

    let data = signed.transaction();
    let gas_owner = data.gas_owner();
    let (sender_auth_digest, sponsor_auth_digest) = signed
        .compute_auth_digests()
        .map_err(|e| anyhow!("auth digests: {e}"))?;
    let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
        extract_auth_fun_refs(data.sender(), gas_owner, |address| {
            authenticators
                .iter()
                .find(|a| a.authenticator.address() == address)
                .map(|a| a.function_ref.authenticator_function_ref.clone())
        });
    let auth_context_data = AuthContextData {
        transaction_data_bytes: bcs::to_bytes(data).context("serializing transaction data")?,
        sender_auth_digest,
        sponsor_auth_digest,
        sender_authenticator_function_ref,
        sponsor_authenticator_function_ref,
    };
    Ok((authenticators, auth_context_data))
}

/// The object at exactly `version`: from the checkpoint if it carries that version, else from
/// the full node.
async fn object_at(
    ctx: &EpochContext,
    objects: &mut BTreeMap<ObjectId, Object>,
    id: ObjectId,
    version: Version,
) -> Result<Object> {
    if let Some(object) = objects.get(&id)
        && object.version() == version
    {
        return Ok(object.clone());
    }
    let object = ctx
        .rpc
        .object_at_version(id, version)
        .await?
        .with_context(|| format!("object {id} at version {version} not found"))?;
    objects.insert(id, object.clone());
    Ok(object)
}

fn shared_object_id(shared: &InputSharedObject) -> Option<ObjectId> {
    match shared {
        InputSharedObject::Mutate(object_ref) | InputSharedObject::ReadOnly(object_ref) => {
            Some(*object_ref.object_id())
        }
        InputSharedObject::ReadDeleted(object_version)
        | InputSharedObject::MutateDeleted(object_version)
        | InputSharedObject::Canceled(object_version) => Some(*object_version.object_id()),
        _ => None,
    }
}

/// Runs the transaction with `executor` against a fresh store and compares the effects with the
/// chain. Returns the report and the number of objects read during execution.
fn execute(
    ctx: &EpochContext,
    executor: &Arc<dyn Executor + Send + Sync>,
    tx: &CheckpointTransaction,
    prepared: &Prepared,
    handle: &Handle,
) -> (ExecutionReport, usize) {
    let signed = tx.transaction.data();
    let data = signed.transaction();
    let max_input_version =
        Version::from_u64(tx.effects.lamport_version().as_u64().saturating_sub(1));
    let store = TransactionStore::new(
        ctx.rpc.clone(),
        ctx.packages.clone(),
        handle.clone(),
        max_input_version,
        prepared.objects.clone(),
    );
    let gas_payment = data.gas_data().clone();
    let gas_status = match IotaGasStatus::new(
        gas_payment.budget,
        gas_payment.price,
        ctx.reference_gas_price,
        &ctx.protocol_config,
    ) {
        Ok(gas_status) => gas_status,
        Err(e) => {
            return (
                ExecutionReport {
                    matches: false,
                    status: format!("gas status: {e}"),
                    gas: GasCostSummary::default(),
                    differences: vec!["gas status".to_string()],
                    store_errors: vec![],
                },
                0,
            );
        }
    };
    let digest = *tx.effects.transaction_digest();
    let deny_set = HashSet::new();
    let input_objects = CheckedInputObjects::new_for_replay(prepared.input_objects.clone());
    let effects = match &prepared.authenticators {
        None => {
            executor
                .execute_transaction_to_effects(
                    &store,
                    &ctx.protocol_config,
                    ctx.metrics.clone(),
                    false,
                    &deny_set,
                    &ctx.epoch,
                    ctx.epoch_start_timestamp_ms,
                    input_objects,
                    gas_payment,
                    gas_status,
                    data.kind().clone(),
                    data.sender(),
                    digest,
                    &mut None,
                )
                .2
        }
        Some((authenticators, auth_context_data)) => {
            executor
                .authenticate_then_execute_transaction_to_effects(
                    &store,
                    &ctx.protocol_config,
                    ctx.metrics.clone(),
                    false,
                    &deny_set,
                    &ctx.epoch,
                    ctx.epoch_start_timestamp_ms,
                    gas_payment,
                    gas_status,
                    MoveAuthenticatorsForExecution::Resolved(
                        authenticators
                            .iter()
                            .map(|inputs| MoveAuthenticatorForExecution {
                                authenticator: inputs.authenticator.clone(),
                                function_ref: inputs.function_ref.clone(),
                                input_objects: CheckedInputObjects::new_for_replay(
                                    inputs.input_objects.clone(),
                                ),
                            })
                            .collect(),
                    ),
                    input_objects,
                    gas_owner_pays(&ctx.protocol_config),
                    data.kind().clone(),
                    data.sender(),
                    digest,
                    auth_context_data.clone(),
                    &mut None,
                )
                .2
        }
    };
    let differences = effects_differences(&tx.effects, &effects);
    (
        ExecutionReport {
            matches: differences.is_empty(),
            status: format!("{:?}", effects.status()),
            gas: effects.gas_cost_summary().clone(),
            differences,
            store_errors: store.errors(),
        },
        store.fetched_during_execution().len(),
    )
}

/// Under validator attestation, whether a gas owner whose own authentication failed pays is
/// decided by a verdict the checkpoint certifies. Like `iota-replay`, the backtest assumes the gas
/// owner paid, which is wrong only when the attestor was found at fault or the judge could not
/// decide.
fn gas_owner_pays(
    protocol_config: &ProtocolConfig,
) -> Option<&'static dyn Fn(&ExecutionErrorKind) -> bool> {
    fn always_pays(_: &ExecutionErrorKind) -> bool {
        true
    }
    protocol_config
        .enable_validator_attestation()
        .then_some(&always_pays)
}

/// Names the effects fields that differ between the chain and the replay.
fn effects_differences(chain: &TransactionEffects, replay: &TransactionEffects) -> Vec<String> {
    if chain == replay {
        return vec![];
    }
    let mut differences = vec![];
    let mut compare = |field: &str, a: String, b: String| {
        if a != b {
            differences.push(format!("{field}: chain {a} / replay {b}"));
        }
    };
    compare(
        "status",
        format!("{:?}", chain.status()),
        format!("{:?}", replay.status()),
    );
    compare(
        "gas",
        format!("{:?}", chain.gas_cost_summary()),
        format!("{:?}", replay.gas_cost_summary()),
    );
    compare(
        "changed objects",
        format!("{:?}", chain.all_changed_objects()),
        format!("{:?}", replay.all_changed_objects()),
    );
    compare(
        "events digest",
        format!("{:?}", chain.events_digest()),
        format!("{:?}", replay.events_digest()),
    );
    compare(
        "dependencies",
        format!("{:?}", chain.dependencies()),
        format!("{:?}", replay.dependencies()),
    );
    if differences.is_empty() {
        differences.push("other effects fields".to_string());
    }
    differences
}
