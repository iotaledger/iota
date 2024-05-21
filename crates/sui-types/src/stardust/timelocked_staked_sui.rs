// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::ident_str;
use move_core_types::identifier::IdentStr;
use move_core_types::language_storage::StructTag;

use crate::base_types::ObjectID;
use crate::committee::EpochId;
use crate::error::SuiError;
use crate::governance::StakedSui;
use crate::id::UID;
use crate::object::Data;
use crate::object::Object;
use crate::STARDUST_ADDRESS;
use serde::Deserialize;
use serde::Serialize;

pub const TIMELOCKED_STAKED_SUI_MODULE_NAME: &IdentStr = ident_str!("timelocked_staked_sui");
pub const TIMELOCKED_STAKED_SUI_STRUCT_NAME: &IdentStr = ident_str!("TimelockedStakedSui");

/// Rust version of the Move stardust::timelocked_staked_sui::TimelockedStakedSui type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TimelockedStakedSui {
    id: UID,
    /// A self-custodial object holding the staked SUI tokens.
    staked_sui: StakedSui,
    /// This is the epoch time stamp of when the lock expires.
    expiration_timestamp_ms: u64,
}

impl TimelockedStakedSui {
    /// Get the TimeLock's `type`.
    pub fn type_() -> StructTag {
        StructTag {
            address: STARDUST_ADDRESS,
            module: TIMELOCKED_STAKED_SUI_MODULE_NAME.to_owned(),
            name: TIMELOCKED_STAKED_SUI_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }

    /// Is this other StructTag representing a TimelockedStakedSui?
    pub fn is_timelocked_staked_sui(s: &StructTag) -> bool {
        s.address == STARDUST_ADDRESS
            && s.module.as_ident_str() == TIMELOCKED_STAKED_SUI_MODULE_NAME
            && s.name.as_ident_str() == TIMELOCKED_STAKED_SUI_STRUCT_NAME
            && s.type_params.is_empty()
    }

    /// Get the TimelockedStakedSui's `id`.
    pub fn id(&self) -> ObjectID {
        self.id.id.bytes
    }

    /// Get the wrapped StakedSui's `pool_id`.
    pub fn pool_id(&self) -> ObjectID {
        self.staked_sui.pool_id()
    }

    /// Get the wrapped StakedSui's `activation_epoch`.
    pub fn activation_epoch(&self) -> EpochId {
        self.staked_sui.activation_epoch()
    }

    /// Get the wrapped StakedSui's `request_epoch`.
    pub fn request_epoch(&self) -> EpochId {
        // TODO: this might change when we implement warm up period.
        self.staked_sui.activation_epoch().saturating_sub(1)
    }

    /// Get the wrapped StakedSui's `principal`.
    pub fn principal(&self) -> u64 {
        self.staked_sui.principal()
    }

    /// Get the TimelockedStakedSui's `expiration_timestamp_ms`.
    pub fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }
}

impl TryFrom<&Object> for TimelockedStakedSui {
    type Error = SuiError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_timelocked_staked_sui() {
                    return bcs::from_bytes(o.contents()).map_err(|err| SuiError::TypeError {
                        error: format!(
                            "Unable to deserialize TimelockedStakedSui object: {:?}",
                            err
                        ),
                    });
                }
            }
            Data::Package(_) => {}
        }

        Err(SuiError::TypeError {
            error: format!("Object type is not a TimelockedStakedSui: {:?}", object),
        })
    }
}
