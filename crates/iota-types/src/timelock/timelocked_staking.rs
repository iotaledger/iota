// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr};

pub const TIMELOCKED_STAKING_MODULE_NAME: &IdentStr = ident_str!("timelocked_staking");

pub const ADD_TIMELOCKED_STAKE_FUN_NAME: &IdentStr = ident_str!("request_add_stake");
pub const WITHDRAW_TIMELOCKED_STAKE_FUN_NAME: &IdentStr = ident_str!("request_withdraw_stake");
