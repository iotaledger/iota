// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_sdk_types::InputSharedObject;

use crate::types::{iota_address::IotaAddress, object_read::ObjectRead, uint53::UInt53};

/// Details pertaining to shared objects that are referenced by but not changed
/// by a transaction. This information is considered part of the effects,
/// because although the transaction specifies the shared object as input,
/// consensus must schedule it and pick the version that is actually used.
#[derive(Union)]
pub(crate) enum UnchangedSharedObject {
    Read(SharedObjectRead),
    Delete(SharedObjectDelete),
    Cancelled(SharedObjectCancelled),
}

/// The transaction accepted a shared object as input, but only to read it.
#[derive(SimpleObject)]
pub(crate) struct SharedObjectRead {
    #[graphql(flatten)]
    read: ObjectRead,
}

/// The transaction accepted a shared object as input, but it was deleted before
/// the transaction executed.
#[derive(SimpleObject)]
pub(crate) struct SharedObjectDelete {
    /// ID of the shared object.
    address: IotaAddress,

    /// The version of the shared object that was assigned to this transaction
    /// during by consensus, during sequencing.
    version: UInt53,

    /// Whether this transaction intended to use this shared object mutably or
    /// not. See `SharedInput.mutable` for further details.
    mutable: bool,
}

/// The transaction accepted a shared object as input, but its execution was
/// cancelled.
#[derive(SimpleObject)]
pub(crate) struct SharedObjectCancelled {
    /// ID of the shared object.
    address: IotaAddress,

    /// The assigned shared object version. It is a special version indicating
    /// transaction cancellation reason.
    version: UInt53,
}

/// Error for converting from an `InputSharedObject`.
pub(crate) struct SharedObjectChanged;

impl UnchangedSharedObject {
    pub fn try_from(
        input: InputSharedObject,
        checkpoint_viewed_at: u64,
    ) -> Result<Self, SharedObjectChanged> {
        use InputSharedObject as I;
        use UnchangedSharedObject as U;

        match input {
            I::Mutate(_) => Err(SharedObjectChanged),

            I::ReadOnly(oref) => Ok(U::Read(SharedObjectRead {
                read: ObjectRead {
                    native: oref,
                    checkpoint_viewed_at,
                },
            })),

            I::ReadDeleted(object) => Ok(U::Delete(SharedObjectDelete {
                address: object.object_id.into(),
                version: object.version.as_u64().into(),
                mutable: false,
            })),

            I::MutateDeleted(object) => Ok(U::Delete(SharedObjectDelete {
                address: object.object_id.into(),
                version: object.version.as_u64().into(),
                mutable: true,
            })),

            I::Canceled(object) => Ok(U::Cancelled(SharedObjectCancelled {
                address: object.object_id.into(),
                version: object.version.as_u64().into(),
            })),
        }
    }
}
