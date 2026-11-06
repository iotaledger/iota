// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Judges the attestation of a transaction whose Move authentication failed at
//! execution, by re-running authentication at the object versions the attestor
//! recorded. The verdict is recorded as `AttestationRecord::verdict` and
//! certified in the checkpoint summary unless `Valid`. When the gas owner's own
//! authentication fails, the judge also decides who pays: it finds who is at
//! fault, as an [`AuthenticationFailureBlame`], and the gas owner pays only when
//! the account is, because authentication passes at the attested state. When
//! the judge cannot decide, the record is
//! [`attestation::AttestationVerdict::Undecided`], certified so that a node
//! executing from a checkpoint charges nobody as well.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iota_common::fatal;
use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, Digest, GasPayment, MoveAuthenticator, ObjectId, ObjectReference, Transaction,
    TransactionDigest, Version,
};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefForExecution,
        authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
        validate_account_object,
    },
    attestation::{self, Attestation},
    auth_context::AuthContextData,
    committee::EpochId,
    error::{ExecutionError, ExecutionErrorKind, IotaError},
    execution::{is_any_invariant_violation, is_cancellation, unwrap_move_authentication_error},
    gas::{IotaGasStatus, IotaGasStatusAPI},
    metrics::LimitsMetrics,
    move_authenticator::MoveAuthenticatorExt,
    storage::BackingStore,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult, ObjectReadResultKind,
        TransactionAPI,
    },
};

use crate::execution_cache::ObjectCacheRead;

/// Resolves whether an object version was superseded (overwritten, deleted or
/// wrapped) by a transaction of the current epoch — such versions stay in the
/// epoch's historic bucket and can be re-run at.
pub(crate) struct AttestedObjectVersions<'a> {
    pub(crate) object_cache: &'a dyn ObjectCacheRead,
    pub(crate) current_epoch: EpochId,
}

impl AttestedObjectVersions<'_> {
    pub(crate) fn superseded_in_current_epoch(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> bool {
        // A read failure must not decide a verdict every validator has to
        // reach alike.
        self.object_cache
            .try_object_superseded_in_epoch(object_id, version, self.current_epoch)
            .unwrap_or_else(|error| {
                fatal!(
                    "cannot read whether {object_id}@{version} was superseded this epoch: {error}"
                )
            })
    }
}

/// What the authority needs to judge an attestation when the attested
/// transaction fails Move authentication at execution.
pub(crate) struct AttestationVerdictContext<'a> {
    pub attestation: &'a Attestation,
    pub attested_versions: AttestedObjectVersions<'a>,
    pub store: &'a dyn BackingStore,
    pub executor: &'a dyn Executor,
    pub protocol_config: &'a ProtocolConfig,
    pub metrics: Arc<LimitsMetrics>,
    pub epoch_id: EpochId,
    pub epoch_timestamp_ms: u64,
    pub reference_gas_price: u64,
    pub gas_data: GasPayment,
    /// Each Move authenticator with the input objects execution loaded for it.
    pub authenticators: Vec<(MoveAuthenticator, InputObjects)>,
    /// Versions of the authenticator inputs and function-ref fields execution
    /// ran against.
    pub executed_versions: BTreeMap<ObjectId, Version>,
    /// Serialized, and its kind cloned, only when a re-run needs them.
    pub transaction: &'a Transaction,
    pub transaction_signer: Address,
    pub transaction_digest: TransactionDigest,
    pub sender_auth_digest: Digest,
    pub sponsor_auth_digest: Option<Digest>,
}

/// The authenticators execution runs, paired with the inputs it loaded.
pub(crate) fn authenticator_inputs(
    authenticators: &[&MoveAuthenticator],
    inputs: &[CheckedInputObjects],
) -> Vec<(MoveAuthenticator, InputObjects)> {
    authenticators
        .iter()
        .zip(inputs)
        .map(|(authenticator, objects)| ((*authenticator).clone(), objects.inner().clone()))
        .collect()
}

/// The versions execution authenticates against: every authenticator input
/// plus each resolved function-ref field object.
pub(crate) fn executed_versions(
    inputs: &[CheckedInputObjects],
    resolutions: &[Result<AuthenticatorFunctionRefForExecution, ExecutionError>],
) -> BTreeMap<ObjectId, Version> {
    inputs
        .iter()
        .zip(resolutions)
        .flat_map(|(objects, resolution)| {
            objects
                .inner()
                .iter()
                .map(|object| (object.id(), object.version()))
                .chain(resolution.as_ref().ok().map(|function_ref| {
                    (
                        function_ref.loaded_object_id,
                        function_ref.loaded_object_metadata.version,
                    )
                }))
        })
        .collect()
}

/// Who is at fault when the Move authentication of an attested transaction
/// fails at execution, as the judge finds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthenticationFailureBlame {
    /// The attestor is at fault. Authentication fails at the versions it
    /// recorded too, or the judge blames it without a re-run, as
    /// [`AttestationVerdictContext::decide_authentication_failure_blame()`]
    /// describes. `rerun_computation_units` is what the judge's re-run consumed,
    /// zero when nothing had to be re-run.
    Attestor { rerun_computation_units: u64 },

    /// The account is at fault. Authentication passes at the recorded versions,
    /// so the account authorized the transaction at the attested state.
    Account,

    /// Nobody is charged or penalized. The failure is an invariant violation,
    /// an internal error, or a cancellation, which the judge does not re-run;
    /// see [`AttestationVerdictContext::decide_authentication_failure_blame()`].
    Nobody,

    /// The judge could not decide who is at fault: its own re-run failed with
    /// an internal error. Recorded as
    /// [`attestation::AttestationVerdict::Undecided`], which is certified, so a
    /// node executing from a checkpoint charges nobody as well.
    Undecided,
}

impl AttestationVerdictContext<'_> {
    /// Decides who is at fault for `kind`, the error of a failed Move
    /// authentication, wrapped in an [`ExecutionErrorKind::MoveAuthentication`]
    /// status or not. Returns [`AuthenticationFailureBlame::Nobody`] for a
    /// cancellation or an invariant violation, without a re-run; otherwise, the
    /// recorded versions decide, through a re-run of authentication at them
    /// when the executed state drifted from them. A deleted authenticator input
    /// is a drifted one: the deletion superseded the recorded version.
    ///
    /// # Panics
    ///
    /// If the recorded state cannot be read. The drift check only admits
    /// versions superseded this epoch, which the epoch's historic bucket keeps,
    /// so a missing or unreadable one is a fault of this node and must not
    /// become a verdict the other validators do not share.
    pub(crate) fn decide_authentication_failure_blame(
        &self,
        kind: &ExecutionErrorKind,
    ) -> AuthenticationFailureBlame {
        let kind = unwrap_move_authentication_error(kind);

        // An invariant violation is an internal error, so nobody is at fault.
        // A cancellation was decided at the consensus commit before any
        // authenticator ran: only a re-run at the recorded versions could tell
        // whether the account authorized the transaction, and that would add
        // load exactly when the network is congested. A Move account therefore
        // pays nothing for a cancelled transaction, while a gas owner that
        // signed with a key does.
        // TODO: decide whether a cancelled transaction charges a Move account.
        if is_cancellation(kind) || is_any_invariant_violation(kind) {
            return AuthenticationFailureBlame::Nobody;
        }

        // Nothing to re-run: the failure already reproduces at the recorded
        // state, or a recorded version refutes on its own.
        let pinned_by_transaction: BTreeSet<ObjectId> = self
            .authenticators
            .iter()
            .flat_map(|(_, input_objects)| input_objects.iter())
            .filter(|input| {
                matches!(
                    input.input_object_kind,
                    InputObjectKind::ImmOrOwnedMoveObject(_)
                )
            })
            .map(|input| input.id())
            .collect();
        if !should_reauthenticate(
            self.attestation.object_versions(),
            &self.executed_versions,
            &pinned_by_transaction,
            |object_id, version| {
                self.attested_versions
                    .superseded_in_current_epoch(object_id, version)
            },
        ) {
            return AuthenticationFailureBlame::Attestor {
                rerun_computation_units: 0,
            };
        }

        // The objects authentication read changed after the attestation, so only
        // a re-run at the recorded versions shows who is at fault.
        self.reauthenticate_at_attested_versions()
    }
}

/// Whether authentication should be re-run at the recorded versions. `false`
/// either when nothing the authentication read drifted, so the failure
/// reproduces at the recorded state, or when a recorded version is refuted
/// without a re-run. `pinned_by_transaction` holds the authenticator inputs
/// whose versions the transaction fixes itself.
fn should_reauthenticate(
    attested_versions: &[ObjectReference],
    executed_versions: &BTreeMap<ObjectId, Version>,
    pinned_by_transaction: &BTreeSet<ObjectId>,
    superseded_in_current_epoch: impl Fn(&ObjectId, Version) -> bool,
) -> bool {
    let mut drifted = false;
    for object_ref in attested_versions {
        let Some(&executed) = executed_versions.get(object_ref.object_id()) else {
            continue;
        };
        let attested = object_ref.version();
        if attested == executed {
            continue;
        }
        // A recorded version ahead of the executed one cannot come from an
        // honest dry run. A recorded version superseded before this epoch was
        // never live this epoch, so an honest dry run could not have seen it.
        // An honest dry run records no input the transaction pins, so a
        // differing version for one is a lie as well; judged on its own, it
        // would send the re-run against an object the transaction never named
        // at that version.
        if attested > executed
            || pinned_by_transaction.contains(object_ref.object_id())
            || !superseded_in_current_epoch(object_ref.object_id(), attested)
        {
            return false;
        }
        drifted = true;
    }
    drifted
}

impl AttestationVerdictContext<'_> {
    /// Re-runs Move authentication at the recorded versions. Returns
    /// [`AuthenticationFailureBlame::Attestor`] when the re-run fails too, with
    /// the units it consumed, or with zero when the claimed units cannot fund a
    /// re-run, an authenticator cannot be resolved at the recorded state, or
    /// the account is immutable where execution rejects one;
    /// [`AuthenticationFailureBlame::Account`] when the re-run passes;
    /// [`AuthenticationFailureBlame::Undecided`] when the authenticators' inputs
    /// cannot be combined, or the re-run ends in a cancellation or an invariant
    /// violation: here, all three can only be internal errors.
    fn reauthenticate_at_attested_versions(&self) -> AuthenticationFailureBlame {
        let attested_versions: BTreeMap<ObjectId, &ObjectReference> = self
            .attestation
            .object_versions()
            .iter()
            .map(|object_ref| (*object_ref.object_id(), object_ref))
            .collect();

        // The re-run is metered on its own, capped by the attestor's claimed
        // computation units: exceeding them refutes the attestation by its own
        // claim, and the cost never reaches the transaction's gas or effects.
        let gas_price = self.gas_data.price;
        let attested_budget = self
            .attestation
            .computation_units()
            .saturating_mul(gas_price);
        let Ok(gas_status) = IotaGasStatus::new(
            attested_budget,
            gas_price,
            self.reference_gas_price,
            self.protocol_config,
        ) else {
            return AuthenticationFailureBlame::Attestor {
                rerun_computation_units: 0,
            };
        };

        // Resolve every authenticator at the recorded versions first, so the
        // rebuilt auth context carries the same function refs the re-run
        // executes.
        let mut resolved = Vec::with_capacity(self.authenticators.len());
        for (authenticator, input_objects) in &self.authenticators {
            let reloaded_input_objects =
                self.reload_input_objects_at_attested_versions(input_objects, &attested_versions);

            let Ok((account_id, pinned_version, pinned_digest)) =
                authenticator.object_to_authenticate_components()
            else {
                return AuthenticationFailureBlame::Attestor {
                    rerun_computation_units: 0,
                };
            };

            // The function ref is resolved from the recorded state, anchored
            // at the account exactly like execution resolves it: deriving the
            // field from the account version means a recorded account/field
            // pair that never coexisted cannot be judged at.
            let Some(account_object) = reloaded_input_objects
                .iter()
                .find(|object| object.id() == account_id)
                .and_then(|object| object.as_object())
            else {
                return AuthenticationFailureBlame::Attestor {
                    rerun_computation_units: 0,
                };
            };
            let Ok(account_version) = validate_account_object(
                account_id,
                pinned_version,
                pinned_digest,
                &authenticator.address(),
                account_object,
            ) else {
                return AuthenticationFailureBlame::Attestor {
                    rerun_computation_units: 0,
                };
            };
            // Execution rejects an immutable account at every state, so no
            // honest dry run authenticated against one.
            if self.protocol_config.reject_immutable_account_objects()
                && account_object.is_immutable()
            {
                return AuthenticationFailureBlame::Attestor {
                    rerun_computation_units: 0,
                };
            }
            let Ok(field_object_id) =
                derive_authenticator_function_ref_v1_dynamic_field_id(account_id)
            else {
                return AuthenticationFailureBlame::Attestor {
                    rerun_computation_units: 0,
                };
            };
            let field_object = match self.store.read_child_object(
                &account_id,
                &field_object_id,
                account_version,
            ) {
                Ok(Some(field_object)) => field_object,
                // The structural failure reproduces at the recorded state:
                // no field, or one that is not the account's child.
                Ok(None) | Err(IotaError::InvalidChildObjectAccess { .. }) => {
                    return AuthenticationFailureBlame::Attestor {
                        rerun_computation_units: 0,
                    };
                }
                Err(error) => fatal!(
                    "cannot read the authenticator field of {account_id}@{account_version}: {error}"
                ),
            };
            let Ok(function_ref) =
                authenticator_function_ref_v1_from_dynamic_field_object(account_id, &field_object)
            else {
                return AuthenticationFailureBlame::Attestor {
                    rerun_computation_units: 0,
                };
            };

            resolved.push((
                authenticator.clone(),
                function_ref.authenticator_function_ref,
                CheckedInputObjects::new_with_checked_transaction_inputs(reloaded_input_objects),
            ));
        }

        let per_authenticator_input_objects: Vec<_> = resolved
            .iter()
            .map(|(_, _, input_objects)| input_objects)
            .collect();
        // Combining the authenticators' inputs fails only when two of them
        // declare the same object with conflicting kinds. The reload keeps the
        // declared kinds, and post-consensus validation already accepted them,
        // so a failure here is an internal error.
        let Ok(aggregated_input_objects) =
            iota_transaction_checks::aggregate_authenticator_input_objects(
                &per_authenticator_input_objects,
            )
        else {
            return AuthenticationFailureBlame::Undecided;
        };

        // Rebuild the auth context with the function refs resolved at the
        // recorded versions.
        let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
            extract_auth_fun_refs(self.transaction_signer, self.gas_data.owner, |address| {
                resolved
                    .iter()
                    .find(|(authenticator, _, _)| authenticator.address() == address)
                    .map(|(_, function_ref, _)| function_ref.clone())
            });
        let auth_context_data = AuthContextData {
            transaction_data_bytes: bcs::to_bytes(self.transaction)
                .expect("Transaction serialization cannot fail"),
            sender_auth_digest: self.sender_auth_digest,
            sponsor_auth_digest: self.sponsor_auth_digest,
            sender_authenticator_function_ref,
            sponsor_authenticator_function_ref,
        };
        let (transaction_kind, _, _) = self.transaction.execution_parts();

        // No gas coins: the attested budget is charged to nobody.
        let gas_data = GasPayment {
            objects: Vec::new(),
            ..self.gas_data.clone()
        };
        let (gas_status, result) = self.executor.authenticate_transaction(
            self.store,
            self.protocol_config,
            self.metrics.clone(),
            &self.epoch_id,
            self.epoch_timestamp_ms,
            gas_data,
            gas_status,
            resolved,
            aggregated_input_objects,
            transaction_kind,
            self.transaction_signer,
            self.transaction_digest,
            auth_context_data,
            &mut None,
        );

        match result {
            Ok(()) => AuthenticationFailureBlame::Account,
            Err(error) => {
                // The re-run carries no cancelled input, so a cancellation
                // here is an internal error, like an invariant violation.
                let kind = unwrap_move_authentication_error(error.kind());
                if is_cancellation(kind) || is_any_invariant_violation(kind) {
                    AuthenticationFailureBlame::Undecided
                } else {
                    AuthenticationFailureBlame::Attestor {
                        rerun_computation_units: consumed_units(gas_status),
                    }
                }
            }
        }
    }

    /// Rebuilds the input objects for authentication at the versions the
    /// attestor recorded, reusing the executed object when the recorded
    /// version is the one execution loaded.
    fn reload_input_objects_at_attested_versions(
        &self,
        input_objects: &InputObjects,
        attested_versions: &BTreeMap<ObjectId, &ObjectReference>,
    ) -> InputObjects {
        let mut reloaded = Vec::with_capacity(input_objects.len());
        for object_read_result in input_objects.iter() {
            let object_id = object_read_result.id();
            match attested_versions.get(&object_id) {
                Some(object_ref) if object_ref.version() != object_read_result.version() => {
                    let version = object_ref.version();
                    // The drift check only admits versions superseded this
                    // epoch, which the epoch's historic bucket keeps.
                    let object = self
                        .attested_versions
                        .object_cache
                        .try_get_object_by_key_with_historic_fallback(
                            &object_id,
                            version,
                            self.attested_versions.current_epoch,
                        )
                        .unwrap_or_else(|error| {
                            fatal!("cannot read {object_id}@{version}: {error}")
                        })
                        .unwrap_or_else(|| {
                            fatal!("{object_id}@{version} was superseded this epoch but is gone")
                        });
                    reloaded.push(ObjectReadResult::new(
                        object_read_result.input_object_kind,
                        ObjectReadResultKind::Object(object),
                    ));
                }
                _ => reloaded.push(object_read_result.clone()),
            }
        }
        InputObjects::new(reloaded)
    }
}

/// The computation units a metered run consumed, bucketized like a charged
/// run; the whole budget when it ran out of gas.
pub(crate) fn consumed_units(mut gas_status: IotaGasStatus) -> u64 {
    // Out of gas pins the computation cost at the budget.
    let _ = gas_status.bucketize_computation();
    attestation::computation_units(&gas_status.summary(), gas_status.gas_price())
}

/// What execution established for the verdict on an attested transaction.
pub(crate) struct ExecutionOutcome {
    /// Who the judge found at fault when the Move authentication of an
    /// attested transaction failed; `None` when it passed or nobody judged.
    pub blame: Option<AuthenticationFailureBlame>,
    /// The computation units execution's own run consumed, as
    /// [`consumed_units()`] counts them, whether or not the effects charge them;
    /// zero without Move authentication.
    pub consumed_units: u64,
    /// The body ran, so its computation cost is measurable against the claim.
    pub body_ran: bool,
}

/// Whether the transaction body ran, so its computation cost measures the same
/// work the attestor's dry run did.
pub(crate) fn body_ran(authentication_failed: bool, error: Option<&ExecutionError>) -> bool {
    !authentication_failed && error.is_none_or(|error| !is_pre_execution_failure(error.kind()))
}

/// The checks that run before the transaction body: the input checks on the
/// authenticator's inputs and the transaction's alike, and the coin deny-list
/// check that follows authentication.
fn is_pre_execution_failure(kind: &ExecutionErrorKind) -> bool {
    matches!(
        kind,
        ExecutionErrorKind::InputObjectDeleted
            | ExecutionErrorKind::AddressDeniedForCoin { .. }
            | ExecutionErrorKind::CoinTypeGlobalPause { .. }
    ) || is_cancellation(kind)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iota_types::base_types::random_object_ref;
    use move_core_types::gas_algebra::InternalGas;

    use super::*;

    fn wrapped(kind: ExecutionErrorKind) -> ExecutionError {
        ExecutionError::from_kind(kind).into_move_authentication_error()
    }

    /// Only a body that ran, successfully or not, is measured against the
    /// claim; failed authentication and pre-execution input checks are not.
    #[test]
    fn body_ran_excludes_authentication_and_input_check_failures() {
        let abort = ExecutionError::from_kind(ExecutionErrorKind::InsufficientGas);
        assert!(body_ran(false, None));
        assert!(body_ran(false, Some(&abort)));
        assert!(!body_ran(
            true,
            Some(&wrapped(ExecutionErrorKind::InsufficientGas))
        ));
        for kind in [
            ExecutionErrorKind::CertificateDenied,
            ExecutionErrorKind::InputObjectDeleted,
            ExecutionErrorKind::ExecutionCanceledDueToRandomnessUnavailable,
            ExecutionErrorKind::ExecutionCanceledDueToSharedObjectCongestion {
                congested_objects: vec![ObjectId::random()],
            },
            ExecutionErrorKind::ExecutionCanceledDueToSharedObjectCongestionV2 {
                congested_objects: vec![ObjectId::random()],
                suggested_gas_price: 1,
            },
            ExecutionErrorKind::ExecutionCanceledDueToExecutionWorkerCongestion {
                suggested_gas_price: 1,
            },
            ExecutionErrorKind::AddressDeniedForCoin {
                address: Address::ZERO,
                coin_type: String::new(),
            },
            ExecutionErrorKind::CoinTypeGlobalPause {
                coin_type: String::new(),
            },
        ] {
            let error = ExecutionError::from_kind(kind);
            assert!(!body_ran(false, Some(&error)), "{error:?}");
        }
    }

    /// An authenticator abort, out-of-gas, unresolved function or deleted
    /// input is the attestor's to foresee; the effects wrapper does not change
    /// that.
    #[test]
    fn authenticator_failures_are_judged() {
        for kind in [
            ExecutionErrorKind::InsufficientGas,
            ExecutionErrorKind::FunctionNotFound,
            ExecutionErrorKind::InputObjectDeleted,
        ] {
            let error = wrapped(kind);
            let kind = unwrap_move_authentication_error(error.kind());
            assert!(!is_cancellation(kind), "{error:?}");
            assert!(!is_any_invariant_violation(kind), "{error:?}");
        }
    }

    /// Deny lists, cancellations and invariant violations are decided after
    /// the dry run and cannot judge the attestor.
    #[test]
    fn failures_the_attestor_cannot_foresee_are_not_judged() {
        for kind in [
            ExecutionErrorKind::CertificateDenied,
            ExecutionErrorKind::ExecutionCanceledDueToSharedObjectCongestion {
                congested_objects: vec![ObjectId::random()],
            },
            ExecutionErrorKind::ExecutionCanceledDueToSharedObjectCongestionV2 {
                congested_objects: vec![ObjectId::random()],
                suggested_gas_price: 1,
            },
            ExecutionErrorKind::ExecutionCanceledDueToExecutionWorkerCongestion {
                suggested_gas_price: 1,
            },
            ExecutionErrorKind::ExecutionCanceledDueToRandomnessUnavailable,
        ] {
            let error = wrapped(kind);
            assert!(
                is_cancellation(unwrap_move_authentication_error(error.kind())),
                "{error:?}"
            );
        }
        for kind in [
            ExecutionErrorKind::InvariantViolation,
            ExecutionErrorKind::VmInvariantViolation,
        ] {
            let error = wrapped(kind);
            assert!(
                is_any_invariant_violation(unwrap_move_authentication_error(error.kind())),
                "{error:?}"
            );
        }
    }

    /// The re-run's units are the bucketized computation in gas units, not
    /// NANOS, and the whole budget when the run exhausted it.
    #[test]
    fn consumed_units_are_bucketized_gas_units_capped_at_the_budget() {
        let config = ProtocolConfig::get_for_max_version_UNSAFE();
        let step = config.gas_rounding_step();
        let (gas_price, reference_gas_price) = (2 * step, step);
        let metered = |budget_units: u64| {
            IotaGasStatus::new(
                budget_units * gas_price,
                gas_price,
                reference_gas_price,
                &config,
            )
            .unwrap()
        };

        // A run below one step rounds up to one step; in NANOS it would be
        // `step * gas_price`.
        let mut gas_status = metered(10 * step);
        gas_status.charge_storage_read(1).unwrap();
        assert_eq!(consumed_units(gas_status), step);

        // Exhausting the budget pins the computation at the budget, which is
        // not a step multiple.
        let budget_units = step + step / 2;
        let mut gas_status = metered(budget_units);
        assert!(
            gas_status
                .move_gas_status_mut()
                .deduct_gas(InternalGas::new(u64::MAX))
                .is_err()
        );
        assert_eq!(consumed_units(gas_status), budget_units);
    }

    fn ref_at(version: u64) -> ObjectReference {
        let base = random_object_ref();
        ObjectReference::new(base.object_id, Version::from(version), base.digest)
    }

    /// Runs the drift check against the versions a test marked as superseded
    /// during the current epoch; anything else is unresolvable.
    fn reauthenticates(
        attested: &[ObjectReference],
        executed: &BTreeMap<ObjectId, Version>,
        superseded: BTreeSet<(ObjectId, Version)>,
    ) -> bool {
        should_reauthenticate(
            attested,
            executed,
            &BTreeSet::new(),
            |object_id, version| superseded.contains(&(*object_id, version)),
        )
    }

    /// An honest dry run records no input the transaction pins, so a recorded
    /// version for one is a lie, however plausible the version.
    #[test]
    fn differing_version_of_a_pinned_input_skips_reauthentication() {
        let input = ref_at(3);
        let superseded: BTreeSet<(ObjectId, Version)> = [(input.object_id, input.version())].into();
        let executed = BTreeMap::from([(input.object_id, Version::from(5u64))]);
        let pinned = BTreeSet::from([input.object_id]);

        assert!(!should_reauthenticate(
            &[input],
            &executed,
            &pinned,
            |object_id, version| superseded.contains(&(*object_id, version))
        ));
    }

    /// Authentication failed against exactly the recorded state, so re-running
    /// would only reproduce it: the attestor vouched for a transaction that
    /// fails at the versions it saw.
    #[test]
    fn no_drift_skips_reauthentication() {
        let account = ref_at(3);
        let executed = BTreeMap::from([(account.object_id, account.version())]);

        assert!(!reauthenticates(&[account], &executed, Default::default()));
    }

    /// The account moved on after an honest attestation, so the recorded state
    /// still has to be checked before anyone is charged.
    #[test]
    fn drift_within_the_epoch_reauthenticates() {
        let account = ref_at(3);
        let superseded = [(account.object_id, account.version())].into();
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(reauthenticates(&[account], &executed, superseded));
    }

    /// A drifted version whose supersession is not from the current epoch is
    /// not state an honest attestor can have read this epoch, and an
    /// attestation is never taken on trust without checking it.
    #[test]
    fn stale_version_skips_reauthentication() {
        let account = ref_at(3);
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(!reauthenticates(&[account], &executed, Default::default()));
    }

    #[test]
    fn version_ahead_of_execution_skips_reauthentication() {
        let account = ref_at(7);
        let superseded = [(account.object_id, account.version())].into();
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(!reauthenticates(&[account], &executed, superseded));
    }

    /// An attestation also records the versions the transaction body read.
    /// Those cannot change whether authentication would have succeeded, so a
    /// stale one must not decide the verdict.
    #[test]
    fn versions_the_reauthentication_does_not_read_are_ignored() {
        let account = ref_at(3);
        let body_object = ref_at(9);
        let superseded = [(account.object_id, account.version())].into();
        let executed = BTreeMap::from([(account.object_id, Version::from(5u64))]);

        assert!(
            reauthenticates(&[account, body_object], &executed, superseded),
            "a stale body-side version must not suppress the re-run"
        );
    }

    /// One refuted version decides the verdict even when another version
    /// drifted honestly.
    #[test]
    fn any_refuted_version_skips_reauthentication() {
        let account = ref_at(3);
        let input = ref_at(4);
        let superseded = [(account.object_id, account.version())].into();
        let executed = BTreeMap::from([
            (account.object_id, Version::from(5u64)),
            (input.object_id, Version::from(2u64)),
        ]);

        assert!(!reauthenticates(&[account, input], &executed, superseded));
    }
}
