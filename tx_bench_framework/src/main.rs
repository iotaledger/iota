// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, time::SystemTime};
pub mod utils;
pub mod registry_state;
pub mod tempo_query;
pub mod aa_initialization;
pub mod tx_type;

use anyhow::{Context, Result, anyhow, bail};
use crate::tx_type::{submit_standard_tx, submit_aa_tx};
use crate::aa_initialization::create_abstract_account;
use clap::{Parser, Subcommand, ValueEnum};
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::{
    IotaClient, IotaClientBuilder,
    types::{
        base_types::{IotaAddress},
        crypto::SignatureScheme::ED25519,
    },
};
use crate::utils::publish_move_package;
use iota_types::{
    base_types::ObjectRef,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ObjectArg},
};
use move_core_types::ident_str;
use serde::{Deserialize, Serialize};

use iota_types::{
    TypeTag,
};
use move_core_types::language_storage::StructTag;
use iota_sdk::types::IOTA_FRAMEWORK_ADDRESS;
use crate::utils::{request_tokens, get_two_distinct_coins};
use crate::tempo_query::print_tempo_traceql_queries;
use crate::registry_state::{AccountState, load_registry, save_registry};
pub const DEFAULT_GAS_BUDGET: u64 = 100_000_000;

const DEFAULT_MNEMONIC: &str = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";

#[derive(Parser, Debug)]
#[command(name = "tx-bench-framework")]
#[command(about = "Publish AA Move package and print publish artifacts (step 1)", long_about = None)]
struct Cli {
    #[arg(long)]
    rpc: Option<String>,

    #[arg(long)]
    use_faucet: bool,

    #[arg(long, default_value_t = DEFAULT_GAS_BUDGET)]
    gas_budget: u64,

    #[arg(long, default_value = "tx_bench_state.json")]
    state_out: PathBuf,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(ValueEnum, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AuthenticatorKind {
    Ed25519,
    Ed25519Heavy,
    HelloWorld,
}

#[derive(ValueEnum, Debug, Clone, Copy, Serialize, Deserialize)]
enum SubmitMode {
    Aa,
    Standard,
}

impl AuthenticatorKind {
    fn module_name(&self) -> &'static str {
        "abstract_account"
    }
    fn function_name(&self) -> &'static str {
        match self {
            AuthenticatorKind::Ed25519 => "authenticate_ed25519",
            AuthenticatorKind::Ed25519Heavy => "authenticate_ed25519_heavy",
            AuthenticatorKind::HelloWorld => "authenticate_hello_world",
        }
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    Init {
        #[arg(long)]
        name: String,

        #[arg(long)]
        aa_package_path: PathBuf,

        #[arg(long, value_enum, default_value_t = AuthenticatorKind::Ed25519)]
        authenticator: AuthenticatorKind,

        // #[arg(long)]
        // force_republish: bool,
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
    },
}

#[derive(Subcommand, Debug)]
enum AccountsCmd {
    List,
    Use {
        #[arg(long)]
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Command::Init {
            name,
            aa_package_path,
            authenticator,
        } => {
            let rpc = cli
                .rpc
                .unwrap_or_else(|| "http://127.0.0.1:9000".to_string());
            let client = build_client(&rpc).await?;

            let mut keystore = InMemKeystore::new_insecure_for_tests(0);
            let sender = keystore
                .import_from_mnemonic(DEFAULT_MNEMONIC, ED25519, None, None)
                .context("import_from_mnemonic failed")?;
            println!("Sender: {sender}");
            println!("RPC: {rpc}");

            if cli.use_faucet {
                println!("Tokens requesting from faucet for sender {sender}");
                request_tokens(&client, sender)
                    .await
                    .context("request_tokens failed (faucet)")?;
                println!("Faucet request completed");
            }

            println!(
                "\n=== Publishing AA Move package from path: {} ===",
                aa_package_path.display()
            );

            let (publish_tx_digest, package_id, metadata_ref) =
                publish_move_package(&client, sender, &keystore, &aa_package_path, cli.gas_budget)
                    .await?;
            let metadata_id = metadata_ref.0;
            println!("\n=== Publish result ===");
            println!("publish_tx_digest: {publish_tx_digest}");
            println!("package_id:         {package_id}");
            println!("package_metadata:   {metadata_id}",);

            println!(
                "\n=== Creating AbstractAccount (auth = {:?}) ===",
                authenticator
            );
            let (aa_ref, aa_addr) = create_abstract_account(
                &client,
                sender,
                &keystore,
                package_id,
                metadata_ref,
                authenticator,
                cli.gas_budget,
            )
            .await?;
            println!("\n=== AbstractAccount created ===");
            println!("aa_object_id:  {}", aa_ref.0);
            println!("aa_version:    {}", aa_ref.1.value());
            println!("aa_digest:     {}", aa_ref.2);
            println!("aa_address:    {}", aa_addr);

            // --- update registry ---
            let mut reg = load_registry(&cli.state_out)?;
            reg.accounts.insert(name.clone(), AccountState {
                created_at_unix_ms: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_millis(),
                rpc: rpc.clone(),
                sender: sender.to_string(),
                publish_tx_digest,
                package_id: package_id.to_string(),
                package_metadata_object_id: metadata_id.to_string(),
                aa_account_object_id: aa_ref.0.to_string(),
                aa_account_version: aa_ref.1.value(),
                aa_account_digest: aa_ref.2.to_string(),
                aa_address: aa_addr.to_string(),
                authenticator,
            });

            reg.active_account = Some(name.clone());
            save_registry(&cli.state_out, &reg)?;
            println!("Saved account '{name}' and set active.");
        }
        Command::Accounts { cmd: AccountsCmd::List } => {
            let reg = load_registry(&cli.state_out)?;
            println!("Active: {}", reg.active_account.as_deref().unwrap_or("<none>"));
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
        Command::Accounts { cmd: AccountsCmd::Use { name } } => {
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
        } => {
            let reg = load_registry(&cli.state_out)?;
            let account_name = account
                .or_else(|| reg.active_account.clone())
                .ok_or_else(|| anyhow!("no active account; use `accounts use --name ...` or pass --account"))?;

            let acc = reg.accounts
                .get(&account_name)
                .ok_or_else(|| anyhow!("account '{account_name}' not found"))?;

            let rpc = acc.rpc.clone();
            let client = build_client(&rpc).await?;

            let mut keystore = InMemKeystore::new_insecure_for_tests(0);
            let sender = keystore
                .import_from_mnemonic(DEFAULT_MNEMONIC, ED25519, None, None)
                .context("import_from_mnemonic failed")?;

            let recipient_addr: IotaAddress = if let Some(r) = recipient {
                r.parse().context("bad recipient address")?
            } else {
                sender
            };

            if cli.use_faucet {
                match mode {
                    SubmitMode::Standard => {
                        request_tokens(&client, sender).await?;
                    }
                    SubmitMode::Aa => {
                        let aa_addr: IotaAddress = acc
                            .aa_address
                            .parse()
                            .context("bad aa_address in state")?;
                        request_tokens(&client, aa_addr).await?;
                    }
                }
            }

            println!("\n=== Submit: mode={mode:?}, count={count}, split_amount={split_amount} ===");

            let mut lat_ms: Vec<u128> = Vec::with_capacity(count);
            let mut digests: Vec<String> = Vec::with_capacity(count);
            let started = std::time::Instant::now();
            for i in 0..count {
                let r = match mode {
                    SubmitMode::Standard => {
                        submit_standard_tx(
                            &client,
                            &keystore,
                            sender,
                            recipient_addr,
                            cli.gas_budget,
                            split_amount,
                        )
                        .await?
                    }
                    SubmitMode::Aa => {
                        submit_aa_tx(
                            &client,
                            &keystore,
                            sender,
                            &acc,
                            recipient_addr,
                            cli.gas_budget,
                            split_amount,
                        )
                        .await?
                    }
                };
                digests.push(r.digest.clone());

                lat_ms.push(r.elapsed_ms);
                println!(
                    "[{i}] digest={} elapsed_ms={} gas_used={}",
                    r.digest,
                    r.elapsed_ms,
                    r.gas_used.unwrap_or_else(|| "<none>".to_string())
                );
            }

            let tx_sender_for_query = match mode {
                SubmitMode::Standard => sender.to_string(),
                SubmitMode::Aa => acc.aa_address.clone(),
            };
            let tempo_service_name = "iota";
            print_tempo_traceql_queries(
                tempo_service_name,
                "handle_transaction",
                &tx_sender_for_query,
                &digests,
            );

            lat_ms.sort();

            let total_ms = started.elapsed().as_millis() as f64;
            let tps = (count as f64) / (total_ms / 1000.0);

            println!("\n=== Batch summary ===");
            println!("count={count} total_ms={total_ms:.2} tps={tps:.2}");
        }
    }

    Ok(())
}

async fn build_client(rpc: &str) -> Result<IotaClient> {
    IotaClientBuilder::default()
        .build(rpc)
        .await
        .map_err(|e| anyhow!("Failed to build IotaClient for {rpc}: {e}"))
}

fn build_split_and_transfer_pt(
    pay_coin_ref: ObjectRef,
    recipient: IotaAddress,
    split_amount: u64,
) -> Result<iota_sdk::types::transaction::ProgrammableTransaction> {
    let mut b = ProgrammableTransactionBuilder::new();

    let iota_type = TypeTag::Struct(Box::new(StructTag {
        address: IOTA_FRAMEWORK_ADDRESS.into(),
        module: ident_str!("iota").to_owned(),
        name: ident_str!("IOTA").to_owned(),
        type_params: vec![],
    }));

    let coin_iota_type = TypeTag::Struct(Box::new(StructTag {
        address: IOTA_FRAMEWORK_ADDRESS.into(),
        module: ident_str!("coin").to_owned(),
        name: ident_str!("Coin").to_owned(),
        type_params: vec![iota_type.clone()],
    }));

    let split_amount_arg = b.pure(split_amount)?;
    let pay_coin_arg = b.obj(ObjectArg::ImmOrOwnedObject(pay_coin_ref))?;
    let split_res = b.programmable_move_call(
        IOTA_FRAMEWORK_ADDRESS.into(),
        ident_str!("coin").to_owned(),
        ident_str!("split").to_owned(),
        vec![iota_type],
        vec![pay_coin_arg, split_amount_arg],
    );

    let recipient_arg = b.pure(recipient)?;
    b.programmable_move_call(
        IOTA_FRAMEWORK_ADDRESS.into(),
        ident_str!("transfer").to_owned(),
        ident_str!("public_transfer").to_owned(),
        vec![coin_iota_type],
        vec![split_res, recipient_arg],
    );

    Ok(b.finish())
}

pub struct SubmitResult {
    digest: String,
    gas_used: Option<String>,
    elapsed_ms: u128,
}