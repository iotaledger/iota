// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod client_commands;
#[macro_use]
pub mod client_ptb;
mod clever_error_rendering;
#[cfg(feature = "gen-completions")]
mod completions;
pub mod displays;
pub mod fire_drill;
pub mod genesis_ceremony;
pub mod genesis_inspector;
pub mod iota_commands;
pub mod key_identity;
pub mod keytool;
pub mod name_commands;
pub mod upgrade_compatibility;
pub mod validator_commands;
mod verifier_meter;
// Commented: https://github.com/iotaledger/iota/issues/1777
// pub mod zklogin_commands_util;
