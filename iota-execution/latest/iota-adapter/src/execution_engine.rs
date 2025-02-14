// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {

    use std::{collections::HashSet, sync::Arc};

    use iota_move_natives::all_natives;
    use iota_protocol_config::{LimitThresholdCrossed, ProtocolConfig, check_limit_by_meter};
    #[cfg(msim)]
    use iota_types::iota_system_state::advance_epoch_result_injection::maybe_modify_result;
    use iota_types::{
        BRIDGE_ADDRESS, IOTA_AUTHENTICATOR_STATE_OBJECT_ID, IOTA_BRIDGE_OBJECT_ID,
        IOTA_FRAMEWORK_ADDRESS, IOTA_FRAMEWORK_PACKAGE_ID, IOTA_RANDOMNESS_STATE_OBJECT_ID,
        IOTA_SYSTEM_PACKAGE_ID,
        authenticator_state::{
            AUTHENTICATOR_STATE_CREATE_FUNCTION_NAME,
            AUTHENTICATOR_STATE_EXPIRE_JWKS_FUNCTION_NAME, AUTHENTICATOR_STATE_MODULE_NAME,
            AUTHENTICATOR_STATE_UPDATE_FUNCTION_NAME,
        },
        balance::{
            BALANCE_CREATE_REWARDS_FUNCTION_NAME, BALANCE_DESTROY_REBATES_FUNCTION_NAME,
            BALANCE_MODULE_NAME,
        },
        base_types::{
            IotaAddress, ObjectID, ObjectRef, SequenceNumber, TransactionDigest, TxContext,
        },
        bridge::{
            BRIDGE_COMMITTEE_MINIMAL_VOTING_POWER, BRIDGE_CREATE_FUNCTION_NAME,
            BRIDGE_INIT_COMMITTEE_FUNCTION_NAME, BRIDGE_MODULE_NAME, BridgeChainId,
        },
        clock::{CLOCK_MODULE_NAME, CONSENSUS_COMMIT_PROLOGUE_FUNCTION_NAME},
        committee::EpochId,
        digests::{ChainIdentifier, get_mainnet_chain_identifier, get_testnet_chain_identifier},
        effects::TransactionEffects,
        error::{ExecutionError, ExecutionErrorKind},
        execution::{ExecutionResults, ExecutionResultsV1, is_certificate_denied},
        execution_config_utils::to_binary_config,
        execution_status::{CongestedObjects, ExecutionStatus},
        gas::{GasCostSummary, IotaGasStatus},
        gas_coin::GAS,
        id::UID,
        inner_temporary_store::InnerTemporaryStore,
        iota_system_state::{
            ADVANCE_EPOCH_FUNCTION_NAME, AdvanceEpochParams, IOTA_SYSTEM_MODULE_NAME,
        },
        messages_checkpoint::CheckpointTimestamp,
        metrics::LimitsMetrics,
        object::{OBJECT_START_VERSION, Object, ObjectInner},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        randomness_state::{RANDOMNESS_MODULE_NAME, RANDOMNESS_STATE_UPDATE_FUNCTION_NAME},
        storage::{BackingStore, Storage},
        transaction::{
            Argument, AuthenticatorStateExpire, AuthenticatorStateUpdateV1, CallArg, ChangeEpoch,
            ChangeEpochV2, CheckedInputObjects, Command, EndOfEpochTransactionKind,
            GenesisTransaction, ObjectArg, ProgrammableTransaction, RandomnessStateUpdate,
            TransactionKind,
        },
    };
    use move_binary_format::CompiledModule;
    use move_core_types::ident_str;
    use move_vm_runtime::move_vm::MoveVM;
    use tracing::{info, instrument, trace, warn};

    use crate::{
        adapter::new_move_vm,
        execution_mode::{self, ExecutionMode},
        gas_charger::GasCharger,
        programmable_transactions,
        temporary_store::TemporaryStore,
        type_layout_resolver::TypeLayoutResolver,
    };

    /// The main entry point to the adapter's transaction execution. It
    /// prepares a transaction for execution, then executes it through an
    /// inner execution method and finally produces an instance of
    /// transaction effects. It also returns the inner temporary store, which
    /// contains the objects resulting from the transaction execution, the gas
    /// status instance, which tracks the gas usage, and the execution result.
    /// The function handles transaction execution based on the provided
    /// `TransactionKind`. It checks for any expensive operations, manages
    /// shared object references, and ensures transaction dependencies are
    /// met. The returned objects are not committed to the store until the
    /// resulting effects are applied by the caller.
    #[instrument(name = "tx_execute_to_effects", level = "debug", skip_all)]
    pub fn execute_transaction_to_effects<Mode: ExecutionMode>(
        store: &dyn BackingStore,
        input_objects: CheckedInputObjects,
        gas_coins: Vec<ObjectRef>,
        gas_status: IotaGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: IotaAddress,
        transaction_digest: TransactionDigest,
        move_vm: &Arc<MoveVM>,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        certificate_deny_set: &HashSet<TransactionDigest>,
    ) -> (
        InnerTemporaryStore,
        IotaGasStatus,
        TransactionEffects,
        Result<Mode::ExecutionResults, ExecutionError>,
    ) {
        let input_objects = input_objects.into_inner();
        let mutable_inputs = if enable_expensive_checks {
            input_objects.mutable_inputs().keys().copied().collect()
        } else {
            HashSet::new()
        };
        let shared_object_refs = input_objects.filter_shared_objects();
        let receiving_objects = transaction_kind.receiving_objects();
        let mut transaction_dependencies = input_objects.transaction_dependencies();
        let contains_deleted_input = input_objects.contains_deleted_objects();
        let cancelled_objects = input_objects.get_cancelled_objects();

        let mut temporary_store = TemporaryStore::new(
            store,
            input_objects,
            receiving_objects,
            transaction_digest,
            protocol_config,
            *epoch_id,
        );

        let mut gas_charger =
            GasCharger::new(transaction_digest, gas_coins, gas_status, protocol_config);

        let mut tx_ctx = TxContext::new_from_components(
            &transaction_signer,
            &transaction_digest,
            epoch_id,
            epoch_timestamp_ms,
        );

        let is_epoch_change = transaction_kind.is_end_of_epoch_tx();

        let deny_cert = is_certificate_denied(&transaction_digest, certificate_deny_set);
        let (gas_cost_summary, execution_result) = execute_transaction::<Mode>(
            &mut temporary_store,
            transaction_kind,
            &mut gas_charger,
            &mut tx_ctx,
            move_vm,
            protocol_config,
            metrics,
            enable_expensive_checks,
            deny_cert,
            contains_deleted_input,
            cancelled_objects,
        );

        let status = if let Err(error) = &execution_result {
            // Elaborate errors in logs if they are unexpected or their status is terse.
            use ExecutionErrorKind as K;
            match error.kind() {
                K::InvariantViolation | K::VMInvariantViolation => {
                    #[skip_checked_arithmetic]
                    tracing::error!(
                        kind = ?error.kind(),
                        tx_digest = ?transaction_digest,
                        "INVARIANT VIOLATION! Source: {:?}",
                        error.source(),
                    );
                }

                K::IotaMoveVerificationError | K::VMVerificationOrDeserializationError => {
                    #[skip_checked_arithmetic]
                    tracing::debug!(
                        kind = ?error.kind(),
                        tx_digest = ?transaction_digest,
                        "Verification Error. Source: {:?}",
                        error.source(),
                    );
                }

                K::PublishUpgradeMissingDependency | K::PublishUpgradeDependencyDowngrade => {
                    #[skip_checked_arithmetic]
                    tracing::debug!(
                        kind = ?error.kind(),
                        tx_digest = ?transaction_digest,
                        "Publish/Upgrade Error. Source: {:?}",
                        error.source(),
                    )
                }

                _ => (),
            };

            let (status, command) = error.to_execution_status();
            ExecutionStatus::new_failure(status, command)
        } else {
            ExecutionStatus::Success
        };

        #[skip_checked_arithmetic]
        trace!(
            tx_digest = ?transaction_digest,
            computation_gas_cost = gas_cost_summary.computation_cost,
            computation_gas_cost_burned = gas_cost_summary.computation_cost_burned,
            storage_gas_cost = gas_cost_summary.storage_cost,
            storage_gas_rebate = gas_cost_summary.storage_rebate,
            "Finished execution of transaction with status {:?}",
            status
        );

        // Genesis writes a special digest to indicate that an object was created during
        // genesis and not written by any normal transaction - remove that from the
        // dependencies
        transaction_dependencies.remove(&TransactionDigest::genesis_marker());

        if enable_expensive_checks && !Mode::allow_arbitrary_function_calls() {
            temporary_store
                .check_ownership_invariants(
                    &transaction_signer,
                    &mut gas_charger,
                    &mutable_inputs,
                    is_epoch_change,
                )
                .unwrap()
        } // else, in dev inspect mode and anything goes--don't check

        let (inner, effects) = temporary_store.into_effects(
            shared_object_refs,
            &transaction_digest,
            transaction_dependencies,
            gas_cost_summary,
            status,
            &mut gas_charger,
            *epoch_id,
        );

        (
            inner,
            gas_charger.into_gas_status(),
            effects,
            execution_result,
        )
    }

    /// Function dedicated to the execution of a GenesisTransaction.
    /// The function creates an `InnerTemporaryStore`, processes the input
    /// objects, and executes the transaction in unmetered mode using the
    /// `Genesis` execution mode. It returns an inner temporary store that
    /// contains the objects found into the input `GenesisTransaction` by
    /// adding the data for `previous_transaction` and `storage_rebate` fields.
    pub fn execute_genesis_state_update(
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        move_vm: &Arc<MoveVM>,
        tx_context: &mut TxContext,
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore, ExecutionError> {
        let input_objects = input_objects.into_inner();
        let mut temporary_store = TemporaryStore::new(
            store,
            input_objects,
            vec![],
            tx_context.digest(),
            protocol_config,
            0,
        );
        let mut gas_charger = GasCharger::new_unmetered(tx_context.digest());
        programmable_transactions::execution::execute::<execution_mode::Genesis>(
            protocol_config,
            metrics,
            move_vm,
            &mut temporary_store,
            tx_context,
            &mut gas_charger,
            pt,
        )?;
        temporary_store.update_object_version_and_prev_tx();
        Ok(temporary_store.into_inner())
    }

    /// Executes a transaction by processing the specified `TransactionKind`,
    /// applying the necessary gas charges and running the main execution logic.
    /// The function handles certain error conditions such as denied
    /// certificate, deleted input objects, exceeded execution meter limits,
    /// failed conservation checks. It also accounts for unmetered storage
    /// rebates and adjusts for special cases like epoch change
    /// transactions. Gas costs are managed through the `GasCharger`
    /// argument; gas is also charged in case of errors.
    #[instrument(name = "tx_execute", level = "debug", skip_all)]
    fn execute_transaction<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        transaction_kind: TransactionKind,
        gas_charger: &mut GasCharger,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        deny_cert: bool,
        contains_deleted_input: bool,
        cancelled_objects: Option<(Vec<ObjectID>, SequenceNumber)>,
    ) -> (
        GasCostSummary,
        Result<Mode::ExecutionResults, ExecutionError>,
    ) {
        gas_charger.smash_gas(temporary_store);

        // At this point no charges have been applied yet
        debug_assert!(
            gas_charger.no_charges(),
            "No gas charges must be applied yet"
        );

        let is_genesis_or_epoch_change_tx = matches!(transaction_kind, TransactionKind::Genesis(_))
            || transaction_kind.is_end_of_epoch_tx();

        let advance_epoch_gas_summary = transaction_kind.get_advance_epoch_tx_gas_summary();

        // We must charge object read here during transaction execution, because if this
        // fails we must still ensure an effect is committed and all objects
        // versions incremented
        let result = gas_charger.charge_input_objects(temporary_store);
        let mut result = result.and_then(|()| {
            let mut execution_result = if deny_cert {
                Err(ExecutionError::new(
                    ExecutionErrorKind::CertificateDenied,
                    None,
                ))
            } else if contains_deleted_input {
                Err(ExecutionError::new(
                    ExecutionErrorKind::InputObjectDeleted,
                    None,
                ))
            } else if let Some((cancelled_objects, reason)) = cancelled_objects {
                match reason {
                    SequenceNumber::CONGESTED => Err(ExecutionError::new(
                        ExecutionErrorKind::ExecutionCancelledDueToSharedObjectCongestion {
                            congested_objects: CongestedObjects(cancelled_objects),
                        },
                        None,
                    )),
                    SequenceNumber::RANDOMNESS_UNAVAILABLE => Err(ExecutionError::new(
                        ExecutionErrorKind::ExecutionCancelledDueToRandomnessUnavailable,
                        None,
                    )),
                    _ => panic!("invalid cancellation reason SequenceNumber: {reason}"),
                }
            } else {
                execution_loop::<Mode>(
                    temporary_store,
                    transaction_kind,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics.clone(),
                )
            };

            let meter_check = check_meter_limit(
                temporary_store,
                gas_charger,
                protocol_config,
                metrics.clone(),
            );
            if let Err(e) = meter_check {
                execution_result = Err(e);
            }

            if execution_result.is_ok() {
                let gas_check = check_written_objects_limit::<Mode>(
                    temporary_store,
                    gas_charger,
                    protocol_config,
                    metrics,
                );
                if let Err(e) = gas_check {
                    execution_result = Err(e);
                }
            }

            execution_result
        });

        let cost_summary = gas_charger.charge_gas(temporary_store, &mut result);
        // For advance epoch transaction, we need to provide epoch rewards and rebates
        // as extra information provided to check_iota_conserved, because we
        // mint rewards, and burn the rebates. We also need to pass in the
        // unmetered_storage_rebate because storage rebate is not reflected in
        // the storage_rebate of gas summary. This is a bit confusing.
        // We could probably clean up the code a bit.
        // Put all the storage rebate accumulated in the system transaction
        // to the 0x5 object so that it's not lost.
        temporary_store.conserve_unmetered_storage_rebate(gas_charger.unmetered_storage_rebate());

        if let Err(e) = run_conservation_checks::<Mode>(
            temporary_store,
            gas_charger,
            tx_ctx,
            move_vm,
            enable_expensive_checks,
            &cost_summary,
            is_genesis_or_epoch_change_tx,
            advance_epoch_gas_summary,
        ) {
            // FIXME: we cannot fail the transaction if this is an epoch change transaction.
            result = Err(e);
        }

        (cost_summary, result)
    }

    /// Performs IOTA conservation checks during transaction execution, ensuring
    /// that the transaction does not create or destroy IOTA. If
    /// conservation is violated, the function attempts to recover
    /// by resetting the gas charger, recharging gas, and rechecking
    /// conservation. If recovery fails, it panics to avoid IOTA creation or
    /// destruction. These checks include both simple and expensive
    /// checks based on the configuration and are skipped for genesis or epoch
    /// change transactions.
    #[instrument(name = "run_conservation_checks", level = "debug", skip_all)]
    fn run_conservation_checks<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        enable_expensive_checks: bool,
        cost_summary: &GasCostSummary,
        is_genesis_or_epoch_change_tx: bool,
        advance_epoch_gas_summary: Option<(u64, u64)>,
    ) -> Result<(), ExecutionError> {
        let mut result: std::result::Result<(), iota_types::error::ExecutionError> = Ok(());
        if !is_genesis_or_epoch_change_tx && !Mode::skip_conservation_checks() {
            // ensure that this transaction did not create or destroy IOTA, try to recover
            // if the check fails
            let conservation_result = {
                temporary_store
                    .check_iota_conserved(cost_summary)
                    .and_then(|()| {
                        if enable_expensive_checks {
                            // ensure that this transaction did not create or destroy IOTA, try to
                            // recover if the check fails
                            let mut layout_resolver =
                                TypeLayoutResolver::new(move_vm, Box::new(&*temporary_store));
                            temporary_store.check_iota_conserved_expensive(
                                cost_summary,
                                advance_epoch_gas_summary,
                                &mut layout_resolver,
                            )
                        } else {
                            Ok(())
                        }
                    })
            };
            if let Err(conservation_err) = conservation_result {
                // conservation violated. try to avoid panic by dumping all writes, charging for
                // gas, re-checking conservation, and surfacing an aborted
                // transaction with an invariant violation if all of that works
                result = Err(conservation_err);
                gas_charger.reset(temporary_store);
                gas_charger.charge_gas(temporary_store, &mut result);
                // check conservation once more more
                if let Err(recovery_err) = {
                    temporary_store
                        .check_iota_conserved(cost_summary)
                        .and_then(|()| {
                            if enable_expensive_checks {
                                // ensure that this transaction did not create or destroy IOTA, try
                                // to recover if the check fails
                                let mut layout_resolver =
                                    TypeLayoutResolver::new(move_vm, Box::new(&*temporary_store));
                                temporary_store.check_iota_conserved_expensive(
                                    cost_summary,
                                    advance_epoch_gas_summary,
                                    &mut layout_resolver,
                                )
                            } else {
                                Ok(())
                            }
                        })
                } {
                    // if we still fail, it's a problem with gas
                    // charging that happens even in the "aborted" case--no other option but panic.
                    // we will create or destroy IOTA otherwise
                    panic!(
                        "IOTA conservation fail in tx block {}: {}\nGas status is {}\nTx was ",
                        tx_ctx.digest(),
                        recovery_err,
                        gas_charger.summary()
                    )
                }
            }
        } // else, we're in the genesis transaction which mints the IOTA supply, and hence
        // does not satisfy IOTA conservation, or we're in the non-production
        // dev inspect mode which allows us to violate conservation
        result
    }

    /// Checks if the estimated size of transaction effects exceeds predefined
    /// limits based on the protocol configuration. For metered
    /// transactions, it enforces hard limits, while for system transactions, it
    /// allows soft limits with warnings.
    #[instrument(name = "check_meter_limit", level = "debug", skip_all)]
    fn check_meter_limit(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let effects_estimated_size = temporary_store.estimate_effects_size_upperbound();

        // Check if a limit threshold was crossed.
        // For metered transactions, there is not soft limit.
        // For system transactions, we allow a soft limit with alerting, and a hard
        // limit where we terminate
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
            LimitThresholdCrossed::Hard(_, lim) => Err(ExecutionError::new_with_source(
                ExecutionErrorKind::EffectsTooLarge {
                    current_size: effects_estimated_size as u64,
                    max_size: lim as u64,
                },
                "Transaction effects are too large",
            )),
        }
    }

    /// Checks if the total size of written objects in the transaction exceeds
    /// the limits defined in the protocol configuration. For metered
    /// transactions, it enforces a hard limit, while for system transactions,
    /// it allows a soft limit with warnings.
    #[instrument(name = "check_written_objects_limit", level = "debug", skip_all)]
    fn check_written_objects_limit<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
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
                    return Err(ExecutionError::new_with_source(
                        ExecutionErrorKind::WrittenObjectsTooLarge {
                            current_size: written_objects_size as u64,
                            max_size: lim as u64,
                        },
                        "Written objects size crossed hard limit",
                    ));
                }
            };
        }

        Ok(())
    }

    /// Executes the given transaction based on its `TransactionKind` by
    /// processing it through corresponding handlers such as epoch changes,
    /// genesis transactions, consensus commit prologues, and programmable
    /// transactions. For each type of transaction, the corresponding logic is
    /// invoked, such as advancing the epoch, setting up consensus commits, or
    /// executing a programmable transaction.
    #[instrument(level = "debug", skip_all)]
    fn execution_loop<Mode: ExecutionMode>(
        temporary_store: &mut TemporaryStore<'_>,
        transaction_kind: TransactionKind,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<Mode::ExecutionResults, ExecutionError> {
        let result = match transaction_kind {
            TransactionKind::Genesis(GenesisTransaction { objects, events }) => {
                if tx_ctx.epoch() != 0 {
                    panic!("BUG: Genesis Transactions can only be executed in epoch 0");
                }

                for genesis_object in objects {
                    match genesis_object {
                        iota_types::transaction::GenesisObject::RawObject { data, owner } => {
                            let object = ObjectInner {
                                data,
                                owner,
                                previous_transaction: tx_ctx.digest(),
                                storage_rebate: 0,
                            };
                            temporary_store.create_object(object.into());
                        }
                    }
                }

                temporary_store.record_execution_results(ExecutionResults::V1(
                    ExecutionResultsV1 {
                        user_events: events,
                        ..Default::default()
                    },
                ));

                Ok(Mode::empty_results())
            }
            TransactionKind::ConsensusCommitPrologueV1(prologue) => {
                setup_consensus_commit(
                    prologue.commit_timestamp_ms,
                    temporary_store,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics,
                )
                .expect("ConsensusCommitPrologueV1 cannot fail");
                Ok(Mode::empty_results())
            }
            TransactionKind::ProgrammableTransaction(pt) => {
                programmable_transactions::execution::execute::<Mode>(
                    protocol_config,
                    metrics,
                    move_vm,
                    temporary_store,
                    tx_ctx,
                    gas_charger,
                    pt,
                )
            }
            TransactionKind::EndOfEpochTransaction(txns) => {
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
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics,
                            )?;
                            return Ok(Mode::empty_results());
                        }
                        EndOfEpochTransactionKind::ChangeEpochV2(change_epoch_v2) => {
                            assert_eq!(i, len - 1);
                            advance_epoch_v2(
                                builder,
                                change_epoch_v2,
                                temporary_store,
                                tx_ctx,
                                move_vm,
                                gas_charger,
                                protocol_config,
                                metrics,
                            )?;
                            return Ok(Mode::empty_results());
                        }
                        EndOfEpochTransactionKind::AuthenticatorStateCreate => {
                            assert!(protocol_config.enable_jwk_consensus_updates());
                            builder = setup_authenticator_state_create(builder);
                        }
                        EndOfEpochTransactionKind::AuthenticatorStateExpire(expire) => {
                            assert!(protocol_config.enable_jwk_consensus_updates());

                            // TODO: it would be nice if a failure of this function didn't cause
                            // safe mode.
                            builder = setup_authenticator_state_expire(builder, expire);
                        }
                        EndOfEpochTransactionKind::BridgeStateCreate(chain_id) => {
                            assert!(protocol_config.enable_bridge());
                            builder = setup_bridge_create(builder, chain_id)
                        }
                        EndOfEpochTransactionKind::BridgeCommitteeInit(bridge_shared_version) => {
                            assert!(protocol_config.enable_bridge());
                            assert!(protocol_config.should_try_to_finalize_bridge_committee());
                            builder = setup_bridge_committee_update(builder, bridge_shared_version)
                        }
                    }
                }
                unreachable!(
                    "EndOfEpochTransactionKind::ChangeEpoch should be the last transaction in the list"
                )
            }
            TransactionKind::AuthenticatorStateUpdateV1(auth_state_update) => {
                setup_authenticator_state_update(
                    auth_state_update,
                    temporary_store,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics,
                )?;
                Ok(Mode::empty_results())
            }
            TransactionKind::RandomnessStateUpdate(randomness_state_update) => {
                setup_randomness_state_update(
                    randomness_state_update,
                    temporary_store,
                    tx_ctx,
                    move_vm,
                    gas_charger,
                    protocol_config,
                    metrics,
                )?;
                Ok(Mode::empty_results())
            }
        }?;
        temporary_store.check_execution_results_consistency()?;
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
        let storage_charge_arg = builder
            .input(CallArg::Pure(
                bcs::to_bytes(&params.storage_charge).unwrap(),
            ))
            .unwrap();
        let storage_charges = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            BALANCE_MODULE_NAME.to_owned(),
            BALANCE_CREATE_REWARDS_FUNCTION_NAME.to_owned(),
            vec![GAS::type_tag()],
            vec![storage_charge_arg],
        );

        // Create computation charges.
        let computation_charge_arg = builder
            .input(CallArg::Pure(
                bcs::to_bytes(&params.computation_charge).unwrap(),
            ))
            .unwrap();
        let computation_charges = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            BALANCE_MODULE_NAME.to_owned(),
            BALANCE_CREATE_REWARDS_FUNCTION_NAME.to_owned(),
            vec![GAS::type_tag()],
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
        call_arg_vec: Vec<CallArg>,
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
            IOTA_SYSTEM_PACKAGE_ID,
            IOTA_SYSTEM_MODULE_NAME.to_owned(),
            ADVANCE_EPOCH_FUNCTION_NAME.to_owned(),
            vec![],
            arguments,
        );

        // Step 3: Destroy the storage rebates.
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            BALANCE_MODULE_NAME.to_owned(),
            BALANCE_DESTROY_REBATES_FUNCTION_NAME.to_owned(),
            vec![GAS::type_tag()],
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
            CallArg::IOTA_SYSTEM_MUT, // wrapper: &mut IotaSystemState
            CallArg::Pure(bcs::to_bytes(&params.epoch).unwrap()), // new_epoch: u64
            CallArg::Pure(bcs::to_bytes(&params.next_protocol_version.as_u64()).unwrap()), /* next_protocol_version: u64 */
            CallArg::Pure(bcs::to_bytes(&params.storage_rebate).unwrap()), // storage_rebate: u64
            CallArg::Pure(bcs::to_bytes(&params.non_refundable_storage_fee).unwrap()), /* non_refundable_storage_fee: u64 */
            CallArg::Pure(bcs::to_bytes(&params.reward_slashing_rate).unwrap()), /* reward_slashing_rate: u64 */
            CallArg::Pure(bcs::to_bytes(&params.epoch_start_timestamp_ms).unwrap()), /* epoch_start_timestamp_ms: u64 */
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
            CallArg::Pure(bcs::to_bytes(&params.computation_charge_burned).unwrap()), /* computation_charge_burned: u64 */
            CallArg::IOTA_SYSTEM_MUT, // wrapper: &mut IotaSystemState
            CallArg::Pure(bcs::to_bytes(&params.epoch).unwrap()), // new_epoch: u64
            CallArg::Pure(bcs::to_bytes(&params.next_protocol_version.as_u64()).unwrap()), /* next_protocol_version: u64 */
            CallArg::Pure(bcs::to_bytes(&params.storage_rebate).unwrap()), // storage_rebate: u64
            CallArg::Pure(bcs::to_bytes(&params.non_refundable_storage_fee).unwrap()), /* non_refundable_storage_fee: u64 */
            CallArg::Pure(bcs::to_bytes(&params.reward_slashing_rate).unwrap()), /* reward_slashing_rate: u64 */
            CallArg::Pure(bcs::to_bytes(&params.epoch_start_timestamp_ms).unwrap()), /* epoch_start_timestamp_ms: u64 */
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
        system_packages: Vec<(SequenceNumber, Vec<Vec<u8>>, Vec<ObjectID>)>,
        temporary_store: &mut TemporaryStore<'_>,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let result = programmable_transactions::execution::execute::<execution_mode::System>(
            protocol_config,
            metrics.clone(),
            move_vm,
            temporary_store,
            tx_ctx,
            gas_charger,
            advance_epoch_pt,
        );

        #[cfg(msim)]
        let result = maybe_modify_result(result, params.epoch);

        if result.is_err() {
            tracing::error!(
                "Failed to execute advance epoch transaction. Switching to safe mode. Error: {:?}. Input objects: {:?}. Tx params: {:?}",
                result.as_ref().err(),
                temporary_store.objects(),
                params,
            );
            temporary_store.drop_writes();
            // Must reset the storage rebate since we are re-executing.
            gas_charger.reset_storage_cost_and_rebate();

            temporary_store.advance_epoch_safe_mode(&params, protocol_config);
        }

        let new_vm = new_move_vm(
            all_natives(/* silent */ true, protocol_config),
            protocol_config,
            // enable_profiler
            None,
        )
        .expect("Failed to create new MoveVM");
        process_system_packages(
            system_packages,
            temporary_store,
            tx_ctx,
            &new_vm,
            gas_charger,
            protocol_config,
            metrics,
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
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let params = AdvanceEpochParams {
            epoch: change_epoch.epoch,
            next_protocol_version: change_epoch.protocol_version,
            validator_subsidy: protocol_config.validator_target_reward(),
            storage_charge: change_epoch.storage_charge,
            computation_charge: change_epoch.computation_charge,
            // all computation charge is burned in v1
            computation_charge_burned: change_epoch.computation_charge,
            storage_rebate: change_epoch.storage_rebate,
            non_refundable_storage_fee: change_epoch.non_refundable_storage_fee,
            reward_slashing_rate: protocol_config.reward_slashing_rate(),
            epoch_start_timestamp_ms: change_epoch.epoch_start_timestamp_ms,
        };
        let advance_epoch_pt = construct_advance_epoch_pt_v1(builder, &params)?;
        advance_epoch_impl(
            advance_epoch_pt,
            params,
            change_epoch.system_packages,
            temporary_store,
            tx_ctx,
            move_vm,
            gas_charger,
            protocol_config,
            metrics,
        )
    }

    /// Advances the epoch for the given `ChangeEpochV2` transaction kind by
    /// constructing a programmable transaction, executing it and processing the
    /// system packages.
    fn advance_epoch_v2(
        builder: ProgrammableTransactionBuilder,
        change_epoch_v2: ChangeEpochV2,
        temporary_store: &mut TemporaryStore<'_>,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let params = AdvanceEpochParams {
            epoch: change_epoch_v2.epoch,
            next_protocol_version: change_epoch_v2.protocol_version,
            validator_subsidy: protocol_config.validator_target_reward(),
            storage_charge: change_epoch_v2.storage_charge,
            computation_charge: change_epoch_v2.computation_charge,
            computation_charge_burned: change_epoch_v2.computation_charge_burned,
            storage_rebate: change_epoch_v2.storage_rebate,
            non_refundable_storage_fee: change_epoch_v2.non_refundable_storage_fee,
            reward_slashing_rate: protocol_config.reward_slashing_rate(),
            epoch_start_timestamp_ms: change_epoch_v2.epoch_start_timestamp_ms,
        };
        let advance_epoch_pt = construct_advance_epoch_pt_v2(builder, &params)?;
        advance_epoch_impl(
            advance_epoch_pt,
            params,
            change_epoch_v2.system_packages,
            temporary_store,
            tx_ctx,
            move_vm,
            gas_charger,
            protocol_config,
            metrics,
        )
    }

    fn process_system_packages(
        system_packages: Vec<(SequenceNumber, Vec<Vec<u8>>, Vec<ObjectID>)>,
        temporary_store: &mut TemporaryStore<'_>,
        tx_ctx: &mut TxContext,
        move_vm: &MoveVM,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) {
        let binary_config = to_binary_config(protocol_config);
        for (version, modules, dependencies) in system_packages.into_iter() {
            let deserialized_modules: Vec<_> = modules
                .iter()
                .map(|m| CompiledModule::deserialize_with_config(m, &binary_config).unwrap())
                .collect();

            if version == OBJECT_START_VERSION {
                let package_id = deserialized_modules.first().unwrap().address();
                info!("adding new system package {package_id}");

                let publish_pt = {
                    let mut b = ProgrammableTransactionBuilder::new();
                    b.command(Command::Publish(modules, dependencies));
                    b.finish()
                };

                programmable_transactions::execution::execute::<execution_mode::System>(
                    protocol_config,
                    metrics.clone(),
                    move_vm,
                    temporary_store,
                    tx_ctx,
                    gas_charger,
                    publish_pt,
                )
                .expect("System Package Publish must succeed");
            } else {
                let mut new_package = Object::new_system_package(
                    &deserialized_modules,
                    version,
                    dependencies,
                    tx_ctx.digest(),
                );

                info!(
                    "upgraded system package {:?}",
                    new_package.compute_object_reference()
                );

                // Decrement the version before writing the package so that the store can record
                // the version growing by one in the effects.
                new_package
                    .data
                    .try_as_package_mut()
                    .unwrap()
                    .decrement_version();

                // upgrade of a previously existing framework module
                temporary_store.upgrade_system_package(new_package);
            }
        }
    }

    /// Perform metadata updates in preparation for the transactions in the
    /// upcoming checkpoint:
    ///
    /// - Set the timestamp for the `Clock` shared object from the timestamp in
    ///   the header from consensus.
    fn setup_consensus_commit(
        consensus_commit_timestamp_ms: CheckpointTimestamp,
        temporary_store: &mut TemporaryStore<'_>,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let res = builder.move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                CLOCK_MODULE_NAME.to_owned(),
                CONSENSUS_COMMIT_PROLOGUE_FUNCTION_NAME.to_owned(),
                vec![],
                vec![
                    CallArg::CLOCK_MUT,
                    CallArg::Pure(bcs::to_bytes(&consensus_commit_timestamp_ms).unwrap()),
                ],
            );
            assert_invariant!(
                res.is_ok(),
                "Unable to generate consensus_commit_prologue transaction!"
            );
            builder.finish()
        };
        programmable_transactions::execution::execute::<execution_mode::System>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            tx_ctx,
            gas_charger,
            pt,
        )
    }

    /// This function adds a Move call to the IOTA framework's
    /// `authenticator_state_create` function, preparing the transaction for
    /// execution.
    fn setup_authenticator_state_create(
        mut builder: ProgrammableTransactionBuilder,
    ) -> ProgrammableTransactionBuilder {
        builder
            .move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                AUTHENTICATOR_STATE_MODULE_NAME.to_owned(),
                AUTHENTICATOR_STATE_CREATE_FUNCTION_NAME.to_owned(),
                vec![],
                vec![],
            )
            .expect("Unable to generate authenticator_state_create transaction!");
        builder
    }

    /// Configures a `ProgrammableTransactionBuilder` to create a bridge.
    fn setup_bridge_create(
        mut builder: ProgrammableTransactionBuilder,
        chain_id: ChainIdentifier,
    ) -> ProgrammableTransactionBuilder {
        let bridge_uid = builder
            .input(CallArg::Pure(
                UID::new(IOTA_BRIDGE_OBJECT_ID).to_bcs_bytes(),
            ))
            .expect("Unable to create Bridge object UID!");

        let bridge_chain_id = if chain_id == get_mainnet_chain_identifier() {
            BridgeChainId::IotaMainnet as u8
        } else if chain_id == get_testnet_chain_identifier() {
            BridgeChainId::IotaTestnet as u8
        } else {
            // How do we distinguish devnet from other test envs?
            BridgeChainId::IotaCustom as u8
        };

        let bridge_chain_id = builder.pure(bridge_chain_id).unwrap();
        builder.programmable_move_call(
            BRIDGE_ADDRESS.into(),
            BRIDGE_MODULE_NAME.to_owned(),
            BRIDGE_CREATE_FUNCTION_NAME.to_owned(),
            vec![],
            vec![bridge_uid, bridge_chain_id],
        );
        builder
    }

    /// Configures a `ProgrammableTransactionBuilder` to update the bridge
    /// committee.
    fn setup_bridge_committee_update(
        mut builder: ProgrammableTransactionBuilder,
        bridge_shared_version: SequenceNumber,
    ) -> ProgrammableTransactionBuilder {
        let bridge = builder
            .obj(ObjectArg::SharedObject {
                id: IOTA_BRIDGE_OBJECT_ID,
                initial_shared_version: bridge_shared_version,
                mutable: true,
            })
            .expect("Unable to create Bridge object arg!");
        let system_state = builder
            .obj(ObjectArg::IOTA_SYSTEM_MUT)
            .expect("Unable to create System State object arg!");

        let voting_power = builder.programmable_move_call(
            IOTA_SYSTEM_PACKAGE_ID,
            IOTA_SYSTEM_MODULE_NAME.to_owned(),
            ident_str!("validator_voting_powers").to_owned(),
            vec![],
            vec![system_state],
        );

        // Hardcoding min stake participation to 75.00%
        // TODO: We need to set a correct value or make this configurable.
        let min_stake_participation_percentage = builder
            .input(CallArg::Pure(
                bcs::to_bytes(&BRIDGE_COMMITTEE_MINIMAL_VOTING_POWER).unwrap(),
            ))
            .unwrap();

        builder.programmable_move_call(
            BRIDGE_ADDRESS.into(),
            BRIDGE_MODULE_NAME.to_owned(),
            BRIDGE_INIT_COMMITTEE_FUNCTION_NAME.to_owned(),
            vec![],
            vec![bridge, voting_power, min_stake_participation_percentage],
        );
        builder
    }

    /// Sets up and executes a `ProgrammableTransaction` to update the
    /// authenticator state. This function constructs a transaction that
    /// invokes the `authenticator_state_update` function from the IOTA
    /// framework, passing the authenticator state object and new active JWKS as
    /// arguments. It then executes the transaction using the system
    /// execution mode.
    fn setup_authenticator_state_update(
        update: AuthenticatorStateUpdateV1,
        temporary_store: &mut TemporaryStore<'_>,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let res = builder.move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                AUTHENTICATOR_STATE_MODULE_NAME.to_owned(),
                AUTHENTICATOR_STATE_UPDATE_FUNCTION_NAME.to_owned(),
                vec![],
                vec![
                    CallArg::Object(ObjectArg::SharedObject {
                        id: IOTA_AUTHENTICATOR_STATE_OBJECT_ID,
                        initial_shared_version: update.authenticator_obj_initial_shared_version,
                        mutable: true,
                    }),
                    CallArg::Pure(bcs::to_bytes(&update.new_active_jwks).unwrap()),
                ],
            );
            assert_invariant!(
                res.is_ok(),
                "Unable to generate authenticator_state_update transaction!"
            );
            builder.finish()
        };
        programmable_transactions::execution::execute::<execution_mode::System>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            tx_ctx,
            gas_charger,
            pt,
        )
    }

    /// Configures a `ProgrammableTransactionBuilder` to expire authenticator
    /// state by invoking the `authenticator_state_expire_jwks` function
    /// from the IOTA framework. The function adds the necessary Move call
    /// with the authenticator state object and the minimum epoch as arguments.
    fn setup_authenticator_state_expire(
        mut builder: ProgrammableTransactionBuilder,
        expire: AuthenticatorStateExpire,
    ) -> ProgrammableTransactionBuilder {
        builder
            .move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                AUTHENTICATOR_STATE_MODULE_NAME.to_owned(),
                AUTHENTICATOR_STATE_EXPIRE_JWKS_FUNCTION_NAME.to_owned(),
                vec![],
                vec![
                    CallArg::Object(ObjectArg::SharedObject {
                        id: IOTA_AUTHENTICATOR_STATE_OBJECT_ID,
                        initial_shared_version: expire.authenticator_obj_initial_shared_version,
                        mutable: true,
                    }),
                    CallArg::Pure(bcs::to_bytes(&expire.min_epoch).unwrap()),
                ],
            )
            .expect("Unable to generate authenticator_state_expire transaction!");
        builder
    }

    /// The function constructs a transaction that invokes
    /// the `randomness_state_update` function from the IOTA framework,
    /// passing the randomness state object, the `randomness_round`,
    /// and the `random_bytes` as arguments. It then executes the transaction
    /// using the system execution mode.
    fn setup_randomness_state_update(
        update: RandomnessStateUpdate,
        temporary_store: &mut TemporaryStore<'_>,
        tx_ctx: &mut TxContext,
        move_vm: &Arc<MoveVM>,
        gas_charger: &mut GasCharger,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
    ) -> Result<(), ExecutionError> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let res = builder.move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                RANDOMNESS_MODULE_NAME.to_owned(),
                RANDOMNESS_STATE_UPDATE_FUNCTION_NAME.to_owned(),
                vec![],
                vec![
                    CallArg::Object(ObjectArg::SharedObject {
                        id: IOTA_RANDOMNESS_STATE_OBJECT_ID,
                        initial_shared_version: update.randomness_obj_initial_shared_version,
                        mutable: true,
                    }),
                    CallArg::Pure(bcs::to_bytes(&update.randomness_round).unwrap()),
                    CallArg::Pure(bcs::to_bytes(&update.random_bytes).unwrap()),
                ],
            );
            assert_invariant!(
                res.is_ok(),
                "Unable to generate randomness_state_update transaction!"
            );
            builder.finish()
        };
        programmable_transactions::execution::execute::<execution_mode::System>(
            protocol_config,
            metrics,
            move_vm,
            temporary_store,
            tx_ctx,
            gas_charger,
            pt,
        )
    }
}
