// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
pub(crate) mod checked {

    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
        rc::Rc,
        sync::Arc,
    };

    use iota_move_natives::all_natives;
    use iota_protocol_config::{LimitThresholdCrossed, ProtocolConfig, check_limit_by_meter};
    use iota_sdk_types::{
        Address, Argument, ChangeEpoch, ChangeEpochV2, ChangeEpochV3, ChangeEpochV4, Command,
        EndOfEpochTransactionKind, ExecutionStatus, GasCostSummary, GasPayment, GenesisTransaction,
        Identifier, Input, MoveAuthenticator, ObjectId, ProgrammableTransaction,
        RandomnessStateUpdate, SharedObjectReference, SystemPackage, TransactionDenyRulesUpdate,
        TransactionDigest, TransactionEffects, TransactionKind, Version,
    };
    use iota_types::{
        account_abstraction::authenticator_function::{
            AuthenticatorFunctionRef, AuthenticatorFunctionRefForExecution,
            AuthenticatorFunctionRefV1, MoveAuthenticatorForExecution,
            MoveAuthenticatorsForExecution,
        },
        auth_context::{AuthContext, AuthContextData},
        balance::{BALANCE_CREATE_REWARDS_FUNCTION_NAME, BALANCE_DESTROY_REBATES_FUNCTION_NAME},
        base_types::TxContext,
        clock::CONSENSUS_COMMIT_PROLOGUE_FUNCTION_NAME,
        committee::EpochId,
        error::{ExecutionError, ExecutionErrorKind},
        execution::{
            ExecutionResults, ExecutionResultsV1, ExecutionTiming, ResultWithTimings, SharedInput,
            is_any_invariant_violation, is_cancellation, unwrap_move_authentication_error,
        },
        execution_config_utils::to_binary_config,
        gas::{IotaGasStatus, IotaGasStatusAPI},
        inner_temporary_store::InnerTemporaryStore,
        iota_system_state::{ADVANCE_EPOCH_FUNCTION_NAME, AdvanceEpochParams},
        messages_checkpoint::CheckpointTimestamp,
        metrics::LimitsMetrics,
        move_authenticator::MoveAuthenticatorExt,
        object::{OBJECT_START_VERSION, Object, ObjectInner},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        randomness_state::RANDOMNESS_STATE_UPDATE_FUNCTION_NAME,
        storage::{BackingStore, Storage},
        transaction::{CancelledObjects, CheckedInputObjects, InputObjects, TransactionKindExt},
        transaction_deny_rules::{
            TRANSACTION_DENY_RULES_CREATE_FUNCTION_NAME, TRANSACTION_DENY_RULES_MODULE,
            TRANSACTION_DENY_RULES_UPDATE_FUNCTION_NAME,
        },
    };
    use move_binary_format::CompiledModule;
    use move_trace_format::format::MoveTraceBuilder;
    use move_vm_runtime::runtime::MoveRuntime;
    use tracing::{info, instrument, trace, warn};

    use crate::{
        adapter::new_move_runtime,
        compat::{ExecutionErrorTrait, ExecutionOrEarlyError, failure_status},
        execution_mode,
        execution_mode::ExecutionMode,
        gas_charger::GasCharger,
        static_programmable_transactions as SPT,
        temporary_store::TemporaryStore,
    };

    /// Everything `execute_transaction_to_effects` hands back to the executor layer.
    pub struct ExecutionOutput<Mode: ExecutionMode> {
        pub inner_store: InnerTemporaryStore,
        pub gas_status: IotaGasStatus,
        pub effects: TransactionEffects,
        pub timings: Vec<ExecutionTiming>,
        pub execution_result: Result<Mode::ExecutionResults, Mode::Error>,
    }

    /// Gas summary, execution result, and timings produced by `execute_transaction`.
    struct ExecutionOutcome<Mode: ExecutionMode> {
        cost_summary: GasCostSummary,
        execution_result: Result<Mode::ExecutionResults, Mode::Error>,
        timings: Vec<ExecutionTiming>,
    }

    #[instrument(name = "tx_execute_to_effects", level = "debug", skip_all)]
    pub fn execute_transaction_to_effects<Mode: ExecutionMode>(
        store: &dyn BackingStore,
        input_objects: CheckedInputObjects,
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        transaction_digest: TransactionDigest,
        move_vm: &Arc<MoveRuntime>,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        mut execution_params: ExecutionOrEarlyError,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> ExecutionOutput<Mode> {
        let input_objects = input_objects.into_inner();
        if let Some(kind) = input_objects_error(
            protocol_config,
            input_objects.contains_deleted_objects(),
            input_objects.get_cancelled_objects(),
        ) {
            execution_params.push_early_error(kind);
        }
        let shared_object_refs = input_objects.filter_shared_objects();
        let receiving_objects = transaction_kind.receiving_objects();
        let transaction_dependencies = input_objects.transaction_dependencies();

        let temporary_store = TemporaryStore::new(
            store,
            input_objects,
            receiving_objects,
            transaction_digest,
            protocol_config,
            *epoch_id,
            &transaction_kind,
        );

        let sponsor = resolve_sponsor(&gas_data, &transaction_signer);
        let gas_price = gas_status.gas_price();
        let rgp = gas_status.reference_gas_price();
        let gas_charger = GasCharger::new(
            transaction_digest,
            gas_data.objects.clone(),
            gas_status,
            protocol_config,
        );

        let tx_ctx = TxContext::new_from_components(
            &transaction_signer,
            &transaction_digest,
            epoch_id,
            epoch_timestamp_ms,
            rgp,
            gas_price,
            gas_data.budget,
            sponsor,
            protocol_config,
        );
        let tx_ctx = Rc::new(RefCell::new(tx_ctx));

        execute_transaction_to_effects_inner::<Mode>(
            store,
            temporary_store,
            gas_charger,
            tx_ctx,
            shared_object_refs,
            transaction_dependencies,
            transaction_kind,
            transaction_signer,
            sponsor,
            transaction_digest,
            move_vm,
            epoch_id,
            protocol_config,
            metrics,
            enable_expensive_checks,
            execution_params,
            trace_builder_opt,
            None,
        )
    }

    /// Executes a transaction whose temporary store, gas charger and transaction context are
    /// prepared, and produces its effects. `pre_execution_result` is the outcome of the Move
    /// authenticators that ran before; an error fails the transaction after the early errors.
    fn execute_transaction_to_effects_inner<Mode: ExecutionMode>(
        store: &dyn BackingStore,
        mut temporary_store: TemporaryStore<'_>,
        mut gas_charger: GasCharger,
        tx_ctx: Rc<RefCell<TxContext>>,
        shared_object_refs: Vec<SharedInput>,
        mut transaction_dependencies: BTreeSet<TransactionDigest>,
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        sponsor: Option<Address>,
        transaction_digest: TransactionDigest,
        move_vm: &Arc<MoveRuntime>,
        epoch_id: &EpochId,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        execution_params: ExecutionOrEarlyError,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        pre_execution_result: Option<Result<(), ExecutionError>>,
    ) -> ExecutionOutput<Mode> {
        let is_epoch_change = matches!(transaction_kind, TransactionKind::EndOfEpoch(_));

        let ExecutionOutcome {
            cost_summary: gas_cost_summary,
            mut execution_result,
            timings,
        } = execute_transaction::<Mode>(
            store,
            &mut temporary_store,
            transaction_kind,
            &mut gas_charger,
            tx_ctx,
            move_vm,
            protocol_config,
            metrics,
            execution_params,
            trace_builder_opt,
            pre_execution_result,
        );

        // Post-execution system-invariant checks, run after gas charging: IOTA conservation
        // (recoverable) followed by object-ownership authentication (panics on violation).
        if let Err(e) = run_invariant_checks::<Mode>(
            &mut temporary_store,
            &mut gas_charger,
            transaction_digest,
            move_vm,
            enable_expensive_checks,
            &gas_cost_summary,
            &transaction_signer,
            &sponsor,
            is_epoch_change,
        ) {
            // FIXME: we cannot fail the transaction if this is an epoch change transaction.
            execution_result = Err(e);
        }

        let status = if let Err(error) = &execution_result {
            failure_status(error)
        } else {
            ExecutionStatus::Success
        };

        #[skip_checked_arithmetic]
        trace!(
            tx_digest = ?transaction_digest,
            computation_gas_cost = gas_cost_summary.computation_cost,
            storage_gas_cost = gas_cost_summary.storage_cost,
            storage_gas_rebate = gas_cost_summary.storage_rebate,
            "Finished execution of transaction with status {:?}",
            status
        );

        // Genesis writes a special digest to indicate that an object was created during
        // genesis and not written by any normal transaction - remove that from the
        // dependencies
        transaction_dependencies.remove(&TransactionDigest::genesis_marker());

        let gas_coin = gas_charger.gas_coin();
        let (inner, effects) = temporary_store.into_effects(
            shared_object_refs,
            &transaction_digest,
            transaction_dependencies,
            gas_cost_summary,
            status,
            gas_coin,
            *epoch_id,
        );

        ExecutionOutput {
            inner_store: inner,
            gas_status: gas_charger.into_gas_status(),
            effects,
            timings,
            execution_result,
        }
    }

    /// Produces the effects of a transaction that requires Move authentication. The temporary
    /// store, gas charger and transaction context are shared by the authenticators and the
    /// transaction.
    ///
    /// When consensus cancelled the transaction, no authenticator runs and the effects carry the
    /// cancellation; with the P-COOL flow on, a gas owner that is a Move account is then
    /// not charged. Otherwise the authenticators run first:
    ///   - If one fails, gas is charged for the failed authentication and the effects carry the
    ///     error. With the P-COOL flow on, a gas owner that is a Move account and did not authorize
    ///     the transaction is not charged; see `execute_without_gas_charge_to_effects`.
    ///   - Else the transaction executes normally.
    #[instrument(
        name = "tx_authenticate_then_execute_to_effects",
        level = "debug",
        skip_all
    )]
    pub fn authenticate_then_execute_transaction_to_effects<Mode: ExecutionMode>(
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        mut execution_params: ExecutionOrEarlyError,
        // Epoch
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        // Gas related
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        // Authentication
        authenticators: MoveAuthenticatorsForExecution,
        authenticator_and_transaction_input_objects: CheckedInputObjects,
        // Decides whether a gas owner that is a Move account pays when its own authentication
        // fails; see `charge_gas_owner`.
        gas_owner_pays: Option<&dyn Fn(&ExecutionErrorKind) -> bool>,
        // Transaction
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        transaction_digest: TransactionDigest,
        auth_context_data: AuthContextData,
        // Tracing
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        // VM
        move_vm: &Arc<MoveRuntime>,
    ) -> (ExecutionOutput<Mode>, Option<ExecutionErrorKind>) {
        // Input objects come from both authentication and transaction inputs.
        let input_objects = authenticator_and_transaction_input_objects.into_inner();
        let shared_object_refs = input_objects.filter_shared_objects();
        // Receiving objects can only come from the transaction inputs.
        let transaction_receiving_objects = transaction_kind.receiving_objects();
        let transaction_dependencies = input_objects.transaction_dependencies();
        let contains_deleted_input = input_objects.contains_deleted_objects();
        let cancelled_objects = input_objects.get_cancelled_objects();

        let mut temporary_store = TemporaryStore::new(
            store,
            input_objects,
            transaction_receiving_objects,
            transaction_digest,
            protocol_config,
            *epoch_id,
            &transaction_kind,
        );

        let sponsor = resolve_sponsor(&gas_data, &transaction_signer);
        let gas_owner = gas_data.owner;
        let gas_price = gas_status.gas_price();
        let rgp = gas_status.reference_gas_price();
        let mut gas_charger = GasCharger::new(
            transaction_digest,
            gas_data.objects.clone(),
            gas_status,
            protocol_config,
        );

        let tx_ctx = TxContext::new_from_components(
            &transaction_signer,
            &transaction_digest,
            epoch_id,
            epoch_timestamp_ms,
            rgp,
            gas_price,
            gas_data.budget,
            sponsor,
            protocol_config,
        );
        let tx_ctx = Rc::new(RefCell::new(tx_ctx));

        let enable_pcool = protocol_config.enable_pcool_flow();
        let run_authenticator =
            |temporary_store: &mut TemporaryStore<'_>,
             gas_charger: &mut GasCharger,
             trace_builder_opt: &mut Option<MoveTraceBuilder>,
             authenticator: MoveAuthenticator,
             authenticator_function_ref: AuthenticatorFunctionRef,
             authenticator_input_objects: CheckedInputObjects| {
                match authenticator_function_ref {
                    AuthenticatorFunctionRef::V1(authenticator_function_ref_v1) => {
                        authenticate_transaction_inner(
                            store,
                            temporary_store,
                            protocol_config,
                            metrics.clone(),
                            gas_charger,
                            authenticator,
                            authenticator_function_ref_v1,
                            &authenticator_input_objects.into_inner(),
                            transaction_kind.clone(),
                            transaction_digest,
                            auth_context_data.clone(),
                            tx_ctx.clone(),
                            trace_builder_opt,
                            move_vm,
                        )
                    }
                }
            };

        // A cancelled transaction fails with its cancellation: its authenticators are not run,
        // and a failure to resolve them is not reported. The cancellation is reported as is, not
        // as an authentication failure, and only once: the transaction execution gets no
        // cancelled objects to check.
        let cancellation_check =
            match input_objects_error(protocol_config, false, cancelled_objects) {
                Some(kind) => Err(ExecutionError::from_kind(kind)),
                None => Ok(()),
            };

        // Next to the result, whether the gas owner authorized the transaction. An owner that
        // signed it did, as its signature is checked before consensus. A Move account did only if
        // its authenticator ran and succeeded.
        //
        // Under the P-COOL flow the gas owner's authenticator runs first, and also after
        // another account failed to resolve, so whether the gas owner authorized does not depend
        // on the order of the signatures. When it fails, its failure is the one reported.
        //
        // Otherwise a resolution failure skips the authenticators entirely, so the objects holding
        // their function references are neither recorded as loaded nor charged for.
        let (authentication_execution_result, gas_owner_authorized) = match cancellation_check {
            // No authenticator ran, so a gas owner that is a Move account did not authorize the
            // transaction.
            Err(error) => {
                let gas_owner_authorized = match &authenticators {
                    MoveAuthenticatorsForExecution::ResolutionFailed { accounts, .. } => {
                        !accounts.contains(&gas_owner)
                    }
                    MoveAuthenticatorsForExecution::Resolved(authenticators) => !authenticators
                        .iter()
                        .any(|authenticator| authenticator.authenticator.address() == gas_owner),
                };
                (Err(error), gas_owner_authorized)
            }
            Ok(()) => {
                let (authentication_result, gas_owner_authorized) = match authenticators {
                    MoveAuthenticatorsForExecution::ResolutionFailed {
                        error,
                        accounts,
                        gas_owner: gas_owner_authenticator,
                    } => {
                        if !accounts.contains(&gas_owner) {
                            (Err(error), true)
                        } else {
                            match gas_owner_authenticator
                                .filter(|_| enable_pcool)
                                .map(|gas_owner_authenticator| *gas_owner_authenticator)
                            {
                                Some(MoveAuthenticatorForExecution {
                                    authenticator,
                                    function_ref,
                                    input_objects: authenticator_input_objects,
                                }) => {
                                    let AuthenticatorFunctionRefForExecution {
                                        authenticator_function_ref,
                                        loaded_object_id,
                                        loaded_object_metadata,
                                    } = function_ref;
                                    temporary_store.save_loaded_runtime_objects(BTreeMap::from([
                                        (loaded_object_id, loaded_object_metadata),
                                    ]));
                                    match run_authenticator(
                                        &mut temporary_store,
                                        &mut gas_charger,
                                        trace_builder_opt,
                                        authenticator,
                                        authenticator_function_ref,
                                        authenticator_input_objects,
                                    ) {
                                        Ok(()) => (Err(error), true),
                                        Err(gas_owner_error) => (Err(gas_owner_error), false),
                                    }
                                }
                                None => (Err(error), false),
                            }
                        }
                    }
                    MoveAuthenticatorsForExecution::Resolved(authenticators) => {
                        // Store the loaded object metadata (the field object holding each
                        // `AuthenticatorFunctionRef`) in the temporary store before the
                        // authenticators run.
                        let mut authenticators = authenticators
                            .into_iter()
                            .map(
                                |MoveAuthenticatorForExecution {
                                     authenticator,
                                     function_ref,
                                     input_objects: authenticator_input_objects,
                                 }| {
                                    let AuthenticatorFunctionRefForExecution {
                                        authenticator_function_ref,
                                        loaded_object_id,
                                        loaded_object_metadata,
                                    } = function_ref;
                                    temporary_store.save_loaded_runtime_objects(BTreeMap::from([
                                        (loaded_object_id, loaded_object_metadata),
                                    ]));
                                    (
                                        authenticator,
                                        authenticator_function_ref,
                                        authenticator_input_objects,
                                    )
                                },
                            )
                            .collect::<Vec<_>>();

                        if enable_pcool
                            && let Some(position) =
                                authenticators.iter().position(|(authenticator, ..)| {
                                    authenticator.address() == gas_owner
                                })
                        {
                            let gas_owner_authenticator = authenticators.remove(position);
                            authenticators.insert(0, gas_owner_authenticator);
                        }

                        let mut gas_owner_authorized = !authenticators
                            .iter()
                            .any(|(authenticator, ..)| authenticator.address() == gas_owner);

                        // Run each authenticator in sequence; the first failure aborts the chain.
                        // Authentication changes no state apart from gas charging.
                        let result = authenticators.into_iter().try_for_each(
                            |(
                                authenticator,
                                authenticator_function_ref,
                                authenticator_input_objects,
                            )| {
                                let account = authenticator.address();
                                let result = run_authenticator(
                                    &mut temporary_store,
                                    &mut gas_charger,
                                    trace_builder_opt,
                                    authenticator,
                                    authenticator_function_ref,
                                    authenticator_input_objects,
                                );
                                if result.is_ok() && account == gas_owner {
                                    gas_owner_authorized = true;
                                }
                                result
                            },
                        );
                        (result, gas_owner_authorized)
                    }
                };

                // A resolution failure goes through the same reporting as an authenticator that
                // fails, so a client sees `MoveAuthentication` for both.
                (
                    report_authentication_error(authentication_result, protocol_config),
                    gas_owner_authorized,
                )
            }
        };

        // The checks every transaction runs on its inputs, except the cancellation, which was
        // checked above.
        if let Some(kind) = input_objects_error(protocol_config, contains_deleted_input, None) {
            execution_params.push_early_error(kind);
        }

        // The body's own checks may replace this error in the effects, so its kind is reported
        // alongside.
        let authentication_error = authentication_execution_result
            .as_ref()
            .err()
            .map(|error| error.kind().clone());

        // Under the P-COOL flow, the gas owner's authenticator runs for the first time here, so a
        // transaction it did not authorize can reach execution. Nobody pays for it, unless
        // `gas_owner_pays` answers `true`.
        let authentication_execution_result = match authentication_execution_result {
            Err(error)
                if !gas_owner_authorized
                    && enable_pcool
                    && !charge_gas_owner(protocol_config, &error, gas_owner_pays) =>
            {
                let output = execute_without_gas_charge_to_effects::<Mode>(
                    temporary_store,
                    gas_charger,
                    shared_object_refs,
                    transaction_dependencies,
                    transaction_digest,
                    epoch_id,
                    execution_params,
                    error,
                );
                return (output, authentication_error);
            }
            // Also a gas owner that `gas_owner_pays` charges: it pays like any failed transaction
            // its gas owner authorized.
            result => result,
        };

        let output = execute_transaction_to_effects_inner::<Mode>(
            store,
            temporary_store,
            gas_charger,
            tx_ctx,
            shared_object_refs,
            transaction_dependencies,
            transaction_kind,
            transaction_signer,
            sponsor,
            transaction_digest,
            move_vm,
            epoch_id,
            protocol_config,
            metrics,
            enable_expensive_checks,
            execution_params,
            trace_builder_opt,
            Some(authentication_execution_result),
        );
        (output, authentication_error)
    }

    /// Whether a gas owner that is a Move account and did not authorize the transaction pays for
    /// its failed authentication, as `gas_owner_pays` decides under the protocol feature flag
    /// `enable_validator_attestation`. `gas_owner_pays` is not asked about a cancellation, decided
    /// at the consensus commit before any authenticator ran, nor about an invariant violation, an
    /// internal error: such a gas owner is not charged.
    fn charge_gas_owner(
        protocol_config: &ProtocolConfig,
        error: &ExecutionError,
        gas_owner_pays: Option<&dyn Fn(&ExecutionErrorKind) -> bool>,
    ) -> bool {
        let kind = unwrap_move_authentication_error(error.kind());

        protocol_config.enable_validator_attestation()
            && !is_cancellation(kind)
            && !is_any_invariant_violation(kind)
            && gas_owner_pays.is_some_and(|pays| pays(kind))
    }

    /// Produces the effects of a transaction whose gas owner is a Move account that did not
    /// authorize it. Nobody authorized paying for gas, so nobody is charged: owned inputs, gas
    /// coins included, and mutable shared inputs move to their next versions with their contents,
    /// owners and storage rebates unchanged. The effects name the first gas coin as the gas
    /// object, with a zero gas cost.
    ///
    /// An early error (denied certificate, deleted input) is reported instead of the
    /// authentication error, as when gas is charged.
    fn execute_without_gas_charge_to_effects<Mode: ExecutionMode>(
        mut temporary_store: TemporaryStore<'_>,
        mut gas_charger: GasCharger,
        shared_object_refs: Vec<SharedInput>,
        mut transaction_dependencies: BTreeSet<TransactionDigest>,
        transaction_digest: TransactionDigest,
        epoch_id: &EpochId,
        execution_params: ExecutionOrEarlyError,
        authentication_error: ExecutionError,
    ) -> ExecutionOutput<Mode> {
        let error: Mode::Error = match execution_params.into_early_errors() {
            Some(early_errors) => Mode::Error::from_kind(early_errors.head),
            None => authentication_error.into(),
        };
        let status = failure_status(&error);

        temporary_store.drop_writes();
        temporary_store.ensure_active_inputs_mutated();

        // Genesis writes a special digest to indicate that an object was created during
        // genesis and not written by any normal transaction - remove that from the
        // dependencies
        transaction_dependencies.remove(&TransactionDigest::genesis_marker());

        gas_charger.set_gas_coin_without_smashing();
        let gas_coin = gas_charger.gas_coin();
        let (inner, effects) = temporary_store.into_effects(
            shared_object_refs,
            &transaction_digest,
            transaction_dependencies,
            GasCostSummary::default(),
            status,
            gas_coin,
            *epoch_id,
        );

        ExecutionOutput {
            inner_store: inner,
            gas_status: gas_charger.into_gas_status(),
            effects,
            timings: vec![],
            execution_result: Err(error),
        }
    }

    /// Checks the authentication of a transaction without producing effects: runs the
    /// authenticate function of each authenticator and returns the first error.
    #[instrument(name = "tx_validate", level = "debug", skip_all)]
    pub fn authenticate_transaction(
        store: &dyn BackingStore,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        // Epoch
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        // Gas related
        gas_data: GasPayment,
        gas_status: IotaGasStatus,
        // Authentication
        authenticators: Vec<(
            MoveAuthenticator,
            AuthenticatorFunctionRef,
            CheckedInputObjects,
        )>,
        aggregated_authenticator_input_objects: CheckedInputObjects,
        // Transaction
        transaction_kind: TransactionKind,
        transaction_signer: Address,
        transaction_digest: TransactionDigest,
        auth_context_data: AuthContextData,
        // Tracing
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        // VM
        move_vm: &Arc<MoveRuntime>,
    ) -> (IotaGasStatus, Result<(), ExecutionError>) {
        let sponsor = resolve_sponsor(&gas_data, &transaction_signer);
        let gas_price = gas_status.gas_price();
        let rgp = gas_status.reference_gas_price();
        let mut gas_charger =
            GasCharger::new(transaction_digest, vec![], gas_status, protocol_config);

        let tx_ctx = TxContext::new_from_components(
            &transaction_signer,
            &transaction_digest,
            epoch_id,
            epoch_timestamp_ms,
            rgp,
            gas_price,
            gas_data.budget,
            sponsor,
            protocol_config,
        );
        let tx_ctx = Rc::new(RefCell::new(tx_ctx));

        let mut temporary_store = TemporaryStore::new(
            store,
            aggregated_authenticator_input_objects.into_inner(),
            vec![],
            transaction_digest,
            protocol_config,
            *epoch_id,
            &transaction_kind,
        );

        // Run each authenticator in sequence; return on first failure.
        let authentication_execution_result = authenticators.into_iter().try_for_each(
            |(authenticator, authenticator_function_ref, authenticator_input_objects)| {
                match authenticator_function_ref {
                    AuthenticatorFunctionRef::V1(authenticator_function_ref_v1) => {
                        authenticate_transaction_inner(
                            store,
                            &mut temporary_store,
                            protocol_config,
                            metrics.clone(),
                            &mut gas_charger,
                            authenticator,
                            authenticator_function_ref_v1,
                            &authenticator_input_objects.into_inner(),
                            transaction_kind.clone(),
                            transaction_digest,
                            auth_context_data.clone(),
                            tx_ctx.clone(),
                            trace_builder_opt,
                            move_vm,
                        )
                    }
                }
            },
        );

        let result = report_authentication_error(authentication_execution_result, protocol_config);
        (gas_charger.into_gas_status(), result)
    }

    /// Runs the authenticate function named by `authenticator_function_ref` with the inputs of
    /// `authenticator`, as a programmable transaction with a single Move call that also receives
    /// the `AuthContext`.
    fn authenticate_transaction_inner(
        store: &dyn BackingStore,
        temporary_store: &mut TemporaryStore<'_>,
        // Configuration
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        // Gas related
        gas_charger: &mut GasCharger,
        // Authenticator
        authenticator: MoveAuthenticator,
        authenticator_function_ref: AuthenticatorFunctionRefV1,
        authenticator_input_objects: &InputObjects,
        // Transaction
        transaction_kind: TransactionKind,
        transaction_digest: TransactionDigest,
        auth_context_data: AuthContextData,
        tx_ctx: Rc<RefCell<TxContext>>,
        // Tracing
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        // VM
        move_vm: &Arc<MoveRuntime>,
    ) -> Result<(), ExecutionError> {
        let TransactionKind::Programmable(ptb) = &transaction_kind else {
            invariant_violation!("Only programmable transactions can be authenticated");
        };
        debug_assert!(
            authenticator_input_objects.mutable_inputs().is_empty(),
            "No mutable inputs are allowed"
        );
        debug_assert!(
            authenticator.receiving_objects().is_empty(),
            "No receiving inputs are allowed"
        );

        let contains_deleted_input = authenticator_input_objects.contains_deleted_objects();
        let cancelled_objects = authenticator_input_objects.get_cancelled_objects();

        let auth_ctx = AuthContext::new_from_components(
            authenticator.digest().into(),
            auth_context_data.sender_auth_digest,
            auth_context_data.sponsor_auth_digest,
            auth_context_data
                .sender_authenticator_function_ref
                .and_then(Into::into),
            auth_context_data
                .sponsor_authenticator_function_ref
                .and_then(Into::into),
            ptb,
            auth_context_data.transaction_data_bytes,
        );
        // The executor reads it from the temporary store when it supplies the `AuthContext`
        // argument and the authentication natives.
        temporary_store.store_auth_context(Rc::new(RefCell::new(auth_ctx)));

        let authentication_execution_result = execute_authenticator_move_call(
            store,
            temporary_store,
            authenticator,
            authenticator_function_ref,
            gas_charger,
            tx_ctx,
            move_vm,
            protocol_config,
            metrics,
            contains_deleted_input,
            cancelled_objects,
            trace_builder_opt,
        );

        let authentication_execution_status = match &authentication_execution_result {
            Err(error) => failure_status(error),
            Ok(()) => ExecutionStatus::Success,
        };

        #[skip_checked_arithmetic]
        trace!(
            tx_digest = ?transaction_digest,
            computation_gas_cost = gas_charger.summary().gas_used(),
            "Finished authenticator execution of transaction with status {:?}",
            authentication_execution_status
        );

        authentication_execution_result
    }

    /// Executes the authenticate Move call. Gas is charged only for executing the call: reading
    /// the authenticator input objects is charged later, with the transaction.
    fn execute_authenticator_move_call(
        store: &dyn BackingStore,
        temporary_store: &mut TemporaryStore<'_>,
        authenticator: MoveAuthenticator,
        authenticator_function_ref: AuthenticatorFunctionRefV1,
        gas_charger: &mut GasCharger,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        contains_deleted_input: bool,
        cancelled_objects: Option<(CancelledObjects, Version)>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        if let Some(kind) =
            input_objects_error(protocol_config, contains_deleted_input, cancelled_objects)
        {
            return Err(ExecutionError::from_kind(kind));
        }
        let authenticator_move_call =
            setup_authenticator_move_call(authenticator, authenticator_function_ref)?;
        SPT::execute::<execution_mode::Authentication>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            store,
            tx_ctx,
            gas_charger,
            authenticator_move_call,
            trace_builder_opt,
        )
        .map_err(|(e, _)| e)?;
        temporary_store.check_move_authenticator_results_consistency()
    }

    /// When enabled by the protocol config, reports a failure of the Move authentication as
    /// [`ExecutionErrorKind::MoveAuthentication`], dropping the authenticator's command index so
    /// it is not attributed to a command of the programmable transaction.
    fn report_authentication_error<T>(
        authentication_execution_result: Result<T, ExecutionError>,
        protocol_config: &ProtocolConfig,
    ) -> Result<T, ExecutionError> {
        if protocol_config.report_move_authentication_error() {
            authentication_execution_result.map_err(ExecutionError::into_move_authentication_error)
        } else {
            authentication_execution_result
        }
    }

    /// Builds a programmable transaction with a single Move call to the authenticate function.
    /// `MoveAuthenticator::object_to_authenticate` is the first argument, followed by
    /// `MoveAuthenticator::call_args`.
    fn setup_authenticator_move_call(
        authenticator: MoveAuthenticator,
        authenticator_function_ref: AuthenticatorFunctionRefV1,
    ) -> Result<ProgrammableTransaction, ExecutionError> {
        let mut builder = ProgrammableTransactionBuilder::new();

        let mut args = vec![authenticator.object_to_authenticate().to_owned()];
        args.extend(authenticator.call_args().to_owned());

        let (Ok(module), Ok(function)) = (
            Identifier::new(authenticator_function_ref.module),
            Identifier::new(authenticator_function_ref.function),
        ) else {
            invariant_violation!("`AuthenticatorFunctionRefV1` names are valid identifiers");
        };
        let res = builder.move_call(
            authenticator_function_ref.package,
            module,
            function,
            authenticator.type_args().to_vec(),
            args,
        );

        assert_invariant!(
            res.is_ok(),
            "Unable to generate an account authenticator call transaction!"
        );

        Ok(builder.finish())
    }

    fn resolve_sponsor(gas_data: &GasPayment, transaction_signer: &Address) -> Option<Address> {
        let gas_owner = gas_data.owner;
        if &gas_owner == transaction_signer {
            None
        } else {
            Some(gas_owner)
        }
    }

    pub fn execute_genesis_state_update(
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        move_vm: &Arc<MoveRuntime>,
        tx_context: Rc<RefCell<TxContext>>,
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore, ExecutionError> {
        let mut temporary_store = TemporaryStore::new_for_genesis_state_update(
            store,
            input_objects.into_inner(),
            tx_context.borrow().digest(),
            protocol_config,
        );
        let mut gas_charger = GasCharger::new_unmetered(tx_context.borrow().digest());
        SPT::execute::<execution_mode::Genesis>(
            protocol_config,
            metrics,
            move_vm,
            &mut temporary_store,
            store,
            tx_context,
            &mut gas_charger,
            pt,
            &mut None,
        )
        .map_err(|(e, _)| e)?;
        temporary_store.update_object_version_and_prev_tx();
        Ok(temporary_store.into_inner())
    }

    #[instrument(name = "tx_execute", level = "debug", skip_all)]
    fn execute_transaction<Mode: ExecutionMode>(
        store: &dyn BackingStore,
        temporary_store: &mut TemporaryStore<'_>,
        transaction_kind: TransactionKind,
        gas_charger: &mut GasCharger,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        execution_params: ExecutionOrEarlyError,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        pre_execution_result: Option<Result<(), ExecutionError>>,
    ) -> ExecutionOutcome<Mode> {
        gas_charger.smash_gas(temporary_store);

        // At this point, either no charges have been applied yet or the authenticators already
        // ran.
        debug_assert!(
            pre_execution_result.is_some() || gas_charger.no_charges(),
            "No gas charges must be applied yet"
        );
        // We must charge object read here during transaction execution, because if this fails
        // we must still ensure an effect is committed and all objects versions incremented
        let result = gas_charger.charge_input_objects(temporary_store);

        let result: ResultWithTimings<Mode::ExecutionResults, Mode::Error> =
            result.map_err(|e| (e.into(), vec![])).and_then(
                |()| -> ResultWithTimings<Mode::ExecutionResults, Mode::Error> {
                    let mut execution_result: ResultWithTimings<
                        Mode::ExecutionResults,
                        Mode::Error,
                    > = match execution_params.into_early_errors() {
                        Some(early_execution_errors) => {
                            Err((Mode::Error::from_kind(early_execution_errors.head), vec![]))
                        }
                        // A failed authentication fails the transaction after the early errors.
                        None => match pre_execution_result {
                            Some(Err(error)) => Err((error.into(), vec![])),
                            Some(Ok(())) | None => execution_loop::<Mode>(
                                store,
                                temporary_store,
                                transaction_kind,
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics.clone(),
                                trace_builder_opt,
                            ),
                        },
                    };

                    let meter_check = check_meter_limit::<Mode>(
                        temporary_store,
                        gas_charger,
                        protocol_config,
                        metrics.clone(),
                    );
                    if let Err(e) = meter_check {
                        execution_result = Err((e, vec![]));
                    }

                    if execution_result.is_ok() {
                        let gas_check = check_written_objects_limit::<Mode>(
                            temporary_store,
                            gas_charger,
                            protocol_config,
                            metrics,
                        );
                        if let Err(e) = gas_check {
                            execution_result = Err((e, vec![]));
                        }
                    }

                    execution_result
                },
            );

        let (mut result, timings) = match result {
            Ok((r, t)) => (Ok(r), t),
            Err((e, t)) => (Err(e), t),
        };
        let cost_summary = gas_charger.charge_gas(temporary_store, &mut result);
        // For advance epoch transaction, we need to provide epoch rewards and rebates as extra
        // information provided to check_iota_conserved, because we mint rewards, and burn
        // the rebates. We also need to pass in the unmetered_storage_rebate because storage
        // rebate is not reflected in the storage_rebate of gas summary. This is a bit
        // confusing. We could probably clean up the code a bit.
        // Put all the storage rebate accumulated in the system transaction
        // to the 0x5 object so that it's not lost.
        temporary_store.conserve_unmetered_storage_rebate(gas_charger.unmetered_storage_rebate());

        ExecutionOutcome {
            cost_summary,
            execution_result: result,
            timings,
        }
    }

    /// Run all post-execution system-invariant checks against the finalized (gas-charged)
    /// store.
    ///
    /// Two families, with deliberately different failure handling:
    /// - IOTA conservation / balance-accumulator authorization, via [`run_conservation_checks`]. A
    ///   violation is recoverable: the tx is aborted (and conserves IOTA) rather than panicking.
    /// - Object-ownership authentication (expensive-checks only, skipped under dev-inspect). This
    ///   is a non-recoverable assertion, so it runs *after* conservation and *outside* its
    ///   gas-charging recovery, and panics on violation. (Folding it into the recovery would let
    ///   the recovery's `drop_writes` mask a real violation into a silent abort.)
    ///
    /// Returns the conservation result so the caller can fail the transaction on a violation;
    /// an ownership violation panics directly.
    #[allow(clippy::too_many_arguments)]
    fn run_invariant_checks<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        tx_digest: TransactionDigest,
        move_vm: &Arc<MoveRuntime>,
        enable_expensive_checks: bool,
        cost_summary: &GasCostSummary,
        sender: &Address,
        sponsor: &Option<Address>,
        is_epoch_change: bool,
    ) -> Result<(), Mode::Error> {
        let conservation = run_conservation_checks::<Mode>(
            temporary_store,
            gas_charger,
            tx_digest,
            move_vm,
            enable_expensive_checks,
            cost_summary,
        );
        if enable_expensive_checks && !Mode::allow_arbitrary_function_calls() {
            temporary_store
                .check_ownership_invariants(sender, sponsor, gas_charger, is_epoch_change)
                .unwrap()
        } // else, in dev inspect mode and anything goes--don't check

        conservation
    }

    /// Run the IOTA-conservation and balance-accumulator invariant checks
    /// ([`TemporaryStore::check_conservation_invariants`]) against the finalized store. On a
    /// violation, recover by dumping all writes, charging gas in
    /// the aborted state, and re-checking; a surviving double failure means gas charging itself
    /// mints or burns IOTA, which is unrecoverable, so we panic. The checks themselves are
    /// read-only; the recovery's gas-charging mutations are orchestrated here alongside
    /// the main-path charge.
    #[instrument(name = "run_conservation_checks", level = "debug", skip_all)]
    fn run_conservation_checks<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        tx_digest: TransactionDigest,
        move_vm: &Arc<MoveRuntime>,
        enable_expensive_checks: bool,
        cost_summary: &GasCostSummary,
    ) -> Result<(), Mode::Error> {
        let Err(conservation_err) = temporary_store.check_conservation_invariants::<Mode>(
            move_vm,
            enable_expensive_checks,
            cost_summary,
        ) else {
            return Ok(());
        };

        // Conservation violated. Try to avoid a panic by dumping all writes, charging for gas
        // in the aborted state, and re-checking; surface an aborted transaction
        // with the invariant violation if that works.
        let mut result: Result<(), Mode::Error> = Err(conservation_err.into());
        gas_charger.reset(temporary_store);
        gas_charger.charge_gas(temporary_store, &mut result);
        if let Err(recovery_err) = temporary_store.check_conservation_invariants::<Mode>(
            move_vm,
            enable_expensive_checks,
            cost_summary,
        ) {
            // If we still fail, it's a problem with gas charging that happens even in the
            // "aborted" case - no other option but panic. We would create or destroy IOTA
            // otherwise (or admit an unauthorized accumulator Split).
            panic!(
                "IOTA conservation fail in tx block {}: {}\nGas status is {}\nTx was ",
                tx_digest,
                recovery_err,
                gas_charger.summary()
            )
        }
        result
    }

    #[instrument(name = "check_meter_limit", level = "debug", skip_all)]
    fn check_meter_limit<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), Mode::Error> {
        let effects_estimated_size = temporary_store.estimate_effects_size_upperbound();

        // Check if a limit threshold was crossed.
        // For metered transactions, there is not soft limit.
        // For system transactions, we allow a soft limit with alerting, and a hard limit where we
        // terminate
        match check_limit_by_meter!(
            !gas_charger.is_unmetered(),
            effects_estimated_size,
            protocol_config.max_serialized_tx_effects_size_bytes(),
            protocol_config.max_serialized_tx_effects_size_bytes_system_tx(),
            metrics.excessive_estimated_effects_size
        ) {
            LimitThresholdCrossed::None => Ok(()),
            LimitThresholdCrossed::Soft(_, limit) => {
                warn!(
                    effects_estimated_size = effects_estimated_size,
                    soft_limit = limit,
                    "Estimated transaction effects size crossed soft limit",
                );
                Ok(())
            }
            LimitThresholdCrossed::Hard(_, lim) => Err(Mode::Error::new_with_source(
                ExecutionErrorKind::EffectsTooLarge {
                    current_size: effects_estimated_size as u64,
                    max_size: lim as u64,
                },
                "Transaction effects are too large",
            )),
        }
    }

    #[instrument(name = "check_written_objects_limit", level = "debug", skip_all)]
    fn check_written_objects_limit<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), Mode::Error> {
        if let (Some(normal_lim), Some(system_lim)) = (
            protocol_config.max_size_written_objects_as_option(),
            protocol_config.max_size_written_objects_system_tx_as_option(),
        ) {
            let written_objects_size = temporary_store.written_objects_size();

            match check_limit_by_meter!(
                !gas_charger.is_unmetered(),
                written_objects_size,
                normal_lim,
                system_lim,
                metrics.excessive_written_objects_size
            ) {
                LimitThresholdCrossed::None => (),
                LimitThresholdCrossed::Soft(_, limit) => {
                    warn!(
                        written_objects_size = written_objects_size,
                        soft_limit = limit,
                        "Written objects size crossed soft limit",
                    )
                }
                LimitThresholdCrossed::Hard(_, lim) => {
                    return Err(Mode::Error::new_with_source(
                        ExecutionErrorKind::WrittenObjectsTooLarge {
                            object_size: written_objects_size as u64,
                            max_object_size: lim as u64,
                        },
                        "Written objects size crossed hard limit",
                    ));
                }
            };
        }

        Ok(())
    }

    #[instrument(level = "debug", skip_all)]
    fn execution_loop<Mode: ExecutionMode>(
        store: &dyn BackingStore,
        temporary_store: &mut TemporaryStore<'_>,
        transaction_kind: TransactionKind,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> ResultWithTimings<Mode::ExecutionResults, Mode::Error> {
        let result = match transaction_kind {
            TransactionKind::Genesis(GenesisTransaction { objects, events }) => {
                if tx_ctx.borrow().epoch() != 0 {
                    panic!("BUG: Genesis Transactions can only be executed in epoch 0");
                }

                for genesis_object in objects {
                    let object = ObjectInner {
                        data: genesis_object.data,
                        owner: genesis_object.owner,
                        previous_transaction: tx_ctx.borrow().digest(),
                        storage_rebate: 0,
                    };
                    temporary_store.create_object(object.into());
                }

                temporary_store.record_execution_results(ExecutionResults::V1(
                    ExecutionResultsV1 {
                        user_events: events,
                        ..Default::default()
                    },
                ));

                Ok((Mode::empty_results(), vec![]))
            }
            TransactionKind::ConsensusCommitPrologueV1(prologue) => {
                setup_consensus_commit(
                    prologue.commit_timestamp_ms,
                    temporary_store,
                    store,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics,
                    trace_builder_opt,
                )
                .expect("ConsensusCommitPrologueV1 cannot fail");
                Ok((Mode::empty_results(), vec![]))
            }
            TransactionKind::Programmable(pt) => SPT::execute::<Mode>(
                protocol_config,
                metrics,
                move_vm,
                temporary_store,
                store,
                tx_ctx,
                gas_charger,
                pt,
                trace_builder_opt,
            ),
            TransactionKind::EndOfEpoch(txns) => {
                // Non-change-epoch kinds accumulate commands into the builder;
                // the change-epoch kind is always last and executes the built
                // transaction as part of advancing the epoch.
                let mut builder = ProgrammableTransactionBuilder::new();
                let len = txns.len();

                for (i, tx) in txns.into_iter().enumerate() {
                    match tx {
                        EndOfEpochTransactionKind::ChangeEpoch(change_epoch) => {
                            assert_eq!(i, len - 1);
                            advance_epoch_v1(
                                builder,
                                change_epoch,
                                temporary_store,
                                store,
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics,
                                trace_builder_opt,
                            )
                            .map_err(|e| (Mode::Error::from(e), vec![]))?;
                            return Ok((Mode::empty_results(), vec![]));
                        }
                        EndOfEpochTransactionKind::ChangeEpochV2(change_epoch_v2) => {
                            assert_eq!(i, len - 1);
                            advance_epoch_v2(
                                builder,
                                change_epoch_v2,
                                temporary_store,
                                store,
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics,
                                trace_builder_opt,
                            )
                            .map_err(|e| (Mode::Error::from(e), vec![]))?;
                            return Ok((Mode::empty_results(), vec![]));
                        }
                        EndOfEpochTransactionKind::ChangeEpochV3(change_epoch_v3) => {
                            assert_eq!(i, len - 1);
                            advance_epoch_v3(
                                builder,
                                change_epoch_v3,
                                temporary_store,
                                store,
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics,
                                trace_builder_opt,
                            )
                            .map_err(|e| (Mode::Error::from(e), vec![]))?;
                            return Ok((Mode::empty_results(), vec![]));
                        }
                        EndOfEpochTransactionKind::ChangeEpochV4(change_epoch_v4) => {
                            assert_eq!(i, len - 1);
                            advance_epoch_v4(
                                builder,
                                change_epoch_v4,
                                temporary_store,
                                store,
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics,
                                trace_builder_opt,
                            )
                            .map_err(|e| (Mode::Error::from(e), vec![]))?;
                            return Ok((Mode::empty_results(), vec![]));
                        }
                        EndOfEpochTransactionKind::TransactionDenyRulesCreate => {
                            assert!(
                                protocol_config.enable_pcool_flow(),
                                "unexpected TransactionDenyRulesCreate: on-chain deny rule governance is not enabled"
                            );
                            builder = setup_transaction_deny_rules_create(builder)
                                .map_err(|e| (Mode::Error::from(e), vec![]))?;
                        }
                        _ => unimplemented!(
                            "a new EndOfEpochTransactionKind enum variant was added and needs to be handled"
                        ),
                    }
                }
                unreachable!(
                    "EndOfEpochTransactionKind::ChangeEpoch should be the last transaction in the list"
                )
            }
            #[allow(deprecated)]
            TransactionKind::AuthenticatorStateUpdateV1Deprecated => {
                // Deprecated: Authenticator state (JWK) is deprecated and
                // was never enabled. These transaction kinds are retained
                // only for BCS enum variant compatibility.
                return Err((
                    Mode::Error::from(ExecutionError::new(
                        ExecutionErrorKind::VmInvariantViolation,
                        Some("AuthenticatorState transactions are deprecated and were never created on IOTA".into()),
                    )),
                    vec![],
                ));
            }
            TransactionKind::RandomnessStateUpdate(randomness_state_update) => {
                setup_randomness_state_update(
                    randomness_state_update,
                    temporary_store,
                    store,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics,
                    trace_builder_opt,
                )
                .map_err(|e| (Mode::Error::from(e), vec![]))?;
                Ok((Mode::empty_results(), vec![]))
            }
            TransactionKind::TransactionDenyRulesUpdate(update) => {
                assert!(
                    protocol_config.enable_pcool_flow(),
                    "unexpected TransactionDenyRulesUpdate: on-chain deny rule governance is not enabled"
                );
                setup_transaction_deny_rules_update(
                    update,
                    temporary_store,
                    store,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics,
                    trace_builder_opt,
                )
                .map_err(|e| (Mode::Error::from(e), vec![]))?;
                Ok((Mode::empty_results(), vec![]))
            }
            _ => unimplemented!(
                "a new TransactionKind enum variant was added and needs to be handled"
            ),
        }?;
        temporary_store
            .check_execution_results_consistency::<Mode>()
            .map_err(|e| (e, vec![]))?;
        Ok(result)
    }

    /// Mints epoch rewards by creating both storage and computation charges
    /// using a `ProgrammableTransactionBuilder`. The function takes in the
    /// `AdvanceEpochParams`, serializes the storage and computation
    /// charges, and invokes the reward creation function within the IOTA
    /// Prepares invocations for creating both storage and computation charges
    /// with a `ProgrammableTransactionBuilder` using the `AdvanceEpochParams`.
    /// The corresponding functions from the IOTA framework can be invoked later
    /// during execution of the programmable transaction.
    fn mint_epoch_rewards_in_pt(
        builder: &mut ProgrammableTransactionBuilder,
        params: &AdvanceEpochParams,
    ) -> (Argument, Argument) {
        // Create storage charges.
        let storage_charge_arg = builder.input(Input::pure(&params.storage_charge)).unwrap();
        let storage_charges = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            iota_sdk_types::Identifier::BALANCE_MODULE,
            BALANCE_CREATE_REWARDS_FUNCTION_NAME,
            vec![iota_sdk_types::TypeTag::from(
                iota_sdk_types::StructTag::new_gas(),
            )],
            vec![storage_charge_arg],
        );

        // Create computation charges.
        let computation_charge_arg = builder
            .input(Input::pure(&params.computation_charge))
            .unwrap();
        let computation_charges = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            iota_sdk_types::Identifier::BALANCE_MODULE,
            BALANCE_CREATE_REWARDS_FUNCTION_NAME,
            vec![iota_sdk_types::TypeTag::from(
                iota_sdk_types::StructTag::new_gas(),
            )],
            vec![computation_charge_arg],
        );
        (storage_charges, computation_charges)
    }

    /// Constructs a `ProgrammableTransaction` to advance the epoch. It creates
    /// storage charges and computation charges by invoking
    /// `mint_epoch_rewards_in_pt`, advances the epoch by setting up the
    /// necessary arguments, such as epoch number, protocol version, storage
    /// rebate, and slashing rate, and executing the `advance_epoch` function
    /// within the IOTA system. Then, it destroys the storage rebates to
    /// complete the transaction.
    pub fn construct_advance_epoch_pt_impl(
        mut builder: ProgrammableTransactionBuilder,
        params: &AdvanceEpochParams,
        call_arg_vec: Vec<Input>,
    ) -> Result<ProgrammableTransaction, ExecutionError> {
        // Create storage and computation charges and add them as arguments.
        let (storage_charges, computation_charges) = mint_epoch_rewards_in_pt(&mut builder, params);
        let mut arguments = vec![
            builder
                .pure(params.validator_subsidy)
                .expect("bcs encoding a u64 should not fail"),
            storage_charges,
            computation_charges,
        ];

        let call_arg_arguments = call_arg_vec
            .into_iter()
            .map(|a| builder.input(a))
            .collect::<Result<_, _>>();

        assert_invariant!(
            call_arg_arguments.is_ok(),
            "Unable to generate args for advance_epoch transaction!"
        );

        arguments.append(&mut call_arg_arguments.unwrap());

        info!("Call arguments to advance_epoch transaction: {:?}", params);

        let storage_rebates = builder.programmable_move_call(
            ObjectId::SYSTEM,
            iota_sdk_types::Identifier::IOTA_SYSTEM_MODULE,
            ADVANCE_EPOCH_FUNCTION_NAME,
            vec![],
            arguments,
        );

        // Step 3: Destroy the storage rebates.
        builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            iota_sdk_types::Identifier::BALANCE_MODULE,
            BALANCE_DESTROY_REBATES_FUNCTION_NAME,
            vec![iota_sdk_types::TypeTag::from(
                iota_sdk_types::StructTag::new_gas(),
            )],
            vec![storage_rebates],
        );
        Ok(builder.finish())
    }

    pub fn construct_advance_epoch_pt_v1(
        builder: ProgrammableTransactionBuilder,
        params: &AdvanceEpochParams,
    ) -> Result<ProgrammableTransaction, ExecutionError> {
        // the first three arguments to the advance_epoch function, namely
        // validator_subsidy, storage_charges and computation_charges, are
        // common to both v1 and v2 and are added in `construct_advance_epoch_pt_impl`.
        // The remaining arguments are added here.
        let call_arg_vec = vec![
            Input::IOTA_SYSTEM_MUTABLE, // wrapper: &mut IotaSystemState
            Input::pure(&params.epoch), // new_epoch: u64
            Input::pure(&params.next_protocol_version.as_u64()), // next_protocol_version: u64
            Input::pure(&params.storage_rebate), // storage_rebate: u64
            Input::pure(&params.non_refundable_storage_fee), // non_refundable_storage_fee: u64
            Input::pure(&params.reward_slashing_rate), // reward_slashing_rate: u64
            Input::pure(&params.epoch_start_timestamp_ms), // epoch_start_timestamp_ms: u64
        ];
        construct_advance_epoch_pt_impl(builder, params, call_arg_vec)
    }

    pub fn construct_advance_epoch_pt_v2(
        builder: ProgrammableTransactionBuilder,
        params: &AdvanceEpochParams,
    ) -> Result<ProgrammableTransaction, ExecutionError> {
        // the first three arguments to the advance_epoch function, namely
        // validator_subsidy, storage_charges and computation_charges, are
        // common to both v1 and v2 and are added in `construct_advance_epoch_pt_impl`.
        // The remaining arguments are added here.
        let call_arg_vec = vec![
            Input::pure(&params.computation_charge_burned), // computation_charge_burned: u64
            Input::IOTA_SYSTEM_MUTABLE,                     // wrapper: &mut IotaSystemState
            Input::pure(&params.epoch),                     // new_epoch: u64
            Input::pure(&params.next_protocol_version.as_u64()), // next_protocol_version: u64
            Input::pure(&params.storage_rebate),            // storage_rebate: u64
            Input::pure(&params.non_refundable_storage_fee), // non_refundable_storage_fee: u64
            Input::pure(&params.reward_slashing_rate),      // reward_slashing_rate: u64
            Input::pure(&params.epoch_start_timestamp_ms),  // epoch_start_timestamp_ms: u64
            Input::pure(&params.max_committee_members_count), // max_committee_members_count: u64
        ];
        construct_advance_epoch_pt_impl(builder, params, call_arg_vec)
    }

    pub fn construct_advance_epoch_pt_v3(
        builder: ProgrammableTransactionBuilder,
        params: &AdvanceEpochParams,
    ) -> Result<ProgrammableTransaction, ExecutionError> {
        // the first three arguments to the advance_epoch function, namely
        // validator_subsidy, storage_charges and computation_charges, are
        // common to both v1, v2 and v3 and are added in
        // `construct_advance_epoch_pt_impl`. The remaining arguments are added
        // here.
        let call_arg_vec = vec![
            Input::pure(&params.computation_charge_burned), // computation_charge_burned: u64
            Input::IOTA_SYSTEM_MUTABLE,                     // wrapper: &mut IotaSystemState
            Input::pure(&params.epoch),                     // new_epoch: u64
            Input::pure(&params.next_protocol_version.as_u64()), // next_protocol_version: u64
            Input::pure(&params.storage_rebate),            // storage_rebate: u64
            Input::pure(&params.non_refundable_storage_fee), // non_refundable_storage_fee: u64
            Input::pure(&params.reward_slashing_rate),      // reward_slashing_rate: u64
            Input::pure(&params.epoch_start_timestamp_ms),  // epoch_start_timestamp_ms: u64
            Input::pure(&params.max_committee_members_count), // max_committee_members_count: u64
            Input::pure(&params.eligible_active_validators), /* eligible_active_validators:
                                                             * Vec<u64> */
        ];
        construct_advance_epoch_pt_impl(builder, params, call_arg_vec)
    }

    pub fn construct_advance_epoch_pt_v4(
        builder: ProgrammableTransactionBuilder,
        params: &AdvanceEpochParams,
    ) -> Result<ProgrammableTransaction, ExecutionError> {
        // the first three arguments to the advance_epoch function, namely
        // validator_subsidy, storage_charges and computation_charges, are
        // common to both v1, v2, v3 and v4 and are added in
        // `construct_advance_epoch_pt_impl`. The remaining arguments are added
        // here.
        let call_arg_vec = vec![
            Input::pure(&params.computation_charge_burned), // computation_charge_burned: u64
            Input::IOTA_SYSTEM_MUTABLE,                     // wrapper: &mut IotaSystemState
            Input::pure(&params.epoch),                     // new_epoch: u64
            Input::pure(&params.next_protocol_version.as_u64()), // next_protocol_version: u64
            Input::pure(&params.storage_rebate),            // storage_rebate: u64
            Input::pure(&params.non_refundable_storage_fee), // non_refundable_storage_fee: u64
            Input::pure(&params.reward_slashing_rate),      // reward_slashing_rate: u64
            Input::pure(&params.epoch_start_timestamp_ms),  // epoch_start_timestamp_ms: u64
            Input::pure(&params.max_committee_members_count), // max_committee_members_count: u64
            Input::pure(&params.eligible_active_validators), /* eligible_active_validators:
                                                             * Vec<u64> */
            Input::pure(&params.scores), // scores: Vec<u64>
            Input::pure(&params.adjust_rewards_by_score), // adjust_rewards_by_score: bool
        ];
        construct_advance_epoch_pt_impl(builder, params, call_arg_vec)
    }

    /// Advances the epoch by executing a `ProgrammableTransaction`. If the
    /// transaction fails, it switches to safe mode and retries the epoch
    /// advancement in a more controlled environment. The function also
    /// handles the publication and upgrade of system packages for the new
    /// epoch. If any system package is added or upgraded, it ensures the
    /// proper execution and storage of the changes.
    fn advance_epoch_impl(
        advance_epoch_pt: ProgrammableTransaction,
        params: AdvanceEpochParams,
        system_packages: Vec<SystemPackage>,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let result = SPT::execute::<execution_mode::System<ExecutionError>>(
            protocol_config,
            metrics.clone(),
            move_vm,
            temporary_store,
            store,
            tx_ctx.clone(),
            gas_charger,
            advance_epoch_pt,
            trace_builder_opt,
        );

        #[cfg(msim)]
        let result = maybe_modify_result(result, params.epoch);

        if let Err(err) = &result {
            tracing::error!(
                "Failed to execute advance epoch transaction. Switching to safe mode. Error: {:?}. Input objects: {:?}. Tx params: {:?}",
                err.0,
                temporary_store.objects(),
                params,
            );
            temporary_store.drop_writes();
            // Must reset the storage rebate since we are re-executing.
            gas_charger.reset_storage_cost_and_rebate();

            temporary_store.advance_epoch_safe_mode(&params, protocol_config);
        }

        let new_vm = new_move_runtime(
            all_natives(/* silent */ true, protocol_config),
            protocol_config,
        )
        .expect("Failed to create new MoveVM");
        process_system_packages(
            system_packages,
            temporary_store,
            store,
            tx_ctx,
            &new_vm,
            gas_charger,
            protocol_config,
            metrics,
            trace_builder_opt,
        );

        Ok(())
    }

    /// Advances the epoch for the given `ChangeEpoch` transaction kind by
    /// constructing a programmable transaction, executing it and processing the
    /// system packages.
    fn advance_epoch_v1(
        builder: ProgrammableTransactionBuilder,
        change_epoch: ChangeEpoch,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let params = AdvanceEpochParams {
            epoch: change_epoch.epoch,
            next_protocol_version: change_epoch.protocol_version.into(),
            validator_subsidy: protocol_config.validator_target_reward(),
            storage_charge: change_epoch.storage_charge,
            computation_charge: change_epoch.computation_charge,
            // all computation charge is burned in v1
            computation_charge_burned: change_epoch.computation_charge,
            storage_rebate: change_epoch.storage_rebate,
            non_refundable_storage_fee: change_epoch.non_refundable_storage_fee,
            reward_slashing_rate: protocol_config.reward_slashing_rate(),
            epoch_start_timestamp_ms: change_epoch.epoch_start_timestamp_ms,
            // AdvanceEpochV1 does not use those fields, but keeping them to avoid creating a
            // separate AdvanceEpochParams struct.
            max_committee_members_count: 0,
            eligible_active_validators: vec![],
            scores: vec![],
            adjust_rewards_by_score: false,
        };
        let advance_epoch_pt = construct_advance_epoch_pt_v1(builder, &params)?;
        advance_epoch_impl(
            advance_epoch_pt,
            params,
            change_epoch.system_packages,
            temporary_store,
            store,
            tx_ctx,
            move_vm,
            gas_charger,
            protocol_config,
            metrics,
            trace_builder_opt,
        )
    }

    /// Advances the epoch for the given `ChangeEpochV2` transaction kind by
    /// constructing a programmable transaction, executing it and processing the
    /// system packages.
    fn advance_epoch_v2(
        builder: ProgrammableTransactionBuilder,
        change_epoch_v2: ChangeEpochV2,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let params = AdvanceEpochParams {
            epoch: change_epoch_v2.epoch,
            next_protocol_version: change_epoch_v2.protocol_version.into(),
            validator_subsidy: protocol_config.validator_target_reward(),
            storage_charge: change_epoch_v2.storage_charge,
            computation_charge: change_epoch_v2.computation_charge,
            computation_charge_burned: change_epoch_v2.computation_charge_burned,
            storage_rebate: change_epoch_v2.storage_rebate,
            non_refundable_storage_fee: change_epoch_v2.non_refundable_storage_fee,
            reward_slashing_rate: protocol_config.reward_slashing_rate(),
            epoch_start_timestamp_ms: change_epoch_v2.epoch_start_timestamp_ms,
            max_committee_members_count: protocol_config.max_committee_members_count(),
            // AdvanceEpochV2 does not use these fields, but keeping them to avoid creating a
            // separate AdvanceEpochParams struct.
            eligible_active_validators: vec![],
            scores: vec![],
            adjust_rewards_by_score: false,
        };
        let advance_epoch_pt = construct_advance_epoch_pt_v2(builder, &params)?;
        advance_epoch_impl(
            advance_epoch_pt,
            params,
            change_epoch_v2.system_packages,
            temporary_store,
            store,
            tx_ctx,
            move_vm,
            gas_charger,
            protocol_config,
            metrics,
            trace_builder_opt,
        )
    }

    /// Advances the epoch for the given `ChangeEpochV3` transaction kind by
    /// constructing a programmable transaction, executing it and processing the
    /// system packages.
    fn advance_epoch_v3(
        builder: ProgrammableTransactionBuilder,
        change_epoch_v3: ChangeEpochV3,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let params = AdvanceEpochParams {
            epoch: change_epoch_v3.epoch,
            next_protocol_version: change_epoch_v3.protocol_version.into(),
            validator_subsidy: protocol_config.validator_target_reward(),
            storage_charge: change_epoch_v3.storage_charge,
            computation_charge: change_epoch_v3.computation_charge,
            computation_charge_burned: change_epoch_v3.computation_charge_burned,
            storage_rebate: change_epoch_v3.storage_rebate,
            non_refundable_storage_fee: change_epoch_v3.non_refundable_storage_fee,
            reward_slashing_rate: protocol_config.reward_slashing_rate(),
            epoch_start_timestamp_ms: change_epoch_v3.epoch_start_timestamp_ms,
            max_committee_members_count: protocol_config.max_committee_members_count(),
            eligible_active_validators: change_epoch_v3.eligible_active_validators,
            // AdvanceEpochV3 does not use these fields, but keeping them to avoid creating a
            // separate AdvanceEpochParams struct.
            scores: vec![],
            adjust_rewards_by_score: false,
        };
        let advance_epoch_pt = construct_advance_epoch_pt_v3(builder, &params)?;
        advance_epoch_impl(
            advance_epoch_pt,
            params,
            change_epoch_v3.system_packages,
            temporary_store,
            store,
            tx_ctx,
            move_vm,
            gas_charger,
            protocol_config,
            metrics,
            trace_builder_opt,
        )
    }

    /// Advances the epoch for the given `ChangeEpochV4` transaction kind by
    /// constructing a programmable transaction, executing it and processing the
    /// system packages.
    fn advance_epoch_v4(
        builder: ProgrammableTransactionBuilder,
        change_epoch_v4: ChangeEpochV4,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let params = AdvanceEpochParams {
            epoch: change_epoch_v4.epoch,
            next_protocol_version: change_epoch_v4.protocol_version.into(),
            validator_subsidy: protocol_config.validator_target_reward(),
            storage_charge: change_epoch_v4.storage_charge,
            computation_charge: change_epoch_v4.computation_charge,
            computation_charge_burned: change_epoch_v4.computation_charge_burned,
            storage_rebate: change_epoch_v4.storage_rebate,
            non_refundable_storage_fee: change_epoch_v4.non_refundable_storage_fee,
            reward_slashing_rate: protocol_config.reward_slashing_rate(),
            epoch_start_timestamp_ms: change_epoch_v4.epoch_start_timestamp_ms,
            max_committee_members_count: protocol_config.max_committee_members_count(),
            eligible_active_validators: change_epoch_v4.eligible_active_validators,
            scores: change_epoch_v4.scores,
            adjust_rewards_by_score: change_epoch_v4.adjust_rewards_by_score,
        };
        let advance_epoch_pt = construct_advance_epoch_pt_v4(builder, &params)?;
        advance_epoch_impl(
            advance_epoch_pt,
            params,
            change_epoch_v4.system_packages,
            temporary_store,
            store,
            tx_ctx,
            move_vm,
            gas_charger,
            protocol_config,
            metrics,
            trace_builder_opt,
        )
    }

    fn process_system_packages(
        system_packages: Vec<SystemPackage>,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &MoveRuntime,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) {
        let binary_config = to_binary_config(protocol_config, None);
        for SystemPackage {
            version,
            modules,
            dependencies,
        } in system_packages.into_iter()
        {
            let deserialized_modules: Vec<_> = modules
                .iter()
                .map(|m| CompiledModule::deserialize_with_config(m, &binary_config).unwrap())
                .collect();

            if version == OBJECT_START_VERSION {
                let package_id = deserialized_modules.first().unwrap().address();
                info!("adding new system package {package_id}");

                let publish_pt = {
                    let mut b = ProgrammableTransactionBuilder::new();
                    b.command(Command::new_publish(modules, dependencies));
                    b.finish()
                };

                SPT::execute::<execution_mode::System<ExecutionError>>(
                    protocol_config,
                    metrics.clone(),
                    move_vm,
                    temporary_store,
                    store,
                    tx_ctx.clone(),
                    gas_charger,
                    publish_pt,
                    trace_builder_opt,
                )
                .map_err(|(e, _)| e)
                .expect("System Package Publish must succeed");
            } else {
                let mut new_package = Object::new_system_package(
                    &deserialized_modules,
                    version,
                    dependencies,
                    tx_ctx.borrow().digest(),
                );

                info!("upgraded system package {:?}", new_package.object_ref());

                // Decrement the version before writing the package so that the store can record
                // the version growing by one in the effects.
                new_package
                    .data
                    .as_opt_mut_package()
                    .unwrap()
                    .decrement_version()
                    .expect("package version should never underflow");

                // upgrade of a previously existing framework module
                temporary_store.upgrade_system_package(new_package);
            }
        }
    }

    /// Perform metadata updates in preparation for the transactions in the
    /// upcoming checkpoint:
    ///
    /// - Set the timestamp for the `Clock` shared object from the timestamp in the header from
    ///   consensus.
    fn setup_consensus_commit(
        consensus_commit_timestamp_ms: CheckpointTimestamp,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let res = builder.move_call(
                ObjectId::FRAMEWORK,
                iota_sdk_types::Identifier::CLOCK_MODULE,
                CONSENSUS_COMMIT_PROLOGUE_FUNCTION_NAME,
                vec![],
                vec![
                    Input::CLOCK_MUTABLE,
                    Input::pure(&consensus_commit_timestamp_ms),
                ],
            );
            assert_invariant!(
                res.is_ok(),
                "Unable to generate consensus_commit_prologue transaction!"
            );
            builder.finish()
        };
        SPT::execute::<execution_mode::System<ExecutionError>>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            store,
            tx_ctx,
            gas_charger,
            pt,
            trace_builder_opt,
        )
        .map_err(|(e, _)| e)?;
        Ok(())
    }

    /// The function constructs a transaction that invokes
    /// the `randomness_state_update` function from the IOTA framework,
    /// passing the randomness state object, the `randomness_round`,
    /// and the `random_bytes` as arguments. It then executes the transaction
    /// using the system execution mode.
    fn setup_randomness_state_update(
        update: RandomnessStateUpdate,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let res = builder.move_call(
                ObjectId::FRAMEWORK,
                iota_sdk_types::Identifier::RANDOM_MODULE,
                RANDOMNESS_STATE_UPDATE_FUNCTION_NAME,
                vec![],
                vec![
                    Input::Shared(SharedObjectReference::new(
                        ObjectId::RANDOMNESS_STATE,
                        update.randomness_obj_initial_shared_version,
                        true,
                    )),
                    Input::pure(&update.randomness_round),
                    Input::pure(&update.random_bytes),
                ],
            );
            assert_invariant!(
                res.is_ok(),
                "Unable to generate randomness_state_update transaction!"
            );
            builder.finish()
        };
        SPT::execute::<execution_mode::System<ExecutionError>>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            store,
            tx_ctx,
            gas_charger,
            pt,
            trace_builder_opt,
        )
        .map_err(|(e, _)| e)?;
        Ok(())
    }

    /// Appends the `transaction_deny_rules::create` call to the end-of-epoch
    /// transaction being built. If the built transaction later fails and epoch
    /// advancement falls back to safe mode, the creation is dropped with it
    /// and must be re-injected at a later epoch end while the object is
    /// absent.
    fn setup_transaction_deny_rules_create(
        mut builder: ProgrammableTransactionBuilder,
    ) -> Result<ProgrammableTransactionBuilder, ExecutionError> {
        let res = builder.move_call(
            ObjectId::FRAMEWORK,
            TRANSACTION_DENY_RULES_MODULE,
            TRANSACTION_DENY_RULES_CREATE_FUNCTION_NAME,
            vec![],
            vec![],
        );
        assert_invariant!(
            res.is_ok(),
            "Unable to generate transaction_deny_rules create transaction!"
        );
        Ok(builder)
    }

    /// Executes a `TransactionDenyRulesUpdate` system transaction: a single
    /// call to `transaction_deny_rules::update` applying the payload's
    /// add/remove delta and switch states.
    fn setup_transaction_deny_rules_update(
        update: TransactionDenyRulesUpdate,
        temporary_store: &mut TemporaryStore<'_>,
        store: &dyn BackingStore,
        tx_ctx: Rc<RefCell<TxContext>>,
        move_vm: &Arc<MoveRuntime>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            // Argument order must match `transaction_deny_rules::update`. The
            // set-typed delta lists BCS-encode identically to the `vector`
            // parameters the Move function takes.
            let res = builder.move_call(
                ObjectId::FRAMEWORK,
                TRANSACTION_DENY_RULES_MODULE,
                TRANSACTION_DENY_RULES_UPDATE_FUNCTION_NAME,
                vec![],
                vec![
                    Input::Shared(SharedObjectReference::new(
                        ObjectId::TRANSACTION_DENY_RULES,
                        update.deny_rules_obj_initial_shared_version,
                        true,
                    )),
                    Input::pure(&update.added_addresses),
                    Input::pure(&update.removed_addresses),
                    Input::pure(&update.added_objects),
                    Input::pure(&update.removed_objects),
                    Input::pure(&update.added_packages),
                    Input::pure(&update.removed_packages),
                    Input::pure(&update.package_publish_disabled),
                    Input::pure(&update.package_upgrade_disabled),
                    Input::pure(&update.shared_object_disabled),
                    Input::pure(&update.user_transaction_disabled),
                    Input::pure(&update.receiving_objects_disabled),
                    Input::pure(&update.move_authenticator_disabled),
                ],
            );
            assert_invariant!(
                res.is_ok(),
                "Unable to generate transaction_deny_rules update transaction!"
            );
            builder.finish()
        };
        SPT::execute::<execution_mode::System<ExecutionError>>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            store,
            tx_ctx,
            gas_charger,
            pt,
            trace_builder_opt,
        )
        .map(|(results, _timings)| results)
        .map_err(|(error, _timings)| error)
    }

    /// The error a transaction fails with before execution because of its input objects: an input
    /// that was deleted, or shared objects cancelled by congestion control or missing randomness.
    fn input_objects_error(
        protocol_config: &ProtocolConfig,
        contains_deleted_input: bool,
        cancelled_objects: Option<(CancelledObjects, Version)>,
    ) -> Option<ExecutionErrorKind> {
        if contains_deleted_input {
            return Some(ExecutionErrorKind::InputObjectDeleted);
        }
        let (cancelled_objects, reason) = cancelled_objects?;
        Some(match reason {
            version if version.is_congested() => match cancelled_objects {
                // A transaction cancelled through its gas object has no shared inputs, so the
                // execution workers are the congested resource and no individual object is
                // responsible.
                CancelledObjects::GasObject => {
                    ExecutionErrorKind::ExecutionCanceledDueToExecutionWorkerCongestion {
                        suggested_gas_price: version
                            .get_congested_version_suggested_gas_price()
                            .expect(
                                "execution-worker congestion control requires the gas price \
                                 feedback mechanism",
                            ),
                    }
                }
                CancelledObjects::SharedObjects(congested_objects) => {
                    if protocol_config.congestion_control_gas_price_feedback_mechanism() {
                        ExecutionErrorKind::ExecutionCanceledDueToSharedObjectCongestionV2 {
                            congested_objects,
                            suggested_gas_price: version
                                .get_congested_version_suggested_gas_price()
                                .expect("congested version carries a suggested gas price"),
                        }
                    } else {
                        ExecutionErrorKind::ExecutionCanceledDueToSharedObjectCongestion {
                            congested_objects,
                        }
                    }
                }
            },
            Version::RANDOMNESS_UNAVAILABLE => {
                ExecutionErrorKind::ExecutionCanceledDueToRandomnessUnavailable
            }
            _ => panic!("invalid cancellation reason Version: {reason}"),
        })
    }
}
