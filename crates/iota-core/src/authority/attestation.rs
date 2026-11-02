// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The authority's attestation work: producing the attestation on the submit
//! path and the post-consensus checks of an attested transaction.

use std::{collections::HashSet, sync::Arc};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{MoveAuthenticator, ObjectId, ObjectReference, Owner, Transaction};
use iota_transaction_checks::VerifierLimitsSource;
use iota_types::{
    account_abstraction::authenticator_function::{
        MoveAuthenticatorForExecution, MoveAuthenticatorsForExecution, extract_auth_fun_refs,
    },
    attestation::AttestationData,
    auth_context::AuthContextData,
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    move_authenticator::MoveAuthenticatorExt,
    transaction::{
        CheckedInputObjects, InputObjectKind, InputObjects, ObjectReadResult, TransactionAPI,
        VerifiedTransaction,
    },
};

use crate::authority::{
    AuthorityState, ValidationOutputs, authority_per_epoch_store::AuthorityPerEpochStore,
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
        } = self.run_validation_checks(
            transaction,
            epoch_store,
            deny_config,
            true,
            true,
            VerifierLimitsSource::NodeConfig(&self.config.verifier_signing_config),
        )?;

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
        // source of truth for the input versions the attestor observed.
        let (inner_temp_store, effects, authentication_failed) = if move_authenticators.is_empty() {
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
            (inner_temp_store, effects, false)
        } else {
            // Recover the `AuthenticatorFunctionRefForExecution` for each authenticator and
            // run auth + execution in one pass.

            let funcs_for_exec = move_authenticators
                .iter()
                .zip(per_authenticator_inputs)
                .map(|(ma, (_, account_object))| {
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
                })
                .collect::<IotaResult<Vec<_>>>()?;

            // Union the checked transaction + authenticator input objects for
            // execution. Borrow the checked inputs here before they are consumed
            // by `authenticators_for_exec` below.
            let checked_for_union: Vec<&CheckedInputObjects> =
                std::iter::once(&tx_checked_input_objects)
                    .chain(per_authenticator_checked_inputs.iter().map(|(c, _)| c))
                    .collect();
            let auth_and_tx_checked =
                iota_transaction_checks::aggregate_authenticator_input_objects(&checked_for_union)?;

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
                    kind,
                    signer,
                    tx_digest,
                    auth_context_data,
                    &mut None,
                );
            (inner_temp_store, effects, authentication_error.is_some())
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

        // Build `AttestationData`. `gas_cost_summary().computation_cost` is in
        // NANOS; converted to gas units (`computation_cost / gas_price`) so the
        // claim is independent of the gas price the user chose.
        let computation_units = effects
            .gas_cost_summary()
            .computation_cost
            .checked_div(tx.gas_price())
            .unwrap_or(0);

        // `object_versions` is the evidence base for malicious-attestor detection:
        // `input_objects` (resolved tx + authenticator inputs) and
        // `loaded_runtime_objects` (dynamic fields / children) of the execution's
        // temporary store, both at the pre-execution read version and disjoint by
        // `ObjectId`, minus the owned, gas and receiving objects (pinned by
        // `TransactionData`) and immutable objects / packages (frozen).
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

        let object_versions: Vec<ObjectReference> = inner_temp_store
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
            .filter(|oref| !tx_pinned_ids.contains(&oref.object_id))
            .collect();

        Ok((
            AttestationData::V1 {
                computation_units,
                object_versions,
            },
            owned_objects,
        ))
    }

    /// Like [`AuthorityState::check_transaction_inputs_at_commit`], for an
    /// attested transaction: the input checks execution runs, through the
    /// same function, so the two cannot drift: Move-authenticator inputs, gas
    /// (owner, coin type, balance), the transaction's own inputs and their
    /// union. The receiving objects were read through the commit-indexed
    /// reader, which decides whether they are available; the rest of their
    /// validity and the publish verifier stay at execution, where a failure
    /// produces failure effects.
    pub(crate) fn check_attested_transaction_inputs_at_commit(
        &self,
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        tx: &Transaction,
        tx_input_objects: InputObjects,
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
                authenticator_gas_budget,
                protocol_config,
                reference_gas_price,
            )?;
        Ok((tx_checked_input_objects, per_authenticator_checked_inputs))
    }
}
