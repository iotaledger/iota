// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;
use std::fmt::{Display, Formatter, Result};

use iota_types::{iota_serde::IotaTypeTag, object::Owner};
use move_core_types::language_storage::TypeTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BalanceChange {
    /// Owner of the balance change
    pub owner: Owner,
    #[schemars(with = "String")]
    #[serde_as(as = "IotaTypeTag")]
    pub coin_type: TypeTag,
    /// The amount indicate the balance value changes,
    /// negative amount means spending coin value and positive means receiving
    /// coin value.
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub amount: i128,
}

impl Display for BalanceChange {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            " ┌──\n │ Owner: {} \n │ CoinType: {} \n │ Amount: {}\n └──",
            self.owner, self.coin_type, self.amount
        )
    }
}

pub fn from_sdk_balance_change(
    balance_change: iota_sdk_types::BalanceChange,
    is_object_owner: bool,
) -> BalanceChange {
    BalanceChange {
        owner: if is_object_owner {
            Owner::ObjectOwner(balance_change.address)
        } else {
            Owner::AddressOwner(balance_change.address)
        },
        // TODO use SDK TypeTag
        coin_type: TypeTag::from_str(&balance_change.coin_type.to_canonical_string(false)).unwrap(),
        amount: balance_change.amount,
    }
}

pub fn try_into_sdk_balance_change(
    balance_change: BalanceChange,
) -> anyhow::Result<iota_sdk_types::BalanceChange> {
    match balance_change.owner {
        Owner::AddressOwner(address) | Owner::ObjectOwner(address) => {
            Ok(iota_sdk_types::BalanceChange {
                address,
                // TODO use SDK TypeTag
                coin_type: iota_sdk_types::TypeTag::from_str(
                    &balance_change.coin_type.to_canonical_string(false),
                )
                .unwrap(),
                amount: balance_change.amount,
            })
        }
        _ => anyhow::bail!(
            "Expected address or object owner, got {}",
            balance_change.owner
        ),
    }
}
