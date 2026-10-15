// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The authority's attestation work: producing the attestation on the submit
//! path, the post-consensus checks of an attested transaction, and the record
//! of an attested transaction's execution.

use std::{collections::HashSet, sync::Arc};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectId, ObjectReference, Owner, Transaction, TransactionEffects};
use iota_types::{
    account_abstraction::authenticator_function::{
        MoveAuthenticatorForExecution, MoveAuthenticatorsForExecution, extract_auth_fun_refs,
    },
    attestation::{self, AttestationData, AttestationRecord, AttestationVerdict},
    auth_context::AuthContextData,
    committee::EpochId,
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    inner_temporary_store::InnerTemporaryStore,
    move_authenticator::MoveAuthenticatorExt,
    transaction::*,
};

use super::{
    AuthorityState, ValidationOutputs, attestation_verdict::ExecutionOutcome,
    authority_per_epoch_store::AuthorityPerEpochStore,
};
use crate::execution_scheduler::transaction_manager::AttestationSource;

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
        } = self.run_validation_checks(transaction, epoch_store, deny_config, true, true)?;

        let owned_objects = tx_checked_input_objects.inner().filter_owned_objects();

        let epoch_id = epoch_store.epoch_start_config().epoch_data().epoch_id();
        let epoch_start_timestamp = epoch_store
            .epoch_start_config()
            .epoch_data()
            .epoch_start_timestamp();
        let backing_store = self.get_backing_store();
        let (kind, signer, gas_data) = tx.execution_parts();
        let tx_digest = *transaction.digest();

        // Capture the `InnerTemporaryStore` from the dry-run: its object maps are the
        // source of truth for the input versions the attestor observed. The account
        // objects are read outside the store, so their observed versions are captured
        // separately.
        let (inner_temp_store, effects, authentication_failed, account_object_refs) =
            if move_authenticators.is_empty() {
                // No Move authentication, execute directly.
                let (inner_temp_store, _, effects, _) =
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

                let (inner_temp_store, _, effects, _, authentication_error) = epoch_store
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

        // Refuse to attest a transaction when its Move authentication did not succeed.
        // Any other failure (out-of-gas or a Move abort in the transaction body) is the
        // issuer's responsibility and is still attested, sequenced and charged as a
        // failed transaction under the normal flow.
        if authentication_failed {
            return Err(IotaError::Execution(format!(
                "refusing to attest transaction with failed Move authentication: {:?}",
                effects.status()
            )));
        }

        // Step 7: build AttestationData. The claim is in gas units, independent
        // of the gas price the user chose.
        let computation_units =
            attestation::computation_units(effects.gas_cost_summary(), tx.gas_price());

        let object_versions =
            attestable_object_versions(tx, &inner_temp_store, account_object_refs)?;

        Ok((
            AttestationData::V1 {
                computation_units,
                object_versions,
            },
            owned_objects,
        ))
    }

    /// Re-runs the `TransactionDenyConfig` deny checks (disabled features,
    /// denied signers, denied input/receiving objects, denied package
    /// dependencies) for an attested (`UserTransactionV2`) transaction in the
    /// post-consensus phase.
    pub(crate) fn check_transaction_deny_list_for_attested_tx(
        &self,
        transaction: &VerifiedTransaction,
        deny_config: &dyn DenyRuleConfig,
    ) -> IotaResult<()> {
        let tx = transaction.data().transaction();
        iota_transaction_checks::deny::check_transaction_for_validation(
            tx,
            transaction.signatures(),
            &transaction.input_objects()?,
            &tx.receiving_objects(),
            deny_config,
            self.get_backing_package_store().as_ref(),
        )?;
        Ok(())
    }

    pub(crate) fn check_transaction_inputs_exist(
        &self,
        transaction: &VerifiedTransaction,
        epoch: EpochId,
    ) -> IotaResult<()> {
        self.input_loader.read_objects_for_signing(
            Some(transaction.digest()),
            &transaction.collect_all_input_object_kind_for_reading()?,
            &[],
            epoch,
        )?;
        Ok(())
    }
}

/// The record of an attested transaction's execution: the certified verdict
/// when executing from a checkpoint, the judged one when executing from
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
        AttestationSource::Attestation(attestation) => {
            // The claim is only measurable against a body that ran.
            let executed_units = outcome
                .body_ran
                .then(|| attestation::computation_units(effects.gas_cost_summary(), gas_price));
            let verdict = AttestationVerdict::new(
                outcome.refuted,
                attestation.computation_units(),
                executed_units,
                protocol_config.attestor_reward_accuracy_tolerance_percentage_as_option(),
            );
            Some(AttestationRecord::new(attestation, verdict))
        }
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
