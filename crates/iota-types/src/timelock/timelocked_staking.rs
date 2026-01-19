// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::IdentifierRef;

pub const TIMELOCKED_STAKING_MODULE_NAME: &IdentifierRef =
    IdentifierRef::const_new("timelocked_staking");

pub const ADD_TIMELOCKED_STAKE_FUN_NAME: &IdentifierRef =
    IdentifierRef::const_new("request_add_stake");
pub const WITHDRAW_TIMELOCKED_STAKE_FUN_NAME: &IdentifierRef =
    IdentifierRef::const_new("request_withdraw_stake");
