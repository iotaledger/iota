// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_ext::types::{Identifier, ObjectData, ObjectId};
use serde::{Deserialize, Serialize};

use crate::{
    balance::Balance,
    committee::EpochId,
    error::IotaError,
    id::{ID, UID},
    object::Object,
};

pub const ADD_STAKE_MUL_COIN_FUN_NAME: Identifier =
    Identifier::from_static("request_add_stake_mul_coin");
pub const ADD_STAKE_FUN_NAME: Identifier = Identifier::from_static("request_add_stake");
pub const WITHDRAW_STAKE_FUN_NAME: Identifier = Identifier::from_static("request_withdraw_stake");

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct StakedIota {
    id: UID,
    pool_id: ID,
    stake_activation_epoch: u64,
    principal: Balance,
}

impl StakedIota {
    pub fn id(&self) -> ObjectId {
        self.id.id.bytes
    }

    pub fn pool_id(&self) -> ObjectId {
        self.pool_id.bytes
    }

    pub fn activation_epoch(&self) -> EpochId {
        self.stake_activation_epoch
    }

    pub fn request_epoch(&self) -> EpochId {
        // TODO: this might change when we implement warm up period.
        self.stake_activation_epoch.saturating_sub(1)
    }

    pub fn principal(&self) -> u64 {
        self.principal.value()
    }
}

impl TryFrom<&Object> for StakedIota {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            ObjectData::Struct(o) => {
                if o.struct_tag().is_staked_iota() {
                    return bcs::from_bytes(o.contents()).map_err(|err| IotaError::Type {
                        error: format!("Unable to deserialize StakedIota object: {err:?}"),
                    });
                }
            }
            ObjectData::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a StakedIota: {object:?}"),
        })
    }
}
