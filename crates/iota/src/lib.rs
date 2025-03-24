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

use colored::Colorize;

pub trait PrintableResult: std::fmt::Display + std::fmt::Debug {
    fn print(&self, pretty: bool) {
        if !self.should_print() {
            return;
        }
        let line = if pretty {
            format!("{self}")
        } else {
            format!("{self:?}")
        };
        // Log line by line
        for line in line.lines() {
            // Logs write to a file on the side. Print to stdout and also log to file, for
            // tests to pass.
            println!("{line}");
            tracing::info!("{line}")
        }
    }

    fn should_print(&self) -> bool {
        true
    }
}

fn unwrap_err_to_string<T: std::fmt::Display, F: FnOnce() -> anyhow::Result<T>>(func: F) -> String {
    match func() {
        Ok(s) => format!("{s}"),
        Err(err) => format!("{err}").red().to_string(),
    }
}
