// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shims for upstream (`sui-types` / `mysten-common`) items that the adapter
//! relies on but IOTA keeps elsewhere or does not have. Kept local to the
//! `vm2` cut so the shared crates stay untouched.

use std::{fmt::Debug, panic::Location, slice::SliceIndex};

use iota_types::error::{ExecutionError, ExecutionErrorKind, IotaError};

pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type CommandIndex = usize;
pub type IotaErrorKind = IotaError;

pub mod execution_status {
    pub use iota_sdk_types::{CommandArgumentError, PackageUpgradeError, TypeArgumentError};
    pub use iota_types::error::ExecutionErrorKind;

    pub use super::ExecutionFailure;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailure {
    pub error: ExecutionErrorKind,
    pub command: Option<CommandIndex>,
}

impl ExecutionFailure {
    pub fn new(error: ExecutionErrorKind, command: Option<CommandIndex>) -> Self {
        ExecutionFailure { error, command }
    }
}

impl From<ExecutionError> for ExecutionFailure {
    fn from(value: ExecutionError) -> Self {
        ExecutionFailure::new(
            value.kind().clone(),
            value.command().map(|c| c as CommandIndex),
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionErrorMetadata {}

pub trait ExecutionErrorTrait:
    From<ExecutionError> + Debug + std::error::Error + Send + Sync + Sized + 'static
{
    fn new(
        failure: ExecutionFailure,
        source: Option<BoxError>,
        metadata: ExecutionErrorMetadata,
    ) -> Self;

    fn from_execution_failure(failure: ExecutionFailure) -> Self {
        Self::new(failure, None, ExecutionErrorMetadata::default())
    }

    fn from_kind(kind: ExecutionErrorKind) -> Self {
        Self::from_execution_failure(ExecutionFailure::new(kind, None))
    }

    fn new_with_source<E>(kind: ExecutionErrorKind, source: E) -> Self
    where
        E: Into<BoxError>,
    {
        Self::new(
            ExecutionFailure::new(kind, None),
            Some(source.into()),
            ExecutionErrorMetadata::default(),
        )
    }

    fn with_command_index(self, command: CommandIndex) -> Self;
    fn kind(&self) -> &ExecutionErrorKind;
    fn command(&self) -> Option<CommandIndex>;

    fn to_execution_failure(&self) -> ExecutionFailure {
        ExecutionFailure::new(self.kind().clone(), self.command())
    }
}

impl ExecutionErrorTrait for ExecutionError {
    fn new(
        failure: ExecutionFailure,
        source: Option<BoxError>,
        _metadata: ExecutionErrorMetadata,
    ) -> Self {
        let error = ExecutionError::new(failure.error, source);
        match failure.command {
            Some(command) => error.with_command_index(command as u64),
            None => error,
        }
    }

    fn with_command_index(self, command: CommandIndex) -> Self {
        ExecutionError::with_command_index(self, command as u64)
    }

    fn kind(&self) -> &ExecutionErrorKind {
        ExecutionError::kind(self)
    }

    fn command(&self) -> Option<CommandIndex> {
        ExecutionError::command(self).map(|c| c as CommandIndex)
    }
}

pub trait SafeIndex<T> {
    fn safe_get<'a, I>(&'a self, index: I) -> Result<&'a I::Output, ExecutionError>
    where
        I: SliceIndex<[T]>,
        T: 'a;

    fn safe_get_mut<'a, I>(&'a mut self, index: I) -> Result<&'a mut I::Output, ExecutionError>
    where
        I: SliceIndex<[T]>,
        T: 'a;
}

impl<T> SafeIndex<T> for [T] {
    fn safe_get<'a, I>(&'a self, index: I) -> Result<&'a I::Output, ExecutionError>
    where
        I: SliceIndex<[T]>,
        T: 'a,
    {
        self.get(index)
            .ok_or_else(|| ExecutionError::invariant_violation("index out of bounds"))
    }

    fn safe_get_mut<'a, I>(&'a mut self, index: I) -> Result<&'a mut I::Output, ExecutionError>
    where
        I: SliceIndex<[T]>,
        T: 'a,
    {
        self.get_mut(index)
            .ok_or_else(|| ExecutionError::invariant_violation("index out of bounds"))
    }
}

impl<T> SafeIndex<T> for Vec<T> {
    fn safe_get<'a, I>(&'a self, index: I) -> Result<&'a I::Output, ExecutionError>
    where
        I: SliceIndex<[T]>,
        T: 'a,
    {
        self.as_slice().safe_get(index)
    }

    fn safe_get_mut<'a, I>(&'a mut self, index: I) -> Result<&'a mut I::Output, ExecutionError>
    where
        I: SliceIndex<[T]>,
        T: 'a,
    {
        self.as_mut_slice().safe_get_mut(index)
    }
}

/// Like `zip_eq`, but reports a length mismatch through `debug_fatal!` and then
/// stops like `zip` instead of panicking.
pub trait ZipDebugEqIteratorExt: Iterator + Sized {
    #[track_caller]
    fn zip_debug_eq<J: IntoIterator>(self, other: J) -> ZipDebugEq<Self, J::IntoIter> {
        ZipDebugEq {
            a: self,
            b: other.into_iter(),
            finished: false,
            caller: Location::caller(),
        }
    }
}

impl<I: Iterator> ZipDebugEqIteratorExt for I {}

pub struct ZipDebugEq<A, B> {
    a: A,
    b: B,
    finished: bool,
    caller: &'static Location<'static>,
}

impl<A: Iterator, B: Iterator> Iterator for ZipDebugEq<A, B> {
    type Item = (A::Item, B::Item);

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match (self.a.next(), self.b.next()) {
            (Some(a), Some(b)) => Some((a, b)),
            (None, None) => {
                self.finished = true;
                None
            }
            _ => {
                self.finished = true;
                iota_common::debug_fatal!(
                    "zip_debug_eq: iterators have different lengths at {}",
                    self.caller
                );
                None
            }
        }
    }
}

pub const BALANCE_MODULE_NAME: &move_core_types::identifier::IdentStr =
    move_core_types::ident_str!("balance");

pub fn is_system_package(
    addr: impl Into<move_core_types::account_address::AccountAddress>,
) -> bool {
    iota_sdk_types::ObjectId::new(addr.into().into_bytes()).is_system_package()
}

// Gas-model version thresholds, as upstream defines them. IOTA's gas model
// version is 2, so these select the legacy behaviour everywhere.
pub fn use_legacy_abstract_size(gas_model_version: u64) -> bool {
    gas_model_version <= 7
}
pub fn legacy_charge_native_pops_args(gas_model_version: u64) -> bool {
    gas_model_version <= 13
}

/// Either execute the transaction normally or fail it up front with
/// predetermined errors (for example a denied certificate), mirroring the
/// upstream `execution_params` type without the accumulator version.
#[derive(Debug, Clone)]
pub struct ExecutionOrEarlyError {
    early_errors: Option<nonempty::NonEmpty<ExecutionErrorKind>>,
}

impl ExecutionOrEarlyError {
    pub fn ok() -> Self {
        Self { early_errors: None }
    }

    /// Fails the transaction up front with `kind`.
    pub fn early_error(kind: ExecutionErrorKind) -> Self {
        Self::failed(nonempty::nonempty![kind])
    }

    /// Adds `kind` to the errors the transaction fails with up front.
    pub fn push_early_error(&mut self, kind: ExecutionErrorKind) {
        match &mut self.early_errors {
            Some(errors) => errors.push(kind),
            None => self.early_errors = Some(nonempty::nonempty![kind]),
        }
    }

    pub fn failed(errors: nonempty::NonEmpty<ExecutionErrorKind>) -> Self {
        Self {
            early_errors: Some(errors),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.early_errors.is_none()
    }

    pub fn early_errors(&self) -> Option<&nonempty::NonEmpty<ExecutionErrorKind>> {
        self.early_errors.as_ref()
    }

    pub fn into_early_errors(self) -> Option<nonempty::NonEmpty<ExecutionErrorKind>> {
        self.early_errors
    }
}

pub const BALANCE_STRUCT_NAME: &move_core_types::identifier::IdentStr =
    move_core_types::ident_str!("Balance");
pub const RESOLVED_BALANCE_STRUCT: (
    &move_core_types::account_address::AccountAddress,
    &move_core_types::identifier::IdentStr,
    &move_core_types::identifier::IdentStr,
) = (
    &iota_types::IOTA_FRAMEWORK_ADDRESS,
    BALANCE_MODULE_NAME,
    BALANCE_STRUCT_NAME,
);

/// Individual permissions an object input grants to the transaction. Upstream this is part of
/// the object owner (party objects); IOTA only has address-owned, immutable and shared objects,
/// so the permission set is derived from the owner and the input's declared mutability.
#[repr(u8)]
#[derive(Debug, Clone, Eq, Copy, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObjectPermission {
    ImmutableUsage = 1 << 0,
    MutableUsage = 1 << 1,
    Write = 1 << 2,
    Delete = 1 << 3,
    InternalTransfer = 1 << 4,
    PublicTransfer = 1 << 5,
    Wrap = 1 << 6,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd)]
pub struct ObjectPermissions(u64);

impl ObjectPermissions {
    const MUTABLE_PERMISSION_BITS: u64 = (ObjectPermission::Write as u64)
        | (ObjectPermission::Delete as u64)
        | (ObjectPermission::InternalTransfer as u64)
        | (ObjectPermission::PublicTransfer as u64)
        | (ObjectPermission::Wrap as u64);
    const ALL_BITS: u64 = Self::MUTABLE_PERMISSION_BITS
        | (ObjectPermission::MutableUsage as u64)
        | (ObjectPermission::ImmutableUsage as u64);

    pub const IMMUTABLE_USAGE: Self = Self::from_bits(ObjectPermission::ImmutableUsage as u64);
    pub const ALL: Self = Self::from_bits(Self::ALL_BITS);
    /// Shared objects: may be mutated and deleted but not transferred, wrapped or frozen.
    pub const LEGACY_SHARED_OBJECT: Self = Self::from_bits(
        (ObjectPermission::Write as u64)
            | (ObjectPermission::Delete as u64)
            | (ObjectPermission::MutableUsage as u64)
            | (ObjectPermission::ImmutableUsage as u64),
    );

    const fn from_bits(bits: u64) -> Self {
        assert!(bits & !Self::ALL_BITS == 0, "unknown permission bit");
        let has_mutable_perm = bits & Self::MUTABLE_PERMISSION_BITS != 0;
        let has_mutable_usage = bits & (ObjectPermission::MutableUsage as u64) != 0;
        assert!(
            !has_mutable_perm || has_mutable_usage,
            "a mutable permission requires MutableUsage"
        );
        Self(bits)
    }

    pub fn can(&self, permission: ObjectPermission) -> bool {
        let p = permission as u64;
        (self.0 & p) == p
    }

    pub fn can_use_mutably(&self) -> bool {
        self.can(ObjectPermission::MutableUsage)
    }

    pub fn can_use_immutably(&self) -> bool {
        self.can(ObjectPermission::ImmutableUsage)
    }
}

/// Builds the effects status for a failed execution.
pub fn failure_status(error: &impl ExecutionErrorTrait) -> iota_sdk_types::ExecutionStatus {
    let ExecutionFailure { error, command } = error.to_execution_failure();
    iota_sdk_types::ExecutionStatus::new_failure(error, command.map(|c| c as u64))
}

// Translation gas, charged by the static PTB executor before any Move code runs. IOTA history was
// produced without it, so all charges are zero (Sui charges 1/1/1/1/1/1/10 internal gas units).
pub const TRANSLATION_PER_INPUT_BASE_CHARGE: u64 = 0;
pub const TRANSLATION_PURE_INPUT_PER_BYTE_CHARGE: u64 = 0;
pub const TRANSLATION_PER_COMMAND_BASE_CHARGE: u64 = 0;
pub const TRANSLATION_PER_TYPE_NODE_CHARGE: u64 = 0;
pub const TRANSLATION_PER_REFERENCE_NODE_CHARGE: u64 = 0;
pub const TRANSLATION_PER_LIVE_REFERENCE_CHARGE: u64 = 0;
pub const TRANSLATION_PER_LINKAGE_ENTRY_CHARGE: u64 = 0;

/// Whether the static PTB executor charges the Move gas meter for the PTB values it loads, moves,
/// copies and borrows, and for each command. IOTA history was produced without these charges.
pub const CHARGE_PTB_VALUE_OPERATIONS: bool = false;

// Sui protocol-config limits that have no IOTA counterpart, at Sui's current values.
pub const MAX_PTB_LIVE_REFERENCES: u64 = 64;
pub const MAX_PTB_RETURNED_REFERENCES: u64 = 16;
pub const MAX_PTB_TOTAL_RETURNED_REFERENCES: u64 = 256;
pub const PACKAGE_ARENA_SIZE_IN_BYTES: u64 = 10_000_000;
