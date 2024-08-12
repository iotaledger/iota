// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    base_types::ObjectID,
    committee::EpochId,
    error::IotaError,
    governance::StakedIota,
    id::UID,
    object::{Data, Object},
    IOTA_SYSTEM_ADDRESS,
};

pub const TIMELOCKED_STAKED_IOTA_MODULE_NAME: &IdentStr = ident_str!("timelocked_staking");
pub const TIMELOCKED_STAKED_IOTA_STRUCT_NAME: &IdentStr = ident_str!("TimelockedStakedIota");

/// Rust version of the Move
/// stardust::timelocked_staked_iota::TimelockedStakedIota type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TimelockedStakedIota {
    id: UID,
    /// A self-custodial object holding the staked IOTA tokens.
    staked_iota: StakedIota,
    /// This is the epoch time stamp of when the lock expires.
    expiration_timestamp_ms: u64,
    /// Timelock related label.
    label: Option<String>,
}

impl TimelockedStakedIota {
    /// Get the TimeLock's `type`.
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_SYSTEM_ADDRESS,
            module: TIMELOCKED_STAKED_IOTA_MODULE_NAME.to_owned(),
            name: TIMELOCKED_STAKED_IOTA_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }

    /// Is this other StructTag representing a TimelockedStakedIota?
    pub fn is_timelocked_staked_iota(s: &StructTag) -> bool {
        s.address == IOTA_SYSTEM_ADDRESS
            && s.module.as_ident_str() == TIMELOCKED_STAKED_IOTA_MODULE_NAME
            && s.name.as_ident_str() == TIMELOCKED_STAKED_IOTA_STRUCT_NAME
            && s.type_params.is_empty()
    }

    /// Get the TimelockedStakedIota's `id`.
    pub fn id(&self) -> ObjectID {
        self.id.id.bytes
    }

    /// Get the wrapped StakedIota's `pool_id`.
    pub fn pool_id(&self) -> ObjectID {
        self.staked_iota.pool_id()
    }

    /// Get the wrapped StakedIota's `activation_epoch`.
    pub fn activation_epoch(&self) -> EpochId {
        self.staked_iota.activation_epoch()
    }

    /// Get the wrapped StakedIota's `request_epoch`.
    pub fn request_epoch(&self) -> EpochId {
        // TODO: this might change when we implement warm up period.
        self.staked_iota.activation_epoch().saturating_sub(1)
    }

    /// Get the wrapped StakedIota's `principal`.
    pub fn principal(&self) -> u64 {
        self.staked_iota.principal()
    }

    /// Get the TimelockedStakedIota's `expiration_timestamp_ms`.
    pub fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    /// Get the TimelockedStakedIota's `label``.
    pub fn label(&self) -> &Option<String> {
        &self.label
    }
}

impl TryFrom<&Object> for TimelockedStakedIota {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_timelocked_staked_iota() {
                    return bcs::from_bytes(o.contents()).map_err(|err| IotaError::Type {
                        error: format!(
                            "Unable to deserialize TimelockedStakedIota object: {:?}",
                            err
                        ),
                    });
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a TimelockedStakedIota: {:?}", object),
        })
    }
}
