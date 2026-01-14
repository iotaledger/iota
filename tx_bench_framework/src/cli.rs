// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

pub const DEFAULT_GAS_BUDGET: u64 = 100_000_000;

#[derive(Parser, Debug)]
#[command(name = "tx-bench-framework")]
#[command(about = "Benchmark tool for transaction latency", long_about = None)]
pub struct Cli {
    /// RPC endpoint. Defaults to local node.
    #[arg(long)]
    pub rpc: Option<String>,

    /// Request tokens from local faucet before actions (dev only).
    #[arg(long)]
    pub use_faucet: bool,

    #[arg(long, default_value_t = DEFAULT_GAS_BUDGET)]
    pub gas_budget: u64,

    #[arg(long, default_value = "tx_bench_state.json")]
    pub state_out: PathBuf,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(ValueEnum, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthenticatorKind {
    Ed25519,
    Ed25519Heavy,
    HelloWorld,
    MaxArgs128,
    MaxArgs255,
}

#[derive(ValueEnum, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SubmitMode {
    Aa,
    Standard,
}

#[derive(Debug)]
pub struct BenchInitSpec {
    pub entry_fn: &'static str,
    pub expected_count: usize,
}

impl AuthenticatorKind {
    pub fn module_name(&self) -> &'static str {
        "abstract_account"
    }

    pub fn function_name(&self) -> &'static str {
        match self {
            AuthenticatorKind::Ed25519 => "authenticate_ed25519",
            AuthenticatorKind::Ed25519Heavy => "authenticate_ed25519_heavy",
            AuthenticatorKind::HelloWorld => "authenticate_hello_world",
            AuthenticatorKind::MaxArgs128 => "authenticate_max_args_128",
            AuthenticatorKind::MaxArgs255 => "authenticate_max_args_255",
        }
    }

    pub fn bench_init_spec(&self) -> Option<BenchInitSpec> {
        match self {
            AuthenticatorKind::MaxArgs128 => Some(BenchInitSpec {
                entry_fn: "create_125_bench_objects",
                expected_count: 125,
            }),
            AuthenticatorKind::MaxArgs255 => Some(BenchInitSpec {
                entry_fn: "create_252_bench_objects",
                expected_count: 252,
            }),
            _ => None,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Init {
        #[arg(long)]
        name: String,

        #[arg(long)]
        aa_package_path: PathBuf,

        #[arg(long, value_enum, default_value_t = AuthenticatorKind::Ed25519)]
        authenticator: AuthenticatorKind,

        #[arg(long, default_value_t = false)]
        force_republish: bool,
    },

    Accounts {
        #[command(subcommand)]
        cmd: AccountsCmd,
    },

    Submit {
        #[arg(long, value_enum)]
        mode: SubmitMode,

        #[arg(long)]
        count: usize,

        #[arg(long)]
        recipient: Option<String>,

        #[arg(long, default_value_t = 1_000)]
        split_amount: u64,

        #[arg(long)]
        account: Option<String>,

        #[arg(long, value_enum, default_value_t = WaitMode::Effects)]
        wait_mode: WaitMode,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum WaitMode {
    Effects,
    Local,
}

impl WaitMode {
    pub fn to_exec_request(
        self,
    ) -> iota_sdk::types::quorum_driver_types::ExecuteTransactionRequestType {
        match self {
            WaitMode::Effects => iota_sdk::types::quorum_driver_types::ExecuteTransactionRequestType::WaitForEffectsCert,
            WaitMode::Local => iota_sdk::types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum AccountsCmd {
    List,
    Use {
        #[arg(long)]
        name: String,
    },
}
