// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversion of derived balance and object changes into their proto form.
//!
//! The derivation itself lives in [`iota_types::changes`], shared with the
//! other consumers that assemble node-shaped responses.

use iota_grpc_types::v1::transaction as grpc_tx;
use iota_sdk_types::{StructTag, TypeTag};
use iota_types::changes::{DeriveChangesError, DerivedBalanceChange, DerivedObjectChange};

impl From<DeriveChangesError> for crate::error::RpcError {
    fn from(error: DeriveChangesError) -> Self {
        Self::new(
            tonic::Code::FailedPrecondition,
            format!(
                "cannot derive the requested change fields: {error}; \
                 retry without balance_changes/object_changes in the read mask"
            ),
        )
    }
}

/// Convert a derived balance change into its proto form.
pub fn balance_change_to_proto(change: DerivedBalanceChange) -> grpc_tx::BalanceChange {
    grpc_tx::BalanceChange::default()
        .with_owner(change.owner)
        .with_coin_type(&change.coin_type)
        .with_amount(change.amount.to_be_bytes().to_vec())
}

/// A Move object's type is always a struct; the proto carries it as the more
/// general `TypeTag`, matching `BalanceChange.coin_type`.
fn object_type_to_proto(object_type: StructTag) -> iota_grpc_types::v1::types::TypeTag {
    (&TypeTag::Struct(Box::new(object_type))).into()
}

/// Convert a derived object change into its proto form.
pub fn object_change_to_proto(change: DerivedObjectChange) -> grpc_tx::ObjectChange {
    match change {
        DerivedObjectChange::Published {
            package_id,
            version,
            digest,
            modules,
        } => grpc_tx::ObjectChange::default().with_published(
            grpc_tx::ObjectChangePublished::default()
                .with_package_id(package_id)
                .with_version(version.as_u64())
                .with_digest(digest)
                .with_modules(modules),
        ),
        DerivedObjectChange::Mutated {
            sender,
            owner,
            object_type,
            object_id,
            version,
            previous_version,
            digest,
        } => grpc_tx::ObjectChange::default().with_mutated(
            grpc_tx::ObjectChangeMutated::default()
                .with_sender(sender)
                .with_owner(owner)
                .with_object_type(object_type_to_proto(object_type))
                .with_object_id(object_id)
                .with_version(version.as_u64())
                .with_previous_version(previous_version.as_u64())
                .with_digest(digest),
        ),
        DerivedObjectChange::Deleted {
            sender,
            object_type,
            object_id,
            version,
        } => grpc_tx::ObjectChange::default().with_deleted(
            grpc_tx::ObjectChangeDeleted::default()
                .with_sender(sender)
                .with_object_type(object_type_to_proto(object_type))
                .with_object_id(object_id)
                .with_version(version.as_u64()),
        ),
        DerivedObjectChange::Wrapped {
            sender,
            object_type,
            object_id,
            version,
        } => grpc_tx::ObjectChange::default().with_wrapped(
            grpc_tx::ObjectChangeWrapped::default()
                .with_sender(sender)
                .with_object_type(object_type_to_proto(object_type))
                .with_object_id(object_id)
                .with_version(version.as_u64()),
        ),
        DerivedObjectChange::Unwrapped {
            sender,
            owner,
            object_type,
            object_id,
            version,
            digest,
        } => grpc_tx::ObjectChange::default().with_unwrapped(
            grpc_tx::ObjectChangeUnwrapped::default()
                .with_sender(sender)
                .with_owner(owner)
                .with_object_type(object_type_to_proto(object_type))
                .with_object_id(object_id)
                .with_version(version.as_u64())
                .with_digest(digest),
        ),
        DerivedObjectChange::Created {
            sender,
            owner,
            object_type,
            object_id,
            version,
            digest,
        } => grpc_tx::ObjectChange::default().with_created(
            grpc_tx::ObjectChangeCreated::default()
                .with_sender(sender)
                .with_owner(owner)
                .with_object_type(object_type_to_proto(object_type))
                .with_object_id(object_id)
                .with_version(version.as_u64())
                .with_digest(digest),
        ),
    }
}
