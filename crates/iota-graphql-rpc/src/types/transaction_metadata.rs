// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;

use super::{iota_address::IotaAddress, object::ObjectRef};

/// The optional extra data a user can provide to a transaction dry run.
/// `sender` defaults to `0x0`. If gasObjects` is not present, or is an empty
/// list, it is substituted with a mock Coin object, `gasPrice` defaults to the
/// reference gas price, `gasBudget` defaults to the max gas budget and
/// `gasSponsor` defaults to the sender.
#[derive(Clone, Debug, PartialEq, Eq, InputObject)]
pub(crate) struct TransactionMetadata {
    pub sender: Option<IotaAddress>,
    pub gas_price: Option<u64>,
    pub gas_objects: Option<Vec<ObjectRef>>,
    pub gas_budget: Option<u64>,
    pub gas_sponsor: Option<IotaAddress>,
}
