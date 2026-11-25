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
    cell::Cell,
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use iota_common::fatal;
use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, Digest, GasPayment, MoveAuthenticator, ObjectId, SenderSignedTransaction,
    TransactionDigest, TransactionEffects, TransactionKind, Version,
};
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefForExecution,
        authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
        validate_account_object,
    },
    attestation::{self, Attestation, SharedInputIndex, list_move_authenticator_shared_objects},
    auth_context::AuthContextData,
    committee::EpochId,
    effects::TransactionEffectsAPI,
    error::{ExecutionError, ExecutionErrorKind, IotaError},
    execution::{is_any_invariant_violation, is_cancellation, unwrap_move_authentication_error},
    gas::{IotaGasStatus, IotaGasStatusAPI},
    metrics::LimitsMetrics,
    move_authenticator::MoveAuthenticatorExt,
    storage::{BackingStore, ObjectKey},
    transaction::{
        CheckedInputObjects, InputObjects, ObjectReadResult, ObjectReadResultKind, TransactionAPI,
    },
};

use crate::{
    authority::authority_per_epoch_store::{
        AuthorityPerEpochStore, ExecutionIndices,
        handler_object_state::{CommitIndex, HandlerProcessedObject},
    },
    execution_cache::{ObjectCacheRead, TransactionCacheRead},
};

/// When a recorded version stopped being current: the commit of the
/// transaction that replaced it, the leader round when that transaction is a
/// commit prologue or a deny-rules update, which carry no commit index, or
/// only the epoch when it is another system transaction, which carries
/// neither, or when no window is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SupersededAt {
    Commit(CommitIndex),
    Round(u64),
    Epoch,
}

/// Finds when a recorded version stopped being current this epoch.
pub(crate) struct AttestedObjectVersions<'a> {
    pub(crate) object_cache: &'a dyn ObjectCacheRead,
    pub(crate) transaction_cache: &'a dyn TransactionCacheRead,
    pub(crate) epoch_store: &'a AuthorityPerEpochStore,
    /// Where consensus first sequenced the attested transaction.
    pub(crate) sequenced_at: ExecutionIndices,
    /// How many commits, or rounds, before `sequenced_at` a supersession may
    /// be; `None` admits any of the epoch.
    pub(crate) window: Option<u64>,
}

impl AttestedObjectVersions<'_> {
    /// When `attested` stopped being current, if a transaction of this epoch
    /// replaced it inside the window.
    ///
    /// # Panics
    ///
    /// If a read fails, or the transaction that replaced `attested` has no
    /// stored body, no row for its gas coin, or does not record its effects:
    /// a verdict every validator has to reach alike must not depend on this
    /// node's storage.
    pub(crate) fn superseded_within_window(
        &self,
        object_id: &ObjectId,
        attested: Version,
    ) -> Option<SupersededAt> {
        let epoch = self.epoch_store.epoch();
        let (replacing, uncommitted) = self
            .object_cache
            .try_find_lowest_version_above_in_epoch(object_id, attested, epoch)
            .unwrap_or_else(|error| {
                fatal!("cannot read the version after {object_id}@{attested}: {error}")
            })?;
        let object = self
            .object_cache
            .try_get_object_by_key_with_historic_fallback(object_id, replacing, epoch)
            .unwrap_or_else(|error| fatal!("cannot read {object_id}@{replacing}: {error}"));
        let digest = match object {
            Some(object) => object.previous_transaction,
            // Deleted: its row names the transaction. A wrap's row names
            // none, but neither a shared object nor a dynamic field is ever
            // wrapped.
            None => self.row(&ObjectKey(*object_id, replacing))?.deleted_by?,
        };
        let effects = self.effects_in_epoch(&digest, uncommitted)?;
        // Consuming another version, or none, means `attested` was never a
        // version of the object, or was superseded before this epoch.
        let consumed = effects
            .modified_at_versions()
            .into_iter()
            .find(|modified| modified.object_id() == object_id)?
            .version();
        if consumed != attested {
            return None;
        }
        // Without a window, being superseded this epoch is all that counts.
        let Some(window) = self.window else {
            return Some(SupersededAt::Epoch);
        };
        let at = self.transaction_position(&digest, &effects);
        within_freshness_window(at, self.sequenced_at, window).then_some(at)
    }

    /// The effects of `digest` when it executed in this epoch, `None` when it
    /// ran in an earlier one, whose records a node may have pruned. Waits for
    /// them when the version naming `digest` is `uncommitted`: a version is
    /// readable before its writer records its effects.
    ///
    /// # Panics
    ///
    /// If a read fails, or the effects waited for are still missing after
    /// ten seconds.
    fn effects_in_epoch(
        &self,
        digest: &TransactionDigest,
        uncommitted: bool,
    ) -> Option<TransactionEffects> {
        // The writer records its effects in the synchronous call that made
        // its outputs readable: a wait this long means that order broke.
        const PAUSE: Duration = Duration::from_millis(1);
        const MAX_PAUSES: u32 = 10_000;

        let epoch = self.epoch_store.epoch();
        let read = || {
            let effects_digest = self
                .transaction_cache
                .try_multi_get_executed_effects_digests_in_epoch(epoch, &[*digest])
                .unwrap_or_else(|error| fatal!("cannot read whether {digest} executed: {error}"))
                .pop()
                .flatten()?;
            self.transaction_cache
                .try_multi_get_effects_in_epoch(epoch, &[effects_digest])
                .unwrap_or_else(|error| fatal!("cannot read the effects of {digest}: {error}"))
                .pop()
                .flatten()
        };
        let mut pauses = 0;
        loop {
            match read() {
                // The committed caches outlive the epoch, so either read can
                // answer for an earlier one.
                Some(effects) => return (effects.epoch() == epoch).then_some(effects),
                None if !uncommitted => return None,
                None if pauses == MAX_PAUSES => fatal!(
                    "no effects for {digest} {:?} after its outputs became readable",
                    PAUSE * MAX_PAUSES
                ),
                // The writer is inside the cache write that records them.
                None => {
                    pauses += 1;
                    std::thread::sleep(PAUSE);
                }
            }
        }
    }

    /// Where `digest` ran: its commit; its round for a commit prologue or a
    /// deny-rules update; only the epoch for any other system transaction.
    ///
    /// # Panics
    ///
    /// If a read fails, or `digest` has no stored body or no row for its gas
    /// coin.
    fn transaction_position(
        &self,
        digest: &TransactionDigest,
        effects: &TransactionEffects,
    ) -> SupersededAt {
        let transaction = self
            .transaction_cache
            .try_multi_get_transaction_blocks_in_epoch(self.epoch_store.epoch(), &[*digest])
            .unwrap_or_else(|error| fatal!("cannot read transaction {digest}: {error}"))
            .pop()
            .flatten()
            .unwrap_or_else(|| fatal!("no transaction {digest} behind its executed effects"));
        match transaction.inner().transaction().kind() {
            TransactionKind::ConsensusCommitPrologueV1(prologue) => {
                return SupersededAt::Round(prologue.round);
            }
            TransactionKind::TransactionDenyRulesUpdate(update) => {
                return SupersededAt::Round(update.round);
            }
            _ if transaction.is_system_tx() => return SupersededAt::Epoch,
            _ => {}
        }
        // Recorded before the outputs of `digest` became readable.
        let gas = effects.gas_object();
        let gas = gas.reference();
        let row = self
            .row(&ObjectKey(gas.object_id, gas.version))
            .unwrap_or_else(|| fatal!("no row for the gas coin of {digest}"));
        SupersededAt::Commit(row.produced_at)
    }

    /// The handler row at `key`.
    ///
    /// # Panics
    ///
    /// If the read fails.
    fn row(&self, key: &ObjectKey) -> Option<HandlerProcessedObject> {
        self.epoch_store
            .handler_processed_object(key)
            .unwrap_or_else(|error| fatal!("cannot read the row of {key:?}: {error}"))
    }
}

/// Whether a version superseded at `at` was current recently enough before
/// `sequenced_at` for an honest dry run to have read it: at or after the
/// window's start. A supersession known only by its epoch cannot be
/// measured, so it is admitted.
fn within_freshness_window(at: SupersededAt, sequenced_at: ExecutionIndices, window: u64) -> bool {
    match at {
        SupersededAt::Commit(commit) => commit >= sequenced_at.sub_dag_index.saturating_sub(window),
        SupersededAt::Round(round) => {
            round >= sequenced_at.last_committed_round.saturating_sub(window)
        }
        SupersededAt::Epoch => true,
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
    /// The signed transaction. The entries of the attestation name objects by
    /// their index in it. Its data is serialized, and its kind cloned, only
    /// when a re-run needs them.
    pub transaction: &'a SenderSignedTransaction,
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
    /// versions superseded this epoch within the freshness window, which the
    /// epoch's historic bucket keeps, so a missing or unreadable one is a fault
    /// of this node and must not become a verdict the other validators do not
    /// share.
    pub(crate) fn decide_authentication_failure_blame(
        &self,
        kind: &ExecutionErrorKind,
    ) -> AuthenticationFailureBlame {
        let kind = unwrap_move_authentication_error(kind);

        // An invariant violation is an internal error, so nobody is at fault.
        // A cancellation was decided at the consensus commit before any
        // authenticator ran. No validator ran one at execution, and judging
        // the transaction would mean running one only to charge for that run.
        if is_cancellation(kind) || is_any_invariant_violation(kind) {
            return AuthenticationFailureBlame::Nobody;
        }

        // Nothing to re-run: the failure already reproduces at the recorded
        // state, or a recorded version refutes on its own.
        let attested_versions = collect_attested_versions(self.attestation, self.transaction);
        if !should_reauthenticate(
            &attested_versions,
            &self.executed_versions,
            |object_id, version| {
                self.attested_versions
                    .superseded_within_window(object_id, version)
                    .is_some()
            },
        ) {
            return AuthenticationFailureBlame::Attestor {
                rerun_computation_units: 0,
            };
        }

        // The objects authentication read changed after the attestation, so only
        // a re-run at the recorded versions shows who is at fault.
        self.reauthenticate_at_attested_versions(attested_versions.into_iter().collect())
    }
}

/// Returns the object id and the version of each entry that `attestation`
/// records, in the order of the record. `transaction` is the signed
/// transaction in which the entries name objects by index, as
/// [`list_move_authenticator_shared_objects()`] lists them for the check of the
/// record at the commit. An entry whose index names no shared object there is
/// left out: the check drops a copy whose record has one.
fn collect_attested_versions(
    attestation: &Attestation,
    transaction: &SenderSignedTransaction,
) -> Vec<(ObjectId, Version)> {
    let object_ids: HashMap<SharedInputIndex, ObjectId> =
        list_move_authenticator_shared_objects(transaction)
            .into_iter()
            .map(|(index, object)| (index, object.object_id))
            .collect();

    attestation
        .shared_object_versions()
        .iter()
        .filter_map(|(index, version)| Some((*object_ids.get(index)?, *version)))
        .collect()
}

/// Whether authentication should be re-run at the recorded versions. `false`
/// either when nothing the authentication read drifted, so the failure
/// reproduces at the recorded state, or when a recorded version is refuted
/// without a re-run.
fn should_reauthenticate(
    attested_versions: &[(ObjectId, Version)],
    executed_versions: &BTreeMap<ObjectId, Version>,
    superseded_within_window: impl Fn(&ObjectId, Version) -> bool,
) -> bool {
    let mut drifted = false;
    for &(object_id, attested) in attested_versions {
        let Some(&executed) = executed_versions.get(&object_id) else {
            continue;
        };
        if attested == executed {
            continue;
        }
        // A recorded version ahead of the executed one cannot come from an
        // honest dry run. A recorded version superseded before the freshness
        // window was not current when an honest dry run could have read it.
        if attested > executed || !superseded_within_window(&object_id, attested) {
            return false;
        }
        drifted = true;
    }
    drifted
}

impl AttestationVerdictContext<'_> {
    /// Re-runs Move authentication at `attested_versions`. The engine meters
    /// the re-run as execution does: the gas owner's authenticator first, and
    /// all of them within `max_auth_gas`. Returns
    /// [`AuthenticationFailureBlame::Attestor`] when the re-run fails too, with
    /// the units it consumed, or with zero when the claimed units cannot fund a
    /// re-run, an authenticator cannot be resolved at the recorded state, or
    /// the account is immutable where execution rejects one;
    /// [`AuthenticationFailureBlame::Account`] when the re-run passes;
    /// [`AuthenticationFailureBlame::Undecided`] when the authenticators' inputs
    /// cannot be combined, or the re-run ends in a cancellation or an invariant
    /// violation: here, all three can only be internal errors.
    fn reauthenticate_at_attested_versions(
        &self,
        attested_versions: BTreeMap<ObjectId, Version>,
    ) -> AuthenticationFailureBlame {
        // The re-run is metered on its own, capped by the attestor's claimed
        // computation units: exceeding them refutes the attestation by its own
        // claim, and the cost never reaches the transaction's gas or effects.
        let gas_price = self.gas_data.price;
        let attested_budget = self
            .attestation
            .computation_units()
            .saturating_mul(gas_price);
        // The engine also holds the authenticators to `max_auth_gas`, as in an
        // honest dry run, so an authentication that passes only above it
        // refutes the attestation too.
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
            transaction_data_bytes: bcs::to_bytes(self.transaction.transaction())
                .expect("Transaction serialization cannot fail"),
            sender_auth_digest: self.sender_auth_digest,
            sponsor_auth_digest: self.sponsor_auth_digest,
            sender_authenticator_function_ref,
            sponsor_authenticator_function_ref,
        };
        let (transaction_kind, _, _) = self.transaction.transaction().execution_parts();

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

        // Tests register the `judge-rerun-internal-error` fail point with a
        // transaction digest to make the re-run of that transaction end in an
        // internal error, which no transaction can cause. A `Cell` keeps the
        // binding immutable in builds where the fail point compiles to
        // nothing.
        let result = Cell::new(result);
        iota_macros::fail_point_arg!("judge-rerun-internal-error", |digest: TransactionDigest| {
            if digest == self.transaction_digest {
                result.set(Err(ExecutionError::invariant_violation(
                    "the judge-rerun-internal-error fail point",
                )));
            }
        });

        match result.into_inner() {
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
        attested_versions: &BTreeMap<ObjectId, Version>,
    ) -> InputObjects {
        let mut reloaded = Vec::with_capacity(input_objects.len());
        for object_read_result in input_objects.iter() {
            let object_id = object_read_result.id();
            match attested_versions.get(&object_id) {
                Some(&version) if version != object_read_result.version() => {
                    // The drift check only admits versions superseded this
                    // epoch, which the epoch's historic bucket keeps.
                    let object = self
                        .attested_versions
                        .object_cache
                        .try_get_object_by_key_with_historic_fallback(
                            &object_id,
                            version,
                            self.epoch_id,
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
            | ExecutionErrorKind::TransactionDependencyFailed { .. }
            | ExecutionErrorKind::AddressDeniedForCoin { .. }
            | ExecutionErrorKind::CoinTypeGlobalPause { .. }
    ) || is_cancellation(kind)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iota_sdk_types::{
        MoveAuthenticatorV1, ProgrammableTransaction, SharedObjectReference, Transaction,
        UserSignature,
    };
    use iota_types::{
        attestation::{AttestationData, SharedInputIndex},
        base_types::random_object_ref,
        transaction::CallArg,
    };
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
            ExecutionErrorKind::TransactionDependencyFailed {
                dependency: TransactionDigest::random(),
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

    /// Returns a new object id, paired with `version`.
    fn object_at(version: u64) -> (ObjectId, Version) {
        (ObjectId::random(), Version::from(version))
    }

    /// Runs the drift check against the versions a test marked as superseded
    /// during the current epoch; anything else is unresolvable.
    fn reauthenticates(
        attested: &[(ObjectId, Version)],
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
        let account = object_at(3);
        let executed = BTreeMap::from([account]);

        assert!(!reauthenticates(&[account], &executed, Default::default()));
    }

    /// The account moved on after an honest attestation, so the recorded state
    /// still has to be checked before anyone is charged.
    #[test]
    fn drift_within_the_epoch_reauthenticates() {
        let account = object_at(3);
        let superseded = [account].into();
        let executed = BTreeMap::from([(account.0, Version::from(5u64))]);

        assert!(reauthenticates(&[account], &executed, superseded));
    }

    /// A drifted version whose supersession is not from the current epoch is
    /// not state an honest attestor can have read this epoch, and an
    /// attestation is never taken on trust without checking it.
    #[test]
    fn stale_version_skips_reauthentication() {
        let account = object_at(3);
        let executed = BTreeMap::from([(account.0, Version::from(5u64))]);

        assert!(!reauthenticates(&[account], &executed, Default::default()));
    }

    #[test]
    fn version_ahead_of_execution_skips_reauthentication() {
        let account = object_at(7);
        let superseded = [account].into();
        let executed = BTreeMap::from([(account.0, Version::from(5u64))]);

        assert!(!reauthenticates(&[account], &executed, superseded));
    }

    /// A recorded version of an object that authentication did not read
    /// cannot change whether authentication would have succeeded, so a stale
    /// one must not decide the verdict.
    #[test]
    fn versions_the_reauthentication_does_not_read_are_ignored() {
        let account = object_at(3);
        let unread_object = object_at(9);
        let superseded = [account].into();
        let executed = BTreeMap::from([(account.0, Version::from(5u64))]);

        assert!(
            reauthenticates(&[account, unread_object], &executed, superseded),
            "a stale version of an unread object must not suppress the re-run"
        );
    }

    /// One refuted version decides the verdict even when another version
    /// drifted honestly.
    #[test]
    fn any_refuted_version_skips_reauthentication() {
        let account = object_at(3);
        let input = object_at(4);
        let superseded = [account].into();
        let executed = BTreeMap::from([
            (account.0, Version::from(5u64)),
            (input.0, Version::from(2u64)),
        ]);

        assert!(!reauthenticates(&[account, input], &executed, superseded));
    }

    /// The window admits a supersession at its floor and inside it; a
    /// prologue's round is measured against the sequencing round, and a
    /// supersession known only by its epoch is always admitted.
    #[test]
    fn freshness_window_admits_recent_supersessions_only() {
        let sequenced_at = |sub_dag_index, last_committed_round| ExecutionIndices {
            last_committed_round,
            sub_dag_index,
            transaction_index: 0,
        };
        assert!(within_freshness_window(
            SupersededAt::Commit(10),
            sequenced_at(20, 25),
            10
        ));
        assert!(within_freshness_window(
            SupersededAt::Commit(15),
            sequenced_at(20, 25),
            10
        ));
        assert!(!within_freshness_window(
            SupersededAt::Commit(9),
            sequenced_at(20, 25),
            10
        ));
        assert!(within_freshness_window(
            SupersededAt::Round(15),
            sequenced_at(20, 25),
            10
        ));
        assert!(!within_freshness_window(
            SupersededAt::Round(14),
            sequenced_at(20, 25),
            10
        ));
        assert!(within_freshness_window(
            SupersededAt::Commit(1),
            sequenced_at(5, 5),
            32
        ));
        assert!(within_freshness_window(
            SupersededAt::Epoch,
            sequenced_at(20, 25),
            10
        ));
    }

    /// Two Move authenticators name the same account: the first authenticates
    /// it, and the second takes it as an argument, next to an immutable
    /// object. Each entry becomes the id of the shared object at its index,
    /// with the recorded version. The two entries for the account are both
    /// kept, with their different versions, and the entries whose index names
    /// the immutable object or nothing are left out.
    #[test]
    fn entries_become_object_ids_in_record_order() {
        let account = SharedObjectReference {
            object_id: ObjectId::random(),
            initial_shared_version: Version::from(1u64),
            mutable: false,
        };
        let immutable_argument = random_object_ref();
        let first_authenticator =
            MoveAuthenticator::V1(MoveAuthenticatorV1::new(vec![], vec![], account));
        let second_authenticator = MoveAuthenticator::V1(MoveAuthenticatorV1::new(
            vec![
                CallArg::Shared(account),
                CallArg::ImmutableOrOwned(immutable_argument),
            ],
            vec![],
            SharedObjectReference {
                object_id: ObjectId::random(),
                ..account
            },
        ));
        let programmable = ProgrammableTransaction {
            inputs: vec![],
            commands: vec![],
        };
        let transaction = SenderSignedTransaction::new(
            Transaction::new_programmable(
                Address::ZERO,
                vec![random_object_ref()],
                programmable,
                1,
                1,
            ),
            vec![
                UserSignature::MoveAuthenticator(first_authenticator),
                UserSignature::MoveAuthenticator(second_authenticator),
            ],
        );
        let attestation = Attestation::new_authority(
            AttestationData::V1 {
                computation_units: 1,
                shared_object_versions: vec![
                    (
                        SharedInputIndex::AuthenticatorArgument {
                            authenticator: 1,
                            argument: 0,
                        },
                        Version::from(3u64),
                    ),
                    (
                        SharedInputIndex::AuthenticatorArgument {
                            authenticator: 1,
                            argument: 1,
                        },
                        Version::from(7u64),
                    ),
                    (
                        SharedInputIndex::AuthenticatorArgument {
                            authenticator: 1,
                            argument: 2,
                        },
                        Version::from(9u64),
                    ),
                    (
                        SharedInputIndex::AuthenticatorAccount { authenticator: 0 },
                        Version::from(4u64),
                    ),
                ],
            },
            0,
        );

        assert_eq!(
            collect_attested_versions(&attestation, &transaction),
            vec![
                (account.object_id, Version::from(3u64)),
                (account.object_id, Version::from(4u64)),
            ]
        );
    }
}
