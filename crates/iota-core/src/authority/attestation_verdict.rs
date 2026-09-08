// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Judges the attestation of a transaction whose Move authentication failed at
//! execution, by re-running authentication at the object versions the attestor
//! recorded. An attestor is charged only when the attestation is refuted.

use std::{collections::BTreeMap, sync::Arc};

use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, GasPayment, ObjectId, ObjectReference, TransactionDigest, TransactionKind, Version,
};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefForExecution, MoveAuthenticatorForExecution,
        authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
        validate_account_object,
    },
    attestation::Attestation,
    auth_context::AuthContextData,
    committee::EpochId,
    error::{ExecutionError, ExecutionErrorKind},
    gas::IotaGasStatus,
    metrics::LimitsMetrics,
    move_authenticator::{MoveAuthenticator, MoveAuthenticatorExt},
    storage::BackingStore,
    transaction::{CheckedInputObjects, InputObjects, ObjectReadResult, ObjectReadResultKind},
};

use crate::execution_cache::ObjectCacheRead;

/// Resolves whether an object version was superseded (overwritten, deleted or
/// wrapped) by a transaction of the current epoch — such versions are retained
/// and can be re-run at.
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
        matches!(
            self.object_cache
                .try_get_object_superseded_in_epoch(object_id, version),
            Ok(Some(epoch)) if epoch == self.current_epoch
        )
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
    pub transaction_kind: TransactionKind,
    pub transaction_signer: Address,
    pub transaction_digest: TransactionDigest,
    pub auth_context_data: AuthContextData,
}

/// The authenticators execution runs, paired with the inputs it loaded.
pub(crate) fn authenticator_inputs(
    authenticators: &[MoveAuthenticatorForExecution<
        Option<AuthenticatorFunctionRefForExecution>,
    >],
) -> Vec<(MoveAuthenticator, InputObjects)> {
    authenticators
        .iter()
        .map(|authenticator| {
            (
                authenticator.authenticator.clone(),
                authenticator.input_objects.inner().clone(),
            )
        })
        .collect()
}

/// The versions execution authenticates against: every authenticator input
/// plus each resolved function-ref field object.
pub(crate) fn executed_versions(
    authenticators: &[MoveAuthenticatorForExecution<
        Option<AuthenticatorFunctionRefForExecution>,
    >],
) -> BTreeMap<ObjectId, Version> {
    authenticators
        .iter()
        .flat_map(|authenticator| {
            authenticator
                .input_objects
                .inner()
                .iter()
                .map(|object| (object.id(), object.version()))
                .chain(authenticator.function_ref.as_ref().map(|function_ref| {
                    (
                        function_ref.loaded_object_id,
                        function_ref.loaded_object_metadata.version,
                    )
                }))
        })
        .collect()
}

impl AttestationVerdictContext<'_> {
    /// Whether the failure refutes the attestation, so it is charged to the
    /// attestor instead of the issuer.
    pub(crate) fn is_refuted(&self) -> bool {
        let reauthenticate = should_reauthenticate(
            self.attestation.object_versions(),
            &self.executed_versions,
            |object_id, version| {
                self.attested_versions
                    .superseded_in_current_epoch(object_id, version)
            },
        );
        !(reauthenticate && self.reauthenticate_at_attested_versions())
    }
}

/// Whether authentication should be re-run at the recorded versions. `false`
/// either when nothing the authentication read drifted, so the failure
/// reproduces at the recorded state, or when a recorded version is refuted
/// without a re-run.
fn should_reauthenticate(
    attested_versions: &[ObjectReference],
    executed_versions: &BTreeMap<ObjectId, Version>,
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
        if attested > executed || !superseded_in_current_epoch(object_ref.object_id(), attested) {
            return false;
        }
        drifted = true;
    }
    drifted
}

impl AttestationVerdictContext<'_> {
    /// Re-runs Move authentication at the recorded versions. Returns whether
    /// the attestation stands: the re-run passes, or cannot judge it.
    fn reauthenticate_at_attested_versions(&self) -> bool {
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
            return false;
        };

        // Resolve every authenticator at the recorded versions first, so the
        // rebuilt auth context carries the same function refs the re-run
        // executes.
        let mut resolved = Vec::with_capacity(self.authenticators.len());
        for (authenticator, input_objects) in &self.authenticators {
            let Some(reloaded_input_objects) =
                self.reload_input_objects_at_attested_versions(input_objects, &attested_versions)
            else {
                return true;
            };

            let Ok((account_id, pinned_version, pinned_digest)) =
                authenticator.object_to_authenticate_components()
            else {
                return false;
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
                return false;
            };
            let Ok(account_version) = validate_account_object(
                account_id,
                pinned_version,
                pinned_digest,
                &authenticator.address(),
                account_object,
            ) else {
                return false;
            };
            let Ok(field_object_id) =
                derive_authenticator_function_ref_v1_dynamic_field_id(account_id)
            else {
                return false;
            };
            let field_object =
                match self
                    .store
                    .read_child_object(&account_id, &field_object_id, account_version)
                {
                    Ok(Some(field_object)) => field_object,
                    // The structural failure reproduces at the recorded state.
                    Ok(None) => return false,
                    Err(_) => return true,
                };
            let Ok(function_ref) =
                authenticator_function_ref_v1_from_dynamic_field_object(account_id, &field_object)
            else {
                return false;
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
        let Ok(aggregated_input_objects) =
            iota_transaction_checks::aggregate_authenticator_input_objects(
                &per_authenticator_input_objects,
            )
        else {
            return true;
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
        let mut auth_context_data = self.auth_context_data.clone();
        auth_context_data.sender_authenticator_function_ref = sender_authenticator_function_ref;
        auth_context_data.sponsor_authenticator_function_ref = sponsor_authenticator_function_ref;

        // No gas coins: the attested budget is charged to nobody.
        let gas_data = GasPayment {
            objects: Vec::new(),
            ..self.gas_data.clone()
        };
        let result = self.executor.authenticate_transaction(
            self.store,
            self.protocol_config,
            self.metrics.clone(),
            &self.epoch_id,
            self.epoch_timestamp_ms,
            gas_data,
            gas_status,
            resolved,
            aggregated_input_objects,
            self.transaction_kind.clone(),
            self.transaction_signer,
            self.transaction_digest,
            auth_context_data,
            &mut None,
        );

        match result {
            Ok(()) => true,
            Err(error) => !is_authentication_rejection(&error),
        }
    }

    /// Rebuilds the input objects for authentication at the versions the
    /// attestor recorded, reusing the executed object when the recorded
    /// version is the one execution loaded.
    ///
    /// Returns `None` when a recorded version cannot be loaded; the drift
    /// check only lets retained versions through, so that is a broken
    /// invariant rather than evidence against the attestor.
    fn reload_input_objects_at_attested_versions(
        &self,
        input_objects: &InputObjects,
        attested_versions: &BTreeMap<ObjectId, &ObjectReference>,
    ) -> Option<InputObjects> {
        let mut reloaded = Vec::with_capacity(input_objects.len());
        for object_read_result in input_objects.iter() {
            let object_id = object_read_result.id();
            match attested_versions.get(&object_id) {
                Some(object_ref) if object_ref.version() != object_read_result.version() => {
                    let object = self
                        .store
                        .get_object_by_key(&object_id, object_ref.version())?;
                    reloaded.push(ObjectReadResult::new(
                        object_read_result.input_object_kind,
                        ObjectReadResultKind::Object(object),
                    ));
                }
                _ => reloaded.push(object_read_result.clone()),
            }
        }
        Some(InputObjects::new(reloaded))
    }
}

/// Whether a re-run failure is the authenticator rejecting the transaction.
/// An invariant violation is the validator's own and cannot judge the attestor.
fn is_authentication_rejection(error: &ExecutionError) -> bool {
    let kind = match error.kind() {
        ExecutionErrorKind::MoveAuthenticationError { error } => error.as_ref(),
        kind => kind,
    };
    !matches!(
        kind,
        ExecutionErrorKind::InvariantViolation | ExecutionErrorKind::VmInvariantViolation
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iota_types::base_types::random_object_ref;

    use super::*;

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
        should_reauthenticate(attested, executed, |object_id, version| {
            superseded.contains(&(*object_id, version))
        })
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
