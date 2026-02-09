// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod aa_initialization;
pub mod cli;
pub mod command_handlers;
pub mod registry_state;
pub mod tempo_query;
pub mod tx_type;
pub mod utils;

use anyhow::{Result, bail};
use clap::Parser;

use crate::{
    cli::{AccountsCmd, Cli, Command, TxType},
    command_handlers::{handle_init_command, handle_submit_command},
    registry_state::{load_registry, save_registry},
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let rpc = cli
        .rpc
        .unwrap_or_else(|| "http://127.0.0.1:9000".to_string());

    match cli.cmd {
        Command::Init {
            name,
            aa_package_path,
            authenticator,
            force_republish,
        } => {
            handle_init_command(
                cli.state_out.clone(),
                rpc,
                name,
                &aa_package_path,
                authenticator,
                cli.gas_budget,
                cli.use_faucet,
                force_republish,
            )
            .await?;
        }

        Command::Accounts {
            cmd: AccountsCmd::List,
        } => {
            let reg = load_registry(&cli.state_out)?;
            println!(
                "Active: {}",
                reg.active_account.as_deref().unwrap_or("<none>")
            );

            if reg.accounts.is_empty() {
                println!("No accounts in registry.");
            } else {
                for (name, a) in reg.accounts.iter() {
                    println!(
                        "- {name}: aa_address={} auth={:?} package_id={}",
                        a.aa_address, a.authenticator, a.package_id
                    );
                }
            }
        }

        Command::Accounts {
            cmd: AccountsCmd::Use { name },
        } => {
            let mut reg = load_registry(&cli.state_out)?;
            if !reg.accounts.contains_key(&name) {
                bail!("account '{name}' not found");
            }
            reg.active_account = Some(name.clone());
            save_registry(&cli.state_out, &reg)?;
            println!("Active account set to '{name}'");
        }

        Command::Submit {
            mode,
            count,
            recipient,
            split_amount,
            account,
            wait_mode,
            tx_type,
        } => {
            handle_submit_command(
                cli.state_out.clone(),
                mode,
                count,
                recipient,
                split_amount,
                account,
                cli.gas_budget,
                cli.use_faucet,
                wait_mode,
                tx_type,
            )
            .await?;
        }
    }

    Ok(())
}
