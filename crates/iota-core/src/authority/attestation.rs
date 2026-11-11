// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The authority's attestation work: producing the attestation on the submit
//! path, the post-consensus checks of an attested transaction, and the record
//! of an attested transaction's execution.

use std::{
    cell::Cell,
    collections::{BTreeSet, HashSet},
    sync::Arc,
};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    MoveAuthenticator, ObjectId, ObjectReference, Owner, Transaction, TransactionEffects,
};
use iota_types::{
    account_abstraction::authenticator_function::{
        MoveAuthenticatorForExecution, MoveAuthenticatorsForExecution, extract_auth_fun_refs,
    },
    attestation::{
        self, AttestationData, AttestationRecord, AttestationTotals, AttestationVerdict,
    },
    auth_context::AuthContextData,
    base_types::AuthorityName,
    committee::Committee,
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    gas::IotaGasStatus,
    inner_temporary_store::InnerTemporaryStore,
    messages_checkpoint::CheckpointSummaryExt,
    move_authenticator::MoveAuthenticatorExt,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult, ReceivingObjects,
        TransactionAPI, VerifiedTransaction,
    },
};

use crate::{
    authority::{
        AuthorityState, ValidationOutputs,
        attestation_verdict::{AuthenticationFailureBlame, ExecutionOutcome},
        authority_per_epoch_store::{AuthorityPerEpochStore, scorer::MAX_SCORE},
    },
    execution_scheduler::transaction_manager::AttestationSource,
};

impl AuthorityState {
    /// Dry-runs a verified transaction and returns `AttestationData` together
    /// with the owned objects validated during execution, ready to be wrapped
    /// in a validator attestation and attached to the transaction before
    /// submitting it to consensus.
    ///
    /// Shares the validation pipeline with
    /// `handle_transaction_validation_checks`
    /// via [`Self::run_validation_checks`] (deny checks, input object loading,
    /// input checks, coin deny list), and then replaces the
    /// `authenticate_transaction` call with
    /// `authenticate_then_execute_transaction_to_effects` so that Move
    /// authentication and transaction execution run together and produce full
    /// `TransactionEffects` including the accurate `computation_cost`.
    pub(crate) fn attest_transaction(
        &self,
        transaction: &VerifiedTransaction,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        deny_config: &dyn DenyRuleConfig,
    ) -> IotaResult<(AttestationData, Vec<ObjectReference>)> {
        let protocol_config = epoch_store.protocol_config();
        let tx = transaction.data().transaction();

        let ValidationOutputs {
            per_authenticator_inputs,
            move_authenticators,
            gas_status,
            tx_checked_input_objects,
            per_authenticator_checked_inputs,
        } = self.run_validation_checks(transaction, epoch_store, deny_config, true)?;

        let owned_objects = tx_checked_input_objects.inner().filter_owned_objects();

        let epoch_id = epoch_store.epoch_start_config().epoch_data().epoch_id();
        let epoch_start_timestamp = epoch_store
            .epoch_start_config()
            .epoch_data()
            .epoch_start_timestamp();
        let backing_store = self.get_backing_store();
        let (kind, signer, gas_data) = tx.execution_parts();
        let tx_digest = *transaction.digest();

        // Tests register the `attest-ignore-auth-failure` fail point to make a
        // validator attest a transaction whose Move authentication fails, as a
        // faulty validator can, claiming the units the test gives. A `Cell`
        // keeps the binding immutable in builds where the fail point compiles
        // to nothing.
        let units_despite_failed_authentication = Cell::new(None);
        iota_macros::fail_point_arg!("attest-ignore-auth-failure", |units: u64| {
            units_despite_failed_authentication.set(Some(units))
        });

        // The dry run meters authentication and body against the whole budget,
        // as execution does, and a failed authentication is refused unpaid. So
        // the authenticators first run on their own at the signing cap, as at
        // admission: one that fails or exceeds `max_auth_gas` is refused before
        // budget-sized work is done on it.
        if !move_authenticators.is_empty() && units_despite_failed_authentication.get().is_none() {
            let function_refs = extract_auth_fun_refs(signer, gas_data.owner, |address| {
                move_authenticators
                    .iter()
                    .zip(per_authenticator_checked_inputs.iter())
                    .find(|(move_authenticator, _)| move_authenticator.address() == address)
                    .map(|(_, (_, auth_fun_ref))| auth_fun_ref.clone())
            });
            let authenticators = move_authenticators
                .iter()
                .zip(per_authenticator_checked_inputs.iter())
                .map(|(move_authenticator, (input_objects, function_ref))| {
                    (
                        (*move_authenticator).to_owned(),
                        function_ref.clone(),
                        CheckedInputObjects::new_with_checked_transaction_inputs(
                            input_objects.inner().clone(),
                        ),
                    )
                })
                .collect();
            let capped_gas_status = IotaGasStatus::new(
                protocol_config.max_auth_gas(),
                gas_data.price,
                epoch_store.reference_gas_price(),
                protocol_config,
            )?;
            self.authenticate_before_consensus(
                epoch_store,
                transaction,
                capped_gas_status,
                authenticators,
                function_refs,
            )?;
        }

        // Capture the `InnerTemporaryStore` from the dry-run: its object maps are the
        // source of truth for the input versions the attestor observed. The account
        // objects are read outside the store, so their observed versions are captured
        // separately.
        let (inner_temp_store, effects, authentication_failed, account_object_refs) =
            if move_authenticators.is_empty() {
                // No Move authentication, execute directly.
                let (inner_temp_store, _, effects, _, _) =
                    epoch_store.executor().execute_transaction_to_effects(
                        backing_store.as_ref(),
                        protocol_config,
                        self.metrics.limits_metrics.clone(),
                        false, // expensive_checks
                        self.config.certificate_deny_config.certificate_deny_set(),
                        &epoch_id,
                        epoch_start_timestamp,
                        tx_checked_input_objects,
                        gas_data,
                        gas_status,
                        kind,
                        signer,
                        tx_digest,
                        &mut None,
                    );
                // This branch has no Move authentication, so it can never fail auth.
                (inner_temp_store, effects, false, Vec::new())
            } else {
                // Recover the `AuthenticatorFunctionRefForExecution` for each authenticator and
                // run auth + execution in one pass.

                let (account_object_refs, funcs_for_exec): (Vec<_>, Vec<_>) = move_authenticators
                    .iter()
                    .zip(per_authenticator_inputs)
                    .map(|(ma, (_, account_object))| {
                        // Immutable accounts cannot drift, so they are not recorded.
                        let account_object_ref = account_object
                            .as_object()
                            .filter(|object| !object.is_immutable())
                            .map(|object| object.object_ref());
                        let (id, seq, digest) = ma.object_to_authenticate_components()?;
                        let signer_addr = ma.address();
                        self.check_move_account_for_validation(
                            id,
                            seq,
                            digest,
                            account_object,
                            &signer_addr,
                            protocol_config,
                        )
                        .map(|function_ref| (account_object_ref, function_ref))
                    })
                    .collect::<IotaResult<Vec<_>>>()?
                    .into_iter()
                    .unzip();
                let account_object_refs: Vec<_> =
                    account_object_refs.into_iter().flatten().collect();

                // Union the checked transaction + authenticator input objects for
                // execution. Borrow the checked inputs here before they are consumed
                // by `authenticators_for_exec` below.
                let checked_for_union: Vec<&CheckedInputObjects> =
                    std::iter::once(&tx_checked_input_objects)
                        .chain(per_authenticator_checked_inputs.iter().map(|(c, _)| c))
                        .collect();
                let auth_and_tx_checked =
                    iota_transaction_checks::aggregate_authenticator_input_objects(
                        &checked_for_union,
                    )?;

                let authenticators_for_exec = move_authenticators
                    .into_iter()
                    .zip(funcs_for_exec)
                    .zip(per_authenticator_checked_inputs)
                    .map(
                        |((ma, function_ref), (input_objects, _))| MoveAuthenticatorForExecution {
                            authenticator: ma.to_owned(),
                            function_ref,
                            input_objects,
                        },
                    )
                    .collect::<Vec<_>>();

                let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
                    extract_auth_fun_refs(signer, gas_data.owner, |address| {
                        authenticators_for_exec
                            .iter()
                            .find(|a| a.authenticator.address() == address)
                            .map(|a| a.function_ref.authenticator_function_ref.clone())
                    });

                let (sender_auth_digest, sponsor_auth_digest) =
                    transaction.data().compute_auth_digests()?;

                // Serialize the TransactionData for the auth context before decomposing.
                let tx_data_bytes =
                    bcs::to_bytes(tx).expect("TransactionData serialization cannot fail");

                let auth_context_data = AuthContextData {
                    transaction_data_bytes: tx_data_bytes,
                    sender_auth_digest,
                    sponsor_auth_digest,
                    sender_authenticator_function_ref,
                    sponsor_authenticator_function_ref,
                };

                let (inner_temp_store, _, effects, _, _, authentication_error) = epoch_store
                    .executor()
                    .authenticate_then_execute_transaction_to_effects(
                        backing_store.as_ref(),
                        protocol_config,
                        self.metrics.limits_metrics.clone(),
                        false, // expensive_checks
                        self.config.certificate_deny_config.certificate_deny_set(),
                        &epoch_id,
                        epoch_start_timestamp,
                        gas_data,
                        gas_status,
                        MoveAuthenticatorsForExecution::Resolved(authenticators_for_exec),
                        auth_and_tx_checked,
                        None, // gas_owner_pays
                        kind,
                        signer,
                        tx_digest,
                        auth_context_data,
                        &mut None,
                    );
                (
                    inner_temp_store,
                    effects,
                    authentication_error.is_some(),
                    account_object_refs,
                )
            };

        // Build `AttestationData`. The claim is in gas units, independent
        // of the gas price the user chose.
        let computation_units = if authentication_failed {
            // Refuse to attest a transaction when its Move authentication did
            // not succeed. Any other failure (out-of-gas or a Move abort in the
            // transaction body) is the issuer's responsibility and is still
            // attested, sequenced and charged as a failed transaction under
            // the normal flow.
            units_despite_failed_authentication.get().ok_or_else(|| {
                IotaError::Execution(format!(
                    "refusing to attest transaction with failed Move authentication: {:?}",
                    effects.status()
                ))
            })?
        } else {
            attestation::computation_units(effects.gas_cost_summary(), tx.gas_price())
        };

        // Tests register the `attest-object-versions` fail point to make a
        // validator record the versions they give in place of the ones its dry
        // run read, as a faulty validator can.
        let object_versions = Cell::new(attestable_object_versions(
            tx,
            &inner_temp_store,
            account_object_refs,
        )?);
        iota_macros::fail_point_arg!("attest-object-versions", |versions| {
            object_versions.set(versions)
        });

        Ok((
            AttestationData::V1 {
                computation_units,
                object_versions: object_versions.into_inner(),
            },
            owned_objects,
        ))
    }

    /// Like [`AuthorityState::check_transaction_inputs_at_commit`], for an
    /// attested transaction: the input checks execution runs, through the
    /// same function, so the two cannot drift: Move-authenticator inputs, gas
    /// (owner, coin type, balance), the transaction's own inputs and their
    /// union. It also checks the receiving objects the reader loaded, as
    /// admission does; execution does not, since the Move runtime resolves
    /// receives. The publish verifier and the account object stay at
    /// execution, where a failure produces failure effects.
    pub(crate) fn check_attested_transaction_inputs_at_commit(
        &self,
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        tx: &Transaction,
        tx_input_objects: InputObjects,
        tx_receiving_objects: &ReceivingObjects,
        move_authenticators: &[&MoveAuthenticator],
        per_authenticator_inputs: Vec<(InputObjects, ObjectReadResult)>,
    ) -> IotaResult<(CheckedInputObjects, Vec<CheckedInputObjects>)> {
        // Execution meters authentication against `max_auth_gas` only when
        // there is something to authenticate.
        let authenticator_gas_budget = if move_authenticators.is_empty() {
            0
        } else {
            protocol_config.max_auth_gas()
        };
        let per_authenticator_input_objects = per_authenticator_inputs
            .into_iter()
            .map(|(objects, _account)| objects)
            .collect();
        let (_gas_status, per_authenticator_checked_inputs, tx_checked_input_objects) =
            iota_transaction_checks::check_certificate_and_move_authenticator_input(
                tx,
                tx_input_objects,
                per_authenticator_input_objects,
                Some(tx_receiving_objects),
                authenticator_gas_budget,
                protocol_config,
                reference_gas_price,
            )?;
        Ok((tx_checked_input_objects, per_authenticator_checked_inputs))
    }

    /// The validators cut off for the rest of the epoch, as the highest
    /// certified checkpoint summary this node holds shows them; empty when the
    /// cutoff is off or that summary is from an earlier epoch.
    pub(crate) fn cut_off_validators(&self) -> BTreeSet<AuthorityName> {
        let epoch_store = self.load_epoch_store_one_call_per_task();
        let protocol_config = epoch_store.protocol_config();
        let Some(max_refuted_units_squared) =
            protocol_config.attestation_max_refuted_units_squared_as_option()
        else {
            return BTreeSet::new();
        };
        let Ok(Some(checkpoint)) = self
            .get_checkpoint_store()
            .get_highest_verified_checkpoint()
        else {
            return BTreeSet::new();
        };
        if checkpoint.epoch != epoch_store.epoch() {
            return BTreeSet::new();
        }
        match checkpoint.parse_version_specific_data(protocol_config) {
            Ok(Some(data)) => data
                .attestation_totals()
                .map_or_else(BTreeSet::new, |totals| {
                    cut_off_attestors(totals, max_refuted_units_squared, epoch_store.committee())
                }),
            _ => BTreeSet::new(),
        }
    }
}

/// The record of an attested transaction's execution: the certified record
/// when executing from a checkpoint, the judged verdict when executing from
/// consensus, nothing for an unattested transaction.
pub(super) fn attestation_record(
    source: &AttestationSource,
    outcome: &ExecutionOutcome,
    effects: &TransactionEffects,
    gas_price: u64,
    protocol_config: &ProtocolConfig,
) -> Option<AttestationRecord> {
    match source {
        AttestationSource::Unattested => None,
        AttestationSource::Certified(record) => *record,
        AttestationSource::Attestation { attestation, .. } => {
            // The claim is only measurable against a body that ran.
            let executed_units = outcome
                .body_ran
                .then(|| attestation::computation_units(effects.gas_cost_summary(), gas_price));
            let claimed_units = attestation.computation_units();
            let tolerance = protocol_config.attestor_accuracy_tolerance_percentage_as_option();
            let verdict = match outcome.blame {
                Some(AuthenticationFailureBlame::Attestor {
                    rerun_computation_units,
                }) => {
                    // A refuted authentication wasted both runs: execution's
                    // own, whether or not the effects charge it, and the
                    // judge's re-run.
                    let units = outcome
                        .consumed_units
                        .saturating_add(rerun_computation_units);

                    AttestationVerdict::new(Some(units), claimed_units, executed_units, tolerance)
                }
                // Certified, so a node executing from the checkpoint charges
                // nobody, as the validators did.
                Some(AuthenticationFailureBlame::Undecided) => AttestationVerdict::Undecided,
                Some(AuthenticationFailureBlame::Account | AuthenticationFailureBlame::Nobody)
                | None => AttestationVerdict::new(None, claimed_units, executed_units, tolerance),
            };

            Some(AttestationRecord::new(attestation, verdict))
        }
    }
}

/// Whether `units_squared` takes the attestor's whole score, which also
/// stops its attestations for the rest of the epoch.
fn takes_whole_score(units_squared: u64, max_refuted_units_squared: u64) -> bool {
    units_squared >= max_refuted_units_squared.max(1)
}

/// The attestors whose `totals` take their whole score.
pub(crate) fn attestors_taking_whole_score(
    totals: &AttestationTotals,
    max_refuted_units_squared: u64,
) -> BTreeSet<u8> {
    totals
        .refuted_units_squared
        .iter()
        .filter(|&(_, &units_squared)| takes_whole_score(units_squared, max_refuted_units_squared))
        .map(|(&attestor, _)| attestor)
        .collect()
}

/// The committee members whose `totals` take their whole score.
fn cut_off_attestors(
    totals: &AttestationTotals,
    max_refuted_units_squared: u64,
    committee: &Committee,
) -> BTreeSet<AuthorityName> {
    attestors_taking_whole_score(totals, max_refuted_units_squared)
        .into_iter()
        .filter_map(|attestor| committee.authority_by_index(u32::from(attestor)).copied())
        .collect()
}

/// Cuts each attestor's score by the squared units its refuted attestations
/// wasted, relative to `max_refuted_units_squared`: a sum that large takes the
/// whole score, a smaller one its fraction, and zero takes the whole score on
/// any fault; the score floors at zero. `scores` and the totals are both
/// indexed by authority index.
///
/// # Panics
///
/// If a refuted attestor index is beyond `scores`.
pub(crate) fn apply_fault_penalties(
    scores: &mut [u64],
    totals: &AttestationTotals,
    max_refuted_units_squared: u64,
) {
    let full_cut = u128::from(max_refuted_units_squared.max(1));
    for (&attestor, &units_squared) in &totals.refuted_units_squared {
        let cut = u128::from(MAX_SCORE) * u128::from(units_squared).min(full_cut) / full_cut;
        let score = &mut scores[usize::from(attestor)];
        *score = score.saturating_sub(u64::try_from(cut).unwrap_or(u64::MAX));
    }
}

/// Versions of the objects an attestation records to be used as the evidence
/// base for detecting a false attestation. `account_object_refs` carries the
/// Move-authenticator account objects, which are read outside the temporary
/// store.
fn attestable_object_versions(
    tx: &Transaction,
    inner_temp_store: &InnerTemporaryStore,
    account_object_refs: Vec<ObjectReference>,
) -> IotaResult<Vec<ObjectReference>> {
    let tx_pinned_ids: HashSet<ObjectId> = tx
        .input_objects()?
        .into_iter()
        .filter_map(|kind| match kind {
            InputObjectKind::ImmOrOwnedMoveObject(oref) => Some(oref.object_id),
            InputObjectKind::MovePackage(_) | InputObjectKind::SharedMoveObject { .. } => None,
        })
        .chain(tx.gas().iter().map(|oref| oref.object_id))
        .chain(
            tx.receiving_objects()
                .into_iter()
                .map(|oref| oref.object_id),
        )
        .collect();

    Ok(inner_temp_store
        .input_objects
        .values()
        .filter(|obj| !obj.is_immutable())
        .map(|obj| obj.object_ref())
        .chain(
            inner_temp_store
                .loaded_runtime_objects
                .iter()
                .filter(|(_, meta)| !matches!(meta.owner, Owner::Immutable))
                .map(|(id, meta)| ObjectReference::new(*id, meta.version, meta.digest)),
        )
        .chain(account_object_refs)
        .filter(|oref| !tx_pinned_ids.contains(&oref.object_id))
        .collect())
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{GasCostSummary, TransactionDigest};
    use iota_types::effects::TransactionEffectsExtForTesting;

    use super::*;
    use crate::authority::authority_per_epoch_store::ExecutionIndices;

    /// A refuted record counts the units execution's own run consumed plus the
    /// judge's re-run, even when the effects charge nothing.
    #[test]
    fn refuted_record_units_do_not_depend_on_the_uncharged_effects() {
        let attestation = attestation::Attestation::new_authority(
            AttestationData::V1 {
                computation_units: 1_000,
                object_versions: vec![],
            },
            0,
        );
        let effects = TransactionEffects::new_empty_v1_for_testing(TransactionDigest::default());
        assert_eq!(effects.gas_cost_summary(), &GasCostSummary::default());
        let outcome = ExecutionOutcome {
            blame: Some(AuthenticationFailureBlame::Attestor {
                rerun_computation_units: 7,
            }),
            consumed_units: 30,
            body_ran: false,
        };

        let record = attestation_record(
            &AttestationSource::Attestation {
                attestation,
                sequenced_at: ExecutionIndices::default(),
            },
            &outcome,
            &effects,
            1,
            &ProtocolConfig::get_for_max_version_UNSAFE(),
        );

        assert_eq!(
            record,
            Some(AttestationRecord {
                attestor: 0,
                verdict: AttestationVerdict::Refuted { units: 37 },
            })
        );
    }

    /// When the judge could not decide, the record is undecided whatever the
    /// units, so it is certified and a node executing from the checkpoint
    /// charges nobody as well.
    #[test]
    fn undecided_blame_is_recorded_undecided() {
        let attestation = attestation::Attestation::new_authority(
            AttestationData::V1 {
                computation_units: 1_000,
                object_versions: vec![],
            },
            0,
        );
        let effects = TransactionEffects::new_empty_v1_for_testing(TransactionDigest::default());
        let outcome = ExecutionOutcome {
            blame: Some(AuthenticationFailureBlame::Undecided),
            consumed_units: 30,
            body_ran: false,
        };

        let record = attestation_record(
            &AttestationSource::Attestation {
                attestation,
                sequenced_at: ExecutionIndices::default(),
            },
            &outcome,
            &effects,
            1,
            &ProtocolConfig::get_for_max_version_UNSAFE(),
        );

        assert_eq!(
            record,
            Some(AttestationRecord {
                attestor: 0,
                verdict: AttestationVerdict::Undecided,
            })
        );
    }

    /// A fault whose squared units reach `max_refuted_units_squared` takes the
    /// whole score, a smaller one the square of its fraction, and faults beyond
    /// that saturate.
    #[test]
    fn fault_penalties_scale_with_the_square_of_the_wasted_units() {
        // One fault of 10 units takes the whole score.
        const MAX_REFUTED_UNITS_SQUARED: u64 = 100;
        let record = |attestor, verdict| Some(AttestationRecord { attestor, verdict });
        let refuted = |units| AttestationVerdict::Refuted { units };
        let totals = AttestationTotals::default().with_records(&[
            // A full fault: the whole score.
            record(0, refuted(10)),
            // Half a fault: a quarter of the score.
            record(2, refuted(5)),
            // Far beyond a full fault: still the whole score, nothing more.
            record(3, refuted(100)),
            record(1, AttestationVerdict::Valid),
            None,
        ]);

        let mut scores = vec![MAX_SCORE; 4];
        apply_fault_penalties(&mut scores, &totals, MAX_REFUTED_UNITS_SQUARED);
        assert_eq!(scores, vec![0, MAX_SCORE, MAX_SCORE - MAX_SCORE / 4, 0]);
    }

    /// The cutoff threshold is where the epoch-end cut takes the whole score.
    #[test]
    fn cutoff_matches_the_whole_score_cut() {
        for (max_refuted_units_squared, units_squared, whole) in [
            (100, 99, false),
            (100, 100, true),
            // Zero takes the whole score on any fault.
            (0, 0, false),
            (0, 1, true),
        ] {
            assert_eq!(
                takes_whole_score(units_squared, max_refuted_units_squared),
                whole
            );
            let totals = AttestationTotals {
                refuted_units_squared: [(0, units_squared)].into(),
            };
            let mut scores = vec![MAX_SCORE; 4];
            apply_fault_penalties(&mut scores, &totals, max_refuted_units_squared);
            assert_eq!(scores[0] == 0, whole);
        }
    }

    /// Only the members whose totals reach the threshold are cut off.
    #[test]
    fn cut_off_attestors_are_the_members_taking_the_whole_score() {
        let (committee, _) = Committee::new_simple_test_committee();
        let totals = AttestationTotals {
            refuted_units_squared: [(0, 100), (1, 99)].into(),
        };
        let first = *committee.authority_by_index(0).unwrap();
        assert_eq!(
            cut_off_attestors(&totals, 100, &committee),
            BTreeSet::from([first])
        );
    }
}
