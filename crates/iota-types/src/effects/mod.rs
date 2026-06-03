// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};

use iota_sdk_types::{
    Digest, EpochId, ExecutionStatus, GasCostSummary, IntentScope, ObjectId, Owner,
    UnchangedSharedObject, Version, crypto::Intent,
};
pub use iota_sdk_types::{
    effects::{
        ChangedObject as EffectsObjectChange, IdOperation as IDOperation, ObjectIn, ObjectOut,
        TransactionEffects, TransactionEffectsV1, UnchangedSharedKind,
    },
    events::TransactionEvents,
};
pub use test_effects_builder::TestEffectsBuilder;
use tracing::instrument;

use crate::{
    base_types::{ExecutionDigests, ObjectRef, SequenceNumber},
    committee::Committee,
    crypto::{
        AuthoritySignInfo, AuthoritySignInfoTrait, AuthorityStrongQuorumSignInfo, EmptySignInfo,
        default_hash,
    },
    digests::{TransactionDigest, TransactionEffectsDigest, TransactionEventsDigest},
    error::IotaResult,
    execution::SharedInput,
    message_envelope::{Envelope, Message, TrustedEnvelope, VerifiedEnvelope},
    storage::WriteKind,
};

mod test_effects_builder;
mod v1;

// Since `std::mem::size_of` may not be stable across platforms, we use rough
// constants We need these for estimating effects sizes
// Approximate size of `ObjectRef` type in bytes
pub const APPROX_SIZE_OF_OBJECT_REF: usize = 80;
// Approximate size of `ExecutionStatus` type in bytes
pub const APPROX_SIZE_OF_EXECUTION_STATUS: usize = 144;
// Approximate size of `EpochId` type in bytes
pub const APPROX_SIZE_OF_EPOCH_ID: usize = 10;
// Approximate size of `GasCostSummary` type in bytes
pub const APPROX_SIZE_OF_GAS_COST_SUMMARY: usize = 50;
// Approximate size of `Option<TransactionEventsDigest>` type in bytes
pub const APPROX_SIZE_OF_OPT_TX_EVENTS_DIGEST: usize = 40;
// Approximate size of `TransactionDigest` type in bytes
pub const APPROX_SIZE_OF_TX_DIGEST: usize = 40;
// Approximate size of `Owner` type in bytes
pub const APPROX_SIZE_OF_OWNER: usize = 48;

impl Message for TransactionEffects {
    type DigestType = TransactionEffectsDigest;
    const SCOPE: IntentScope = IntentScope::TransactionEffects;

    fn digest(&self) -> Self::DigestType {
        TransactionEffectsDigest::new(default_hash(self))
    }
}

pub enum ObjectRemoveKind {
    Delete,
    Wrap,
}

/// Description of a shared object that was used as input to a transaction.
///
/// Captures how each shared object was accessed during execution: whether it
/// was mutated, read-only, deleted after mutable or read-only access, or
/// cancelled.
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum InputSharedObject {
    Mutate(ObjectRef),
    ReadOnly(ObjectRef),
    ReadDeleted(ObjectId, Version),
    MutateDeleted(ObjectId, Version),
    Cancelled(ObjectId, Version),
}

impl InputSharedObject {
    pub fn id_and_version(&self) -> (ObjectId, Version) {
        let (object_id, version, ..) = self.object_ref().into_parts();
        (object_id, version)
    }

    pub fn object_ref(&self) -> ObjectRef {
        match self {
            InputSharedObject::Mutate(oref) | InputSharedObject::ReadOnly(oref) => *oref,
            InputSharedObject::ReadDeleted(id, version)
            | InputSharedObject::MutateDeleted(id, version) => {
                ObjectRef::new(*id, *version, Digest::OBJECT_DELETED)
            }
            InputSharedObject::Cancelled(id, version) => {
                ObjectRef::new(*id, *version, Digest::OBJECT_CANCELLED)
            }
        }
    }
}

/// Effect on an individual object, keyed by its [`ObjectId`].
///
/// Describes the input and output version/digest of a single object that was
/// read or modified during transaction execution, along with the
/// [`IDOperation`] that was applied to it. This is a flattened,
/// version-agnostic view derived from the effects via
/// [`TransactionEffectsAPI::object_changes`].
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct ObjectChange {
    pub id: ObjectId,
    pub input_version: Option<Version>,
    pub input_digest: Option<Digest>,
    pub output_version: Option<Version>,
    pub output_digest: Option<Digest>,
    pub id_operation: IDOperation,
}

mod transaction_effects_api {
    pub trait Sealed {}
    impl Sealed for super::TransactionEffects {}
    impl Sealed for super::TransactionEffectsV1 {}
}

/// Version-agnostic accessors for [`TransactionEffects`].
///
/// Sealed; implemented for the enum and each version struct. The enum impl
/// dispatches to the active variant.
pub trait TransactionEffectsAPI: transaction_effects_api::Sealed {
    /// Return the status of the transaction.
    fn status(&self) -> &ExecutionStatus;

    /// Consume `self` and return the owned status of the transaction.
    fn into_status(self) -> ExecutionStatus;

    /// Return the epoch in which this transaction was executed.
    fn epoch(&self) -> EpochId;

    /// Return the `(ObjectId, Version)` pair, at their pre-execution version,
    /// of every object that existed in the store before this transaction
    /// and was modified by it (mutated, wrapped, or deleted).
    fn modified_at_versions(&self) -> Vec<(ObjectId, Version)>;

    /// The version assigned to all output objects (apart from packages).
    fn lamport_version(&self) -> Version;

    /// Metadata of objects prior to modification. This includes any object that
    /// exists in the store prior to this transaction and is modified in
    /// this transaction. It includes objects that are mutated, wrapped and
    /// deleted.
    fn old_object_metadata(&self) -> Vec<(ObjectRef, Owner)>;

    /// Returns the list of sequenced shared objects used in the input.
    /// This is needed in effects because in transaction we only have object ID
    /// for shared objects. Their version and digest can only be figured out
    /// after sequencing. Also provides the use kind to indicate whether the
    /// object was mutated or read-only. It does not include per epoch
    /// config objects since they do not require sequencing. TODO: Rename
    /// this function to indicate sequencing requirement.
    fn input_shared_objects(&self) -> Vec<InputSharedObject>;

    /// Objects (Move objects and packages) newly created by this transaction,
    /// paired with their owner. Excludes objects that were created and then
    /// wrapped within the same transaction.
    fn created(&self) -> Vec<(ObjectRef, Owner)>;

    /// Objects that existed before this transaction and whose contents were
    /// updated by it (in-place mutations and system package upgrades),
    /// reported at their post-execution `(ObjectRef, Owner)`.
    fn mutated(&self) -> Vec<(ObjectRef, Owner)>;

    /// Objects that were wrapped inside another object before this transaction
    /// and have been promoted back to top-level objects in the store by it.
    fn unwrapped(&self) -> Vec<(ObjectRef, Owner)>;

    /// Objects that existed before this transaction and were deleted by it.
    /// References use the post-execution version and the
    /// [`TransactionEffectsDigest::OBJECT_DELETED`] tombstone digest.
    fn deleted(&self) -> Vec<ObjectRef>;

    /// Objects that were unwrapped and then deleted within this same
    /// transaction (i.e. did not exist as top-level objects either before
    /// or after). References use the post-execution version and the
    /// [`TransactionEffectsDigest::OBJECT_DELETED`] tombstone digest.
    fn unwrapped_then_deleted(&self) -> Vec<ObjectRef>;

    /// Objects that existed as top-level objects before this transaction and
    /// have been wrapped inside another object by it (i.e. no longer visible
    /// in the object store as top-level). References use the post-execution
    /// version and the [`TransactionEffectsDigest::OBJECT_WRAPPED`] tombstone
    /// digest.
    fn wrapped(&self) -> Vec<ObjectRef>;

    /// Returns a flattened view of every object change recorded in these
    /// effects: for each touched object, the input and output version/digest
    /// (when present) together with the [`IDOperation`] describing whether
    /// the ID was created, deleted, or unchanged.
    fn object_changes(&self) -> Vec<ObjectChange>;

    /// Returns the post-execution reference and owner of the gas object.
    // TODO: We should consider having this function to return Option.
    // When the gas object is not available (i.e. system transaction), we currently
    // return dummy object ref and owner. This is not ideal.
    fn gas_object(&self) -> (ObjectRef, Owner);

    /// Digest of the events emitted by this transaction, or `None` if it
    /// emitted no events.
    fn events_digest(&self) -> Option<&TransactionEventsDigest>;

    /// Digests of the transactions this one depends on, i.e. transactions
    /// that must be executed before this one for its inputs to be available.
    fn dependencies(&self) -> &[TransactionDigest];

    /// Digest of the transaction that produced these effects.
    fn transaction_digest(&self) -> &TransactionDigest;

    /// Return the gas cost summary of the transaction.
    fn gas_cost_summary(&self) -> &GasCostSummary;

    /// IDs of shared objects that were declared as mutable inputs by the
    /// transaction but had already been deleted at the time of execution.
    fn deleted_mutably_accessed_shared_objects(&self) -> Vec<ObjectId> {
        self.input_shared_objects()
            .into_iter()
            .filter_map(|kind| match kind {
                InputSharedObject::MutateDeleted(id, _) => Some(id),
                InputSharedObject::Mutate(..)
                | InputSharedObject::ReadOnly(..)
                | InputSharedObject::ReadDeleted(..)
                | InputSharedObject::Cancelled(..) => None,
            })
            .collect()
    }

    /// Returns all root shared objects (i.e. not child object) that are
    /// read-only in the transaction.
    fn unchanged_shared_objects(&self) -> Vec<(ObjectId, UnchangedSharedKind)>;
}

/// Test-only mutators and unchecked builders for [`TransactionEffects`] that
/// bypass the normal invariants. Not for production use.
pub trait TransactionEffectsAPIForTesting: TransactionEffectsAPI {
    // All of these should be #[cfg(test)], but they are used by tests in other
    // crates, and dependencies don't get built with cfg(test) set as far as I
    // can tell.
    /// Returns a mutable reference to the execution status, for tests.
    fn status_mut_for_testing(&mut self) -> &mut ExecutionStatus;

    /// Returns a mutable reference to the gas cost summary, for tests.
    fn gas_cost_summary_mut_for_testing(&mut self) -> &mut GasCostSummary;

    /// Returns a mutable reference to the transaction digest, for tests.
    fn transaction_digest_mut_for_testing(&mut self) -> &mut TransactionDigest;

    /// Returns a mutable reference to the dependency list, for tests.
    fn dependencies_mut_for_testing(&mut self) -> &mut Vec<TransactionDigest>;

    /// Records `kind` as an input shared object without validating that it is
    /// consistent with the rest of the effects. For tests only.
    fn unsafe_add_input_shared_object_for_testing(&mut self, kind: InputSharedObject);

    /// Records an entry that represents the pre-execution version of a still
    /// live object, without validating consistency with the rest of the
    /// effects. For tests only.
    fn unsafe_add_deleted_live_object_for_testing(&mut self, object_ref: ObjectRef);

    /// Records a tombstone entry for a deleted object, without validating
    /// consistency with the rest of the effects. For tests only.
    fn unsafe_add_object_tombstone_for_testing(&mut self, object_ref: ObjectRef);
}

mod transaction_effects_ext {
    pub trait Sealed {}
    impl Sealed for super::TransactionEffects {}
}

/// The version-selecting constructor and aggregating queries for the
/// [`TransactionEffects`] enum. Sealed; implemented only for the enum.
pub trait TransactionEffectsExt: transaction_effects_ext::Sealed {
    /// Build effects from the results of executing a transaction under the
    /// V1 protocol shape.
    fn new_from_execution_v1(
        status: ExecutionStatus,
        epoch: EpochId,
        gas_cost_summary: GasCostSummary,
        shared_objects: Vec<SharedInput>,
        loaded_per_epoch_config_objects: BTreeSet<ObjectId>,
        transaction_digest: TransactionDigest,
        lamport_version: SequenceNumber,
        changed_objects: BTreeMap<ObjectId, EffectsObjectChange>,
        gas_object: Option<ObjectId>,
        events_digest: Option<TransactionEventsDigest>,
        dependencies: Vec<TransactionDigest>,
    ) -> Self;

    /// Build empty V1 effects for `transaction_digest`: success status, no
    /// object changes, and no gas object. For tests that need a placeholder
    /// whose effects content is irrelevant, e.g. system transactions.
    fn new_empty_v1(transaction_digest: TransactionDigest) -> Self;

    /// Returns the `(transaction_digest, effects_digest)` pair identifying
    /// this execution.
    fn execution_digests(&self) -> ExecutionDigests;

    /// Return an iterator that iterates through all changed objects, including
    /// mutated, created and unwrapped objects. In other words, all objects
    /// that still exist in the object state after this transaction.
    /// It doesn't include deleted/wrapped objects.
    fn all_changed_objects(&self) -> Vec<(ObjectRef, Owner, WriteKind)>;

    /// Return all objects that existed in the state prior to the transaction
    /// but no longer exist in the state after the transaction.
    /// It includes deleted and wrapped objects, but does not include
    /// unwrapped_then_deleted objects.
    fn all_removed_objects(&self) -> Vec<(ObjectRef, ObjectRemoveKind)>;

    /// Returns all objects that will become a tombstone after this transaction.
    /// This includes deleted, unwrapped_then_deleted and wrapped objects.
    fn all_tombstones(&self) -> Vec<(ObjectId, SequenceNumber)>;

    /// Returns all objects that were created + wrapped in the same transaction.
    fn created_then_wrapped_objects(&self) -> Vec<(ObjectId, SequenceNumber)>;

    /// Return an iterator of mutated objects, but excluding the gas object.
    fn mutated_excluding_gas(&self) -> Vec<(ObjectRef, Owner)>;

    /// Returns all affected objects in this transaction effects.
    /// Affected objects include created, mutated, unwrapped, deleted,
    /// unwrapped_then_deleted, wrapped and input shared objects.
    fn all_affected_objects(&self) -> Vec<ObjectRef>;

    /// Returns a condensed [`TransactionEffectsDebugSummary`] suitable for
    /// logging and inspection.
    fn summary_for_debug(&self) -> TransactionEffectsDebugSummary;

    /// Upper-bound estimate of the serialized size in bytes of effects with
    /// the given number of writes, modifies, and dependencies under the V1
    /// protocol shape.
    fn estimate_size_upperbound_v1(
        num_writes: usize,
        num_modifies: usize,
        num_deps: usize,
    ) -> usize {
        let fixed_sizes = APPROX_SIZE_OF_EXECUTION_STATUS
            + APPROX_SIZE_OF_EPOCH_ID
            + APPROX_SIZE_OF_GAS_COST_SUMMARY
            + APPROX_SIZE_OF_OPT_TX_EVENTS_DIGEST;

        // We store object ref and owner for both old objects and new objects.
        let approx_change_entry_size = 1_000
            + (APPROX_SIZE_OF_OWNER + APPROX_SIZE_OF_OBJECT_REF) * num_writes
            + (APPROX_SIZE_OF_OWNER + APPROX_SIZE_OF_OBJECT_REF) * num_modifies;

        let deps_size = 1_000 + APPROX_SIZE_OF_TX_DIGEST * num_deps;

        fixed_sizes + approx_change_entry_size + deps_size
    }
}

// Helper macro to reduce boilerplate code
macro_rules! delegate_effects_api {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        match $self {
            TransactionEffects::V1(v1) => v1.$method($($arg),*),
            _ => unimplemented!(
                "a new TransactionEffects enum variant was added and needs to be handled"
            ),
        }
    };
}

impl TransactionEffectsAPI for TransactionEffects {
    fn status(&self) -> &ExecutionStatus {
        delegate_effects_api!(self, status)
    }

    fn into_status(self) -> ExecutionStatus {
        delegate_effects_api!(self, into_status)
    }

    fn epoch(&self) -> EpochId {
        delegate_effects_api!(self, epoch)
    }

    fn modified_at_versions(&self) -> Vec<(ObjectId, Version)> {
        delegate_effects_api!(self, modified_at_versions)
    }

    fn lamport_version(&self) -> Version {
        delegate_effects_api!(self, lamport_version)
    }

    fn old_object_metadata(&self) -> Vec<(ObjectRef, Owner)> {
        delegate_effects_api!(self, old_object_metadata)
    }

    fn input_shared_objects(&self) -> Vec<InputSharedObject> {
        delegate_effects_api!(self, input_shared_objects)
    }

    fn created(&self) -> Vec<(ObjectRef, Owner)> {
        delegate_effects_api!(self, created)
    }

    fn mutated(&self) -> Vec<(ObjectRef, Owner)> {
        delegate_effects_api!(self, mutated)
    }

    fn unwrapped(&self) -> Vec<(ObjectRef, Owner)> {
        delegate_effects_api!(self, unwrapped)
    }

    fn deleted(&self) -> Vec<ObjectRef> {
        delegate_effects_api!(self, deleted)
    }

    fn unwrapped_then_deleted(&self) -> Vec<ObjectRef> {
        delegate_effects_api!(self, unwrapped_then_deleted)
    }

    fn wrapped(&self) -> Vec<ObjectRef> {
        delegate_effects_api!(self, wrapped)
    }

    fn object_changes(&self) -> Vec<ObjectChange> {
        delegate_effects_api!(self, object_changes)
    }

    fn gas_object(&self) -> (ObjectRef, Owner) {
        delegate_effects_api!(self, gas_object)
    }

    fn events_digest(&self) -> Option<&TransactionEventsDigest> {
        delegate_effects_api!(self, events_digest)
    }

    fn dependencies(&self) -> &[TransactionDigest] {
        delegate_effects_api!(self, dependencies)
    }

    fn transaction_digest(&self) -> &TransactionDigest {
        delegate_effects_api!(self, transaction_digest)
    }

    fn gas_cost_summary(&self) -> &GasCostSummary {
        delegate_effects_api!(self, gas_cost_summary)
    }

    fn unchanged_shared_objects(&self) -> Vec<(ObjectId, UnchangedSharedKind)> {
        delegate_effects_api!(self, unchanged_shared_objects)
    }
}

impl TransactionEffectsAPIForTesting for TransactionEffects {
    fn status_mut_for_testing(&mut self) -> &mut ExecutionStatus {
        delegate_effects_api!(self, status_mut_for_testing)
    }

    fn gas_cost_summary_mut_for_testing(&mut self) -> &mut GasCostSummary {
        delegate_effects_api!(self, gas_cost_summary_mut_for_testing)
    }

    fn transaction_digest_mut_for_testing(&mut self) -> &mut TransactionDigest {
        delegate_effects_api!(self, transaction_digest_mut_for_testing)
    }

    fn dependencies_mut_for_testing(&mut self) -> &mut Vec<TransactionDigest> {
        delegate_effects_api!(self, dependencies_mut_for_testing)
    }

    fn unsafe_add_input_shared_object_for_testing(&mut self, kind: InputSharedObject) {
        delegate_effects_api!(self, unsafe_add_input_shared_object_for_testing, kind)
    }

    fn unsafe_add_deleted_live_object_for_testing(&mut self, object_ref: ObjectRef) {
        delegate_effects_api!(self, unsafe_add_deleted_live_object_for_testing, object_ref)
    }

    fn unsafe_add_object_tombstone_for_testing(&mut self, object_ref: ObjectRef) {
        delegate_effects_api!(self, unsafe_add_object_tombstone_for_testing, object_ref)
    }
}

impl TransactionEffectsExt for TransactionEffects {
    fn new_from_execution_v1(
        status: ExecutionStatus,
        epoch: EpochId,
        gas_cost_summary: GasCostSummary,
        shared_objects: Vec<SharedInput>,
        loaded_per_epoch_config_objects: BTreeSet<ObjectId>,
        transaction_digest: TransactionDigest,
        lamport_version: SequenceNumber,
        changed_objects: BTreeMap<ObjectId, EffectsObjectChange>,
        gas_object: Option<ObjectId>,
        events_digest: Option<TransactionEventsDigest>,
        dependencies: Vec<TransactionDigest>,
    ) -> Self {
        TransactionEffects::V1(Box::new(v1::new_from_execution(
            status,
            epoch,
            gas_cost_summary,
            shared_objects,
            loaded_per_epoch_config_objects,
            transaction_digest,
            lamport_version,
            changed_objects,
            gas_object,
            events_digest,
            dependencies,
        )))
    }

    fn new_empty_v1(transaction_digest: TransactionDigest) -> Self {
        Self::new_from_execution_v1(
            ExecutionStatus::Success,
            0,
            GasCostSummary::default(),
            vec![],
            BTreeSet::new(),
            transaction_digest,
            SequenceNumber::default(),
            BTreeMap::new(),
            None,
            None,
            vec![],
        )
    }

    fn execution_digests(&self) -> ExecutionDigests {
        ExecutionDigests {
            transaction: *self.transaction_digest(),
            effects: self.digest(),
        }
    }

    fn all_changed_objects(&self) -> Vec<(ObjectRef, Owner, WriteKind)> {
        self.mutated()
            .into_iter()
            .map(|(r, o)| (r, o, WriteKind::Mutate))
            .chain(
                self.created()
                    .into_iter()
                    .map(|(r, o)| (r, o, WriteKind::Create)),
            )
            .chain(
                self.unwrapped()
                    .into_iter()
                    .map(|(r, o)| (r, o, WriteKind::Unwrap)),
            )
            .collect()
    }

    fn all_removed_objects(&self) -> Vec<(ObjectRef, ObjectRemoveKind)> {
        self.deleted()
            .iter()
            .map(|obj_ref| (*obj_ref, ObjectRemoveKind::Delete))
            .chain(
                self.wrapped()
                    .iter()
                    .map(|obj_ref| (*obj_ref, ObjectRemoveKind::Wrap)),
            )
            .collect()
    }

    fn all_tombstones(&self) -> Vec<(ObjectId, SequenceNumber)> {
        self.deleted()
            .into_iter()
            .chain(self.unwrapped_then_deleted())
            .chain(self.wrapped())
            .map(|obj_ref| (obj_ref.object_id, obj_ref.version))
            .collect()
    }

    fn created_then_wrapped_objects(&self) -> Vec<(ObjectId, SequenceNumber)> {
        // Filter `ObjectChange` where:
        // - `input_digest` and `output_digest` are `None`, and
        // - `id_operation` is `Created`.
        self.object_changes()
            .into_iter()
            .filter_map(|change| {
                if change.input_digest.is_none()
                    && change.output_digest.is_none()
                    && change.id_operation == IDOperation::Created
                {
                    Some((change.id, change.output_version.unwrap_or_default()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    fn mutated_excluding_gas(&self) -> Vec<(ObjectRef, Owner)> {
        self.mutated()
            .into_iter()
            .filter(|o| o != &self.gas_object())
            .collect()
    }

    fn all_affected_objects(&self) -> Vec<ObjectRef> {
        self.created()
            .into_iter()
            .map(|(r, _)| r)
            .chain(self.mutated().into_iter().map(|(r, _)| r))
            .chain(self.unwrapped().into_iter().map(|(r, _)| r))
            .chain(
                self.input_shared_objects()
                    .into_iter()
                    .map(|r| r.object_ref()),
            )
            .chain(self.deleted())
            .chain(self.unwrapped_then_deleted())
            .chain(self.wrapped())
            .collect()
    }

    fn summary_for_debug(&self) -> TransactionEffectsDebugSummary {
        TransactionEffectsDebugSummary {
            bcs_size: bcs::serialized_size(self).unwrap(),
            status: self.status().clone(),
            gas_cost_summary: self.gas_cost_summary().clone(),
            transaction_digest: *self.transaction_digest(),
            created_object_count: self.created().len(),
            mutated_object_count: self.mutated().len(),
            unwrapped_object_count: self.unwrapped().len(),
            deleted_object_count: self.deleted().len(),
            wrapped_object_count: self.wrapped().len(),
            dependency_count: self.dependencies().len(),
        }
    }
}

#[derive(Debug)]
pub struct TransactionEffectsDebugSummary {
    /// Size of bcs serialized bytes of the effects.
    pub bcs_size: usize,
    pub status: ExecutionStatus,
    pub gas_cost_summary: GasCostSummary,
    pub transaction_digest: TransactionDigest,
    pub created_object_count: usize,
    pub mutated_object_count: usize,
    pub unwrapped_object_count: usize,
    pub deleted_object_count: usize,
    pub wrapped_object_count: usize,
    pub dependency_count: usize,
    // TODO: Add deleted_and_unwrapped_object_count and event digest.
}

pub type TransactionEffectsEnvelope<S> = Envelope<TransactionEffects, S>;
pub type UnsignedTransactionEffects = TransactionEffectsEnvelope<EmptySignInfo>;
pub type SignedTransactionEffects = TransactionEffectsEnvelope<AuthoritySignInfo>;
pub type CertifiedTransactionEffects = TransactionEffectsEnvelope<AuthorityStrongQuorumSignInfo>;

pub type TrustedSignedTransactionEffects = TrustedEnvelope<TransactionEffects, AuthoritySignInfo>;
pub type VerifiedTransactionEffectsEnvelope<S> = VerifiedEnvelope<TransactionEffects, S>;
pub type VerifiedSignedTransactionEffects = VerifiedTransactionEffectsEnvelope<AuthoritySignInfo>;
pub type VerifiedCertifiedTransactionEffects =
    VerifiedTransactionEffectsEnvelope<AuthorityStrongQuorumSignInfo>;

impl CertifiedTransactionEffects {
    #[instrument(level = "trace", skip_all)]
    pub fn verify_authority_signatures(&self, committee: &Committee) -> IotaResult {
        self.auth_sig().verify_secure(
            self.data(),
            Intent::iota_app(IntentScope::TransactionEffects),
            committee,
        )
    }

    #[instrument(level = "trace", skip_all)]
    pub fn verify(self, committee: &Committee) -> IotaResult<VerifiedCertifiedTransactionEffects> {
        self.verify_authority_signatures(committee)?;
        Ok(VerifiedCertifiedTransactionEffects::new_from_verified(self))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// `<TransactionEffects as Message>::digest` and the SDK's inherent
    /// `TransactionEffects::digest` are defined independently in two crates.
    /// They must agree: `Envelope<TransactionEffects, _>` resolves digests via
    /// the trait, while direct call sites resolve to the inherent. Silent
    /// divergence would split-brain storage and consensus digests.
    #[test]
    fn message_trait_and_effects_digest_match() {
        let effects = TransactionEffects::new_empty_v1(TransactionDigest::default());
        let message_digest = <TransactionEffects as Message>::digest(&effects);
        let effects_digest = effects.digest();
        assert_eq!(message_digest, effects_digest);
    }
}
